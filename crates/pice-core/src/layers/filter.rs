//! Git diff filtering by layer glob patterns and layer-specific prompt builder.
//!
//! Pure text processing — no filesystem access, no async, no network.
//! Used by the daemon to extract per-layer diffs from a full `git diff` output
//! and to construct context-isolated evaluation prompts for each layer.

/// Filter a unified diff to include only files matching the given glob patterns.
///
/// Parse unified diff format:
/// - Split on `diff --git a/... b/...` headers
/// - Extract file path from each section (new path from `b/`, or old path for deletions)
/// - Match against glob patterns using `glob::Pattern`
/// - Reassemble only matching sections, preserving diff headers
///
/// Returns an empty string if no files match or the input is empty.
pub fn filter_diff_by_globs(full_diff: &str, globs: &[String]) -> String {
    if full_diff.is_empty() || globs.is_empty() {
        return String::new();
    }

    let patterns: Vec<glob::Pattern> = globs
        .iter()
        .filter_map(|g| glob::Pattern::new(g).ok())
        .collect();

    if patterns.is_empty() {
        return String::new();
    }

    let sections = split_diff_sections(full_diff);
    let mut matched = Vec::new();

    for section in &sections {
        if let Some(file_path) = extract_file_path(section) {
            if patterns.iter().any(|p| p.matches(&file_path)) {
                matched.push(*section);
            }
        }
    }

    if matched.is_empty() {
        return String::new();
    }

    matched.join("\n")
}

/// Build a context-isolated evaluation prompt for a single layer.
///
/// The prompt is structured so the evaluator sees ONLY the layer's contract,
/// its filtered diff, and the project-level CLAUDE.md. No cross-layer
/// information leaks into the prompt.
pub fn build_layer_prompt(
    layer_name: &str,
    contract_toml: &str,
    filtered_diff: &str,
    claude_md: &str,
) -> String {
    format!(
        r#"You are evaluating the "{layer_name}" layer ONLY.

## Contract

{contract_toml}

## Code Changes (filtered to {layer_name} layer)

```diff
{filtered_diff}
```

## Project Conventions

{claude_md}

IMPORTANT: You are evaluating ONLY the {layer_name} layer. Do not consider
changes to other layers. Grade strictly against the contract criteria above."#
    )
}

// ─── Internal helpers ──────────────────────────────────────────────────────

/// Split a unified diff into per-file sections.
///
/// Each section starts with `diff --git a/... b/...` and extends until
/// the next `diff --git` header or end of input.
fn split_diff_sections(diff: &str) -> Vec<&str> {
    let mut sections = Vec::new();
    let mut last_start = None;

    for (idx, _) in diff.match_indices("diff --git ") {
        // Only split on `diff --git ` at the start of a line
        if idx == 0 || diff.as_bytes().get(idx.wrapping_sub(1)) == Some(&b'\n') {
            if let Some(start) = last_start {
                // Trim trailing newline from previous section
                let section = diff[start..idx].trim_end_matches('\n');
                sections.push(section);
            }
            last_start = Some(idx);
        }
    }

    // Push the last section
    if let Some(start) = last_start {
        let section = diff[start..].trim_end_matches('\n');
        sections.push(section);
    }

    sections
}

/// Extract the relevant file path from a diff section.
///
/// Strategy:
/// 1. Parse the `diff --git a/... b/...` header for the `b/` path
/// 2. For deleted files (`+++ /dev/null`), use the `a/` path from `--- a/...`
/// 3. For new files (`--- /dev/null`), use the `b/` path from `+++ b/...`
/// 4. Handle quoted paths (paths with spaces)
fn extract_file_path(section: &str) -> Option<String> {
    let first_line = section.lines().next()?;

    // Check for deleted and new files by scanning `---` and `+++` lines
    let mut minus_path: Option<String> = None;
    let mut plus_path: Option<String> = None;

    for line in section.lines() {
        if let Some(rest) = line.strip_prefix("+++ ") {
            plus_path = Some(rest.to_string());
        } else if let Some(rest) = line.strip_prefix("--- ") {
            minus_path = Some(rest.to_string());
        }
        // Stop scanning after we have both (don't read into hunk content)
        if minus_path.is_some() && plus_path.is_some() {
            break;
        }
    }

    // Deleted file: +++ /dev/null -> use --- a/path
    if plus_path.as_deref() == Some("/dev/null") {
        if let Some(ref minus) = minus_path {
            return strip_prefix_path(minus);
        }
    }

    // New file: --- /dev/null -> use +++ b/path
    if minus_path.as_deref() == Some("/dev/null") {
        if let Some(ref plus) = plus_path {
            return strip_prefix_path(plus);
        }
    }

    // Normal case: extract b/ path from the diff --git header
    extract_b_path_from_header(first_line)
}

/// Extract the `b/...` path from a `diff --git a/... b/...` header.
///
/// The header format is: `diff --git a/path b/path`
/// For paths with spaces, git may quote them:
/// `diff --git "a/path with spaces" "b/path with spaces"`
fn extract_b_path_from_header(header: &str) -> Option<String> {
    let rest = header.strip_prefix("diff --git ")?;

    // Handle quoted paths: diff --git "a/foo bar" "b/foo bar"
    if rest.contains('"') {
        // Find the last quoted string — that's the b/ path
        let mut last_quoted = None;
        let mut in_quote = false;
        let mut quote_start = 0;

        for (i, ch) in rest.char_indices() {
            if ch == '"' {
                if in_quote {
                    last_quoted = Some(&rest[quote_start + 1..i]);
                    in_quote = false;
                } else {
                    quote_start = i;
                    in_quote = true;
                }
            }
        }

        if let Some(quoted) = last_quoted {
            return strip_prefix_path(quoted);
        }
    }

    // Unquoted paths: diff --git a/path b/path
    // Find " b/" — the separator between a-path and b-path.
    // We scan for " b/" to handle paths that themselves contain spaces.
    if let Some(pos) = rest.find(" b/") {
        let b_path = &rest[pos + 1..]; // skip the leading space
        return strip_prefix_path(b_path);
    }

    None
}

/// Strip the `a/` or `b/` prefix from a diff path, handling quoted paths.
fn strip_prefix_path(path: &str) -> Option<String> {
    let trimmed = path.trim_matches('"');
    if let Some(rest) = trimmed.strip_prefix("b/") {
        Some(rest.to_string())
    } else if let Some(rest) = trimmed.strip_prefix("a/") {
        Some(rest.to_string())
    } else {
        // Bare path (shouldn't happen in well-formed diffs, but handle gracefully)
        Some(trimmed.to_string())
    }
}

// ─── Tests ─────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    /// A multi-file diff with Rust, TypeScript, and Python files.
    fn sample_three_file_diff() -> String {
        [
            "diff --git a/src/server/main.rs b/src/server/main.rs",
            "index abc1234..def5678 100644",
            "--- a/src/server/main.rs",
            "+++ b/src/server/main.rs",
            "@@ -1,5 +1,6 @@",
            " fn main() {",
            "+    println!(\"BACKEND_MARKER_XYZ\");",
            "     server::start();",
            " }",
            "diff --git a/src/client/app.ts b/src/client/app.ts",
            "index 1111111..2222222 100644",
            "--- a/src/client/app.ts",
            "+++ b/src/client/app.ts",
            "@@ -1,3 +1,4 @@",
            " import { render } from './render';",
            "+console.log(\"FRONTEND_MARKER_ABC\");",
            " render();",
            "diff --git a/scripts/build.py b/scripts/build.py",
            "index aaaaaaa..bbbbbbb 100644",
            "--- a/scripts/build.py",
            "+++ b/scripts/build.py",
            "@@ -1,2 +1,3 @@",
            " import os",
            "+print(\"SCRIPT_MARKER_123\")",
            " build()",
        ]
        .join("\n")
    }

    #[test]
    fn basic_filter() {
        let diff = sample_three_file_diff();
        let globs = vec!["**/*.rs".to_string()];
        let result = filter_diff_by_globs(&diff, &globs);

        assert!(
            result.contains("src/server/main.rs"),
            "should contain the .rs file"
        );
        assert!(
            result.contains("BACKEND_MARKER_XYZ"),
            "should contain the .rs hunk content"
        );
        assert!(
            !result.contains("src/client/app.ts"),
            "should NOT contain the .ts file"
        );
        assert!(
            !result.contains("scripts/build.py"),
            "should NOT contain the .py file"
        );
    }

    #[test]
    fn multi_glob_filter() {
        let diff = sample_three_file_diff();
        let globs = vec!["**/*.rs".to_string(), "**/*.ts".to_string()];
        let result = filter_diff_by_globs(&diff, &globs);

        assert!(
            result.contains("src/server/main.rs"),
            "should contain .rs file"
        );
        assert!(
            result.contains("src/client/app.ts"),
            "should contain .ts file"
        );
        assert!(
            !result.contains("scripts/build.py"),
            "should NOT contain .py file"
        );
    }

    #[test]
    fn no_match() {
        let diff = sample_three_file_diff();
        let globs = vec!["**/*.go".to_string()];
        let result = filter_diff_by_globs(&diff, &globs);

        assert!(
            result.is_empty(),
            "no Go files in the diff, should be empty"
        );
    }

    #[test]
    fn binary_file() {
        let diff = [
            "diff --git a/assets/logo.png b/assets/logo.png",
            "index 0000000..1234567 100644",
            "Binary files /dev/null and b/assets/logo.png differ",
        ]
        .join("\n");

        let globs = vec!["assets/**".to_string()];
        let result = filter_diff_by_globs(&diff, &globs);

        assert!(
            result.contains("assets/logo.png"),
            "binary file should be included when glob matches"
        );
        assert!(
            result.contains("Binary files"),
            "should preserve the binary files line"
        );
    }

    #[test]
    fn rename() {
        let diff = [
            "diff --git a/old/handler.rs b/new/handler.rs",
            "similarity index 95%",
            "rename from old/handler.rs",
            "rename to new/handler.rs",
            "index abc1234..def5678 100644",
            "--- a/old/handler.rs",
            "+++ b/new/handler.rs",
            "@@ -1,3 +1,3 @@",
            " fn handle() {",
            "-    old_logic();",
            "+    new_logic();",
            " }",
        ]
        .join("\n");

        // Match on the new path
        let globs = vec!["new/**".to_string()];
        let result = filter_diff_by_globs(&diff, &globs);

        assert!(
            result.contains("new/handler.rs"),
            "renamed file should match on new path"
        );

        // The old path glob should NOT match
        let globs_old = vec!["old/**".to_string()];
        let result_old = filter_diff_by_globs(&diff, &globs_old);
        assert!(
            result_old.is_empty(),
            "should not match on old (renamed-from) path"
        );
    }

    #[test]
    fn new_file() {
        let diff = [
            "diff --git a/src/server/new_file.rs b/src/server/new_file.rs",
            "new file mode 100644",
            "index 0000000..abc1234",
            "--- /dev/null",
            "+++ b/src/server/new_file.rs",
            "@@ -0,0 +1,3 @@",
            "+fn new_function() {",
            "+    // brand new file",
            "+}",
        ]
        .join("\n");

        let globs = vec!["**/*.rs".to_string()];
        let result = filter_diff_by_globs(&diff, &globs);

        assert!(
            result.contains("src/server/new_file.rs"),
            "new file should be matched by glob"
        );
        assert!(
            result.contains("new_function"),
            "new file content should be preserved"
        );
    }

    #[test]
    fn deleted_file() {
        let diff = [
            "diff --git a/src/server/old_file.rs b/src/server/old_file.rs",
            "deleted file mode 100644",
            "index abc1234..0000000",
            "--- a/src/server/old_file.rs",
            "+++ /dev/null",
            "@@ -1,3 +0,0 @@",
            "-fn deprecated() {",
            "-    // this file was removed",
            "-}",
        ]
        .join("\n");

        let globs = vec!["**/*.rs".to_string()];
        let result = filter_diff_by_globs(&diff, &globs);

        assert!(
            result.contains("src/server/old_file.rs"),
            "deleted file should be matched by glob on old path"
        );
        assert!(
            result.contains("deprecated"),
            "deleted file content should be preserved"
        );
    }

    #[test]
    fn build_layer_prompt_output() {
        let contract = "[criteria]\n\
                         performance = \"API response time < 200ms\"\n\
                         error_handling = \"All endpoints return structured errors\"";

        let diff = [
            "diff --git a/src/server/routes.rs b/src/server/routes.rs",
            "--- a/src/server/routes.rs",
            "+++ b/src/server/routes.rs",
            "@@ -1,3 +1,4 @@",
            " fn routes() {",
            "+    add_health_check();",
            " }",
        ]
        .join("\n");

        let claude_md = "## Project Conventions\n\n- Use structured error types";

        let prompt = build_layer_prompt("backend", contract, &diff, claude_md);

        assert!(
            prompt.contains("evaluating the \"backend\" layer ONLY"),
            "should contain layer name in intro"
        );
        assert!(
            prompt.contains("API response time < 200ms"),
            "should contain contract text"
        );
        assert!(
            prompt.contains("add_health_check"),
            "should contain diff content"
        );
        assert!(
            prompt.contains("structured error types"),
            "should contain CLAUDE.md content"
        );
        assert!(
            prompt.contains("ONLY the backend layer"),
            "should contain isolation instruction"
        );
        assert!(
            prompt.contains("filtered to backend layer"),
            "should label the diff section with the layer name"
        );
    }

    #[test]
    fn context_isolation_negative() {
        // Build a multi-layer diff, filter to backend only, then verify
        // the prompt does NOT contain frontend markers.
        let full_diff = sample_three_file_diff();
        let backend_globs = vec!["src/server/**".to_string()];
        let filtered = filter_diff_by_globs(&full_diff, &backend_globs);

        let contract = "[criteria]\nbackend_ok = true";
        let claude_md = "# CLAUDE.md\nProject rules here.";

        let prompt = build_layer_prompt("backend", contract, &filtered, claude_md);

        // The prompt should contain backend markers
        assert!(
            prompt.contains("BACKEND_MARKER_XYZ"),
            "backend marker should be present"
        );

        // The prompt should NOT contain frontend or script markers
        assert!(
            !prompt.contains("FRONTEND_MARKER_ABC"),
            "frontend marker must NOT leak into backend prompt"
        );
        assert!(
            !prompt.contains("SCRIPT_MARKER_123"),
            "script marker must NOT leak into backend prompt"
        );
    }

    #[test]
    fn empty_diff_returns_empty() {
        let globs = vec!["**/*.rs".to_string()];
        let result = filter_diff_by_globs("", &globs);
        assert!(result.is_empty());
    }

    #[test]
    fn empty_globs_returns_empty() {
        let diff = sample_three_file_diff();
        let result = filter_diff_by_globs(&diff, &[]);
        assert!(result.is_empty());
    }

    #[test]
    fn preserves_diff_header() {
        let diff = sample_three_file_diff();
        let globs = vec!["**/*.rs".to_string()];
        let result = filter_diff_by_globs(&diff, &globs);

        assert!(
            result.starts_with("diff --git"),
            "filtered output should start with diff header"
        );
        assert!(
            result.contains("index abc1234..def5678"),
            "should preserve index line"
        );
        assert!(
            result.contains("--- a/src/server/main.rs"),
            "should preserve --- line"
        );
        assert!(
            result.contains("+++ b/src/server/main.rs"),
            "should preserve +++ line"
        );
    }

    #[test]
    fn split_diff_sections_count() {
        let diff = sample_three_file_diff();
        let sections = split_diff_sections(&diff);
        assert_eq!(sections.len(), 3, "should split into 3 file sections");
    }

    #[test]
    fn extract_path_normal_file() {
        let section = [
            "diff --git a/src/main.rs b/src/main.rs",
            "index abc..def 100644",
            "--- a/src/main.rs",
            "+++ b/src/main.rs",
        ]
        .join("\n");
        let path = extract_file_path(&section);
        assert_eq!(path, Some("src/main.rs".to_string()));
    }

    #[test]
    fn extract_path_new_file() {
        let section = [
            "diff --git a/new.rs b/new.rs",
            "new file mode 100644",
            "--- /dev/null",
            "+++ b/new.rs",
        ]
        .join("\n");
        let path = extract_file_path(&section);
        assert_eq!(path, Some("new.rs".to_string()));
    }

    #[test]
    fn extract_path_deleted_file() {
        let section = [
            "diff --git a/old.rs b/old.rs",
            "deleted file mode 100644",
            "--- a/old.rs",
            "+++ /dev/null",
        ]
        .join("\n");
        let path = extract_file_path(&section);
        assert_eq!(path, Some("old.rs".to_string()));
    }

    // ─── Comprehensive context isolation harness (Task 19) ──────────────

    /// Full 3-layer context isolation test: the most important test in Phase 1.
    ///
    /// Creates a multi-file diff with unique marker strings per layer,
    /// then verifies each layer's prompt contains ONLY its own markers
    /// and NONE of the other layers' markers.
    #[test]
    fn context_isolation_three_layer_harness() {
        let diff = [
            "diff --git a/src/server/handler.rs b/src/server/handler.rs",
            "index aaa..bbb 100644",
            "--- a/src/server/handler.rs",
            "+++ b/src/server/handler.rs",
            "@@ -1,3 +1,4 @@",
            " fn handle() {",
            "+    println!(\"BACKEND_MARKER_XYZ\");",
            "     do_stuff();",
            " }",
            "diff --git a/pages/index.tsx b/pages/index.tsx",
            "index ccc..ddd 100644",
            "--- a/pages/index.tsx",
            "+++ b/pages/index.tsx",
            "@@ -1,3 +1,4 @@",
            " export default function Home() {",
            "+  return <div>FRONTEND_MARKER_ABC</div>;",
            "   return <div>Hello</div>;",
            " }",
            "diff --git a/prisma/schema.prisma b/prisma/schema.prisma",
            "index eee..fff 100644",
            "--- a/prisma/schema.prisma",
            "+++ b/prisma/schema.prisma",
            "@@ -1,3 +1,4 @@",
            " model User {",
            "+  email String // DATABASE_MARKER_123",
            "   id Int @id",
            " }",
        ]
        .join("\n");

        let backend_globs = vec!["src/server/**".to_string()];
        let frontend_globs = vec!["pages/**".to_string()];
        let database_globs = vec!["prisma/**".to_string()];

        let backend_contract = "[backend]\ncriteria = \"code quality\"";
        let frontend_contract = "[frontend]\ncriteria = \"route coverage\"";
        let database_contract = "[database]\ncriteria = \"schema safety\"";
        let claude_md = "# CLAUDE.md\nProject conventions.";

        // Filter and build prompts
        let backend_diff = filter_diff_by_globs(&diff, &backend_globs);
        let frontend_diff = filter_diff_by_globs(&diff, &frontend_globs);
        let database_diff = filter_diff_by_globs(&diff, &database_globs);

        let backend_prompt =
            build_layer_prompt("backend", backend_contract, &backend_diff, claude_md);
        let frontend_prompt =
            build_layer_prompt("frontend", frontend_contract, &frontend_diff, claude_md);
        let database_prompt =
            build_layer_prompt("database", database_contract, &database_diff, claude_md);

        // BACKEND: contains own marker, not others
        assert!(
            backend_prompt.contains("BACKEND_MARKER_XYZ"),
            "backend prompt must contain BACKEND_MARKER_XYZ"
        );
        assert!(
            !backend_prompt.contains("FRONTEND_MARKER_ABC"),
            "backend prompt must NOT contain FRONTEND_MARKER_ABC"
        );
        assert!(
            !backend_prompt.contains("DATABASE_MARKER_123"),
            "backend prompt must NOT contain DATABASE_MARKER_123"
        );

        // FRONTEND: contains own marker, not others
        assert!(
            frontend_prompt.contains("FRONTEND_MARKER_ABC"),
            "frontend prompt must contain FRONTEND_MARKER_ABC"
        );
        assert!(
            !frontend_prompt.contains("BACKEND_MARKER_XYZ"),
            "frontend prompt must NOT contain BACKEND_MARKER_XYZ"
        );
        assert!(
            !frontend_prompt.contains("DATABASE_MARKER_123"),
            "frontend prompt must NOT contain DATABASE_MARKER_123"
        );

        // DATABASE: contains own marker, not others
        assert!(
            database_prompt.contains("DATABASE_MARKER_123"),
            "database prompt must contain DATABASE_MARKER_123"
        );
        assert!(
            !database_prompt.contains("BACKEND_MARKER_XYZ"),
            "database prompt must NOT contain BACKEND_MARKER_XYZ"
        );
        assert!(
            !database_prompt.contains("FRONTEND_MARKER_ABC"),
            "database prompt must NOT contain FRONTEND_MARKER_ABC"
        );

        // Each prompt contains its own contract, not other contracts
        assert!(backend_prompt.contains("code quality"));
        assert!(!backend_prompt.contains("route coverage"));
        assert!(!backend_prompt.contains("schema safety"));

        assert!(frontend_prompt.contains("route coverage"));
        assert!(!frontend_prompt.contains("code quality"));
        assert!(!frontend_prompt.contains("schema safety"));

        assert!(database_prompt.contains("schema safety"));
        assert!(!database_prompt.contains("code quality"));
        assert!(!database_prompt.contains("route coverage"));

        // Each prompt contains CLAUDE.md
        assert!(backend_prompt.contains("Project conventions"));
        assert!(frontend_prompt.contains("Project conventions"));
        assert!(database_prompt.contains("Project conventions"));

        // Each prompt mentions the correct layer name
        assert!(backend_prompt.contains("\"backend\""));
        assert!(frontend_prompt.contains("\"frontend\""));
        assert!(database_prompt.contains("\"database\""));
    }
}
