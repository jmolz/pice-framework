use anyhow::{Context, Result};
use std::path::Path;
use std::process::Command;

/// Build the planning prompt from a user description.
pub fn build_plan_prompt(description: &str, project_root: &Path) -> Result<String> {
    let claude_md = read_claude_md(project_root)?;
    Ok(format!(
        "You are creating a detailed implementation plan.\n\n\
         ## Project Context\n\n{claude_md}\n\n\
         ## Task\n\n\
         Create a comprehensive plan for: {description}\n\n\
         Write the plan to `.claude/plans/` following the plan template format.\n\
         Include a ## Contract section with JSON criteria for adversarial evaluation.\n\
         Set the tier based on complexity (1=simple, 2=feature, 3=architectural)."
    ))
}

/// Build the execution prompt from a parsed plan.
pub fn build_execute_prompt(plan_content: &str, project_root: &Path) -> Result<String> {
    let claude_md = read_claude_md(project_root)?;
    Ok(format!(
        "You are implementing from an approved plan.\n\n\
         ## Project Conventions\n\n{claude_md}\n\n\
         ## Plan\n\n{plan_content}\n\n\
         ## Instructions\n\n\
         Execute the plan tasks in order. After each task, run its validation command.\n\
         Fix errors immediately before proceeding to the next task."
    ))
}

/// Build the evaluation prompt from contract + diff + CLAUDE.md.
/// This is the ONLY context evaluators see — no implementation rationale.
/// Available for use when Rust-side prompt assembly is needed in Phase 3+.
#[allow(dead_code)]
pub fn build_evaluate_prompt(
    contract: &serde_json::Value,
    diff: &str,
    claude_md: &str,
) -> Result<String> {
    let contract_str =
        serde_json::to_string_pretty(contract).context("failed to serialize contract")?;
    Ok(format!(
        "You are an ADVERSARIAL EVALUATOR. Your job is to find failures, not confirm success.\n\n\
         ## Contract\n\n```json\n{contract_str}\n```\n\n\
         ## Code Changes\n\n```diff\n{diff}\n```\n\n\
         ## Project Conventions\n\n{claude_md}\n\n\
         ## Task\n\n\
         For EACH criterion: read the code, try to break it, score 1-10 with evidence.\n\
         Output structured JSON with scores for each criterion."
    ))
}

/// Build the adversarial design challenge prompt (for Codex/GPT).
/// Available for use when Rust-side prompt assembly is needed in Phase 3+.
#[allow(dead_code)]
pub fn build_adversarial_prompt(
    contract: &serde_json::Value,
    diff: &str,
    claude_md: &str,
) -> Result<String> {
    let contract_str =
        serde_json::to_string_pretty(contract).context("failed to serialize contract")?;
    Ok(format!(
        "You are a DESIGN CHALLENGER reviewing code changes.\n\n\
         ## Contract\n\n```json\n{contract_str}\n```\n\n\
         ## Code Changes\n\n```diff\n{diff}\n```\n\n\
         ## Project Conventions\n\n{claude_md}\n\n\
         ## Task\n\n\
         Challenge the APPROACH, not just correctness:\n\
         - Was this the right design? What assumptions does it depend on?\n\
         - Where could it fail under real-world conditions?\n\
         - What alternative approaches were overlooked?\n\
         Categorize findings as: critical, consider, or acknowledged."
    ))
}

/// Read CLAUDE.md from the project root, returning empty string if not found.
pub fn read_claude_md(project_root: &Path) -> Result<String> {
    let claude_md_path = project_root.join("CLAUDE.md");
    match std::fs::read_to_string(&claude_md_path) {
        Ok(content) => Ok(content),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(String::new()),
        Err(e) => Err(e).context("failed to read CLAUDE.md"),
    }
}

/// Get the git diff for evaluation context.
/// Includes both tracked changes (staged + unstaged) and untracked new files.
/// Handles repos with no commits (unborn HEAD) by skipping tracked diff.
pub fn get_git_diff(project_root: &Path) -> Result<String> {
    let mut parts = Vec::new();

    // Tracked changes against HEAD (may fail on unborn HEAD — that's OK)
    let tracked = Command::new("git")
        .args(["diff", "HEAD"])
        .current_dir(project_root)
        .output()
        .context("failed to run git diff HEAD")?;
    if tracked.status.success() {
        let tracked_diff = String::from_utf8_lossy(&tracked.stdout).to_string();
        if !tracked_diff.trim().is_empty() {
            parts.push(tracked_diff);
        }
    } else {
        // HEAD doesn't exist (unborn repo) — collect both staged and unstaged changes.
        // git diff --cached: staged content (index vs empty tree)
        let staged = Command::new("git")
            .args(["diff", "--cached"])
            .current_dir(project_root)
            .output()
            .context("failed to run git diff --cached")?;
        if staged.status.success() {
            let staged_diff = String::from_utf8_lossy(&staged.stdout).to_string();
            if !staged_diff.trim().is_empty() {
                parts.push(staged_diff);
            }
        }
        // git diff (no args): unstaged changes to staged files (worktree vs index)
        let unstaged = Command::new("git")
            .args(["diff"])
            .current_dir(project_root)
            .output()
            .context("failed to run git diff (unstaged)")?;
        if unstaged.status.success() {
            let unstaged_diff = String::from_utf8_lossy(&unstaged.stdout).to_string();
            if !unstaged_diff.trim().is_empty() {
                parts.push(unstaged_diff);
            }
        }
    }

    // Untracked new files — show their full content as diffs
    let untracked = Command::new("git")
        .args(["ls-files", "--others", "--exclude-standard"])
        .current_dir(project_root)
        .output()
        .context("failed to list untracked files")?;
    if untracked.status.success() {
        let files = String::from_utf8_lossy(&untracked.stdout);
        for file in files.lines() {
            let file = file.trim();
            if file.is_empty() {
                continue;
            }
            let file_path = project_root.join(file);
            if let Ok(content) = std::fs::read_to_string(&file_path) {
                parts.push(format!(
                    "diff --git a/{file} b/{file}\nnew file mode 100644\n--- /dev/null\n+++ b/{file}\n{}",
                    content.lines().map(|l| format!("+{l}")).collect::<Vec<_>>().join("\n")
                ));
            }
        }
    }

    // If nothing found, try diff against the previous commit (may not exist in fresh repos)
    if parts.is_empty() {
        let output = Command::new("git")
            .args(["diff", "HEAD~1", "HEAD"])
            .current_dir(project_root)
            .output()
            .context("failed to run git diff HEAD~1")?;
        if output.status.success() {
            return Ok(String::from_utf8_lossy(&output.stdout).to_string());
        }
        // HEAD~1 doesn't exist (single-commit repo) — return empty diff
    }

    Ok(parts.join("\n"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_plan_prompt_includes_description() {
        let dir = tempfile::tempdir().unwrap();
        // No CLAUDE.md — should still work
        let prompt = build_plan_prompt("add user auth", dir.path()).unwrap();
        assert!(prompt.contains("add user auth"));
        assert!(prompt.contains("## Contract"));
    }

    #[test]
    fn build_plan_prompt_includes_claude_md() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("CLAUDE.md"), "# Project Rules\nUse Rust.").unwrap();
        let prompt = build_plan_prompt("add tests", dir.path()).unwrap();
        assert!(prompt.contains("# Project Rules"));
        assert!(prompt.contains("Use Rust."));
    }

    #[test]
    fn build_execute_prompt_includes_plan() {
        let dir = tempfile::tempdir().unwrap();
        let prompt = build_execute_prompt("# My Plan\n\nDo stuff.", dir.path()).unwrap();
        assert!(prompt.contains("# My Plan"));
        assert!(prompt.contains("Do stuff."));
    }

    #[test]
    fn build_evaluate_prompt_format() {
        let contract = serde_json::json!({"feature": "auth", "tier": 2});
        let prompt = build_evaluate_prompt(&contract, "+added line", "# Rules").unwrap();
        assert!(prompt.contains("ADVERSARIAL EVALUATOR"));
        assert!(prompt.contains("+added line"));
        assert!(prompt.contains("# Rules"));
    }

    #[test]
    fn build_adversarial_prompt_format() {
        let contract = serde_json::json!({"feature": "auth"});
        let prompt = build_adversarial_prompt(&contract, "+line", "# Rules").unwrap();
        assert!(prompt.contains("DESIGN CHALLENGER"));
        assert!(prompt.contains("critical, consider, or acknowledged"));
    }

    #[test]
    fn read_claude_md_missing_returns_empty() {
        let dir = tempfile::tempdir().unwrap();
        let result = read_claude_md(dir.path()).unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn get_git_diff_unborn_head() {
        let dir = tempfile::tempdir().unwrap();
        // Init a repo but don't commit — HEAD is unborn
        std::process::Command::new("git")
            .args(["init"])
            .current_dir(dir.path())
            .output()
            .unwrap();
        // Create a file so there's something to find
        std::fs::write(dir.path().join("test.txt"), "hello").unwrap();

        // Should NOT panic or error — returns the untracked file as diff
        let result = get_git_diff(dir.path()).unwrap();
        assert!(result.contains("test.txt"));
        assert!(result.contains("+hello"));
    }

    #[test]
    fn get_git_diff_includes_untracked_files() {
        let dir = tempfile::tempdir().unwrap();
        std::process::Command::new("git")
            .args(["init"])
            .current_dir(dir.path())
            .output()
            .unwrap();
        std::process::Command::new("git")
            .args([
                "-c",
                "user.name=Test",
                "-c",
                "user.email=test@test.com",
                "commit",
                "--allow-empty",
                "-m",
                "init",
            ])
            .current_dir(dir.path())
            .output()
            .unwrap();

        // Create an untracked file
        std::fs::write(dir.path().join("new-file.rs"), "fn main() {}").unwrap();

        let result = get_git_diff(dir.path()).unwrap();
        assert!(result.contains("new-file.rs"));
        assert!(result.contains("+fn main() {}"));
    }

    #[test]
    fn get_git_diff_staged_files_unborn_head() {
        let dir = tempfile::tempdir().unwrap();
        std::process::Command::new("git")
            .args(["init"])
            .current_dir(dir.path())
            .output()
            .unwrap();
        // Create and stage a file — no commit yet
        std::fs::write(dir.path().join("staged.rs"), "fn staged() {}").unwrap();
        std::process::Command::new("git")
            .args(["add", "staged.rs"])
            .current_dir(dir.path())
            .output()
            .unwrap();

        // Should capture staged file via git diff --cached
        let result = get_git_diff(dir.path()).unwrap();
        assert!(result.contains("staged.rs"));
    }
}
