//! Pure path helpers shared by CLI (status reporting) and daemon (metrics writes).
//!
//! Moved from `pice-cli/src/metrics/mod.rs` in T7 of the Phase 0 refactor.
//! The `pice-cli` metrics module re-exports `normalize_plan_path` so existing
//! callers (`commands/execute.rs`, `commands/evaluate.rs`) can keep using
//! `metrics::normalize_plan_path(..)` without churn — the implementation now
//! lives here.
//!
//! From `.claude/rules/metrics.md` ("Plan Path Normalization"): plan paths
//! stored in the metrics DB must be normalized to project-relative canonical
//! form so different invocation spellings (absolute, relative, `./`-prefixed)
//! don't fragment history. The canonical form is `.claude/plans/<filename>`,
//! matching what `pice status` uses for lookups.

use std::path::Path;

/// Normalize a plan path to a project-relative canonical form.
/// Converts absolute paths and various relative spellings to `.claude/plans/<filename>`.
/// This ensures consistent keys in the metrics DB regardless of how the user invoked the command.
pub fn normalize_plan_path(plan_path: &str, project_root: &Path) -> String {
    let path = std::path::Path::new(plan_path);

    // Try to extract the filename
    let filename = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or(plan_path);

    // If the path contains ".claude/plans/", extract the suffix after it
    if let Some(idx) = plan_path.find(".claude/plans/") {
        return plan_path[idx..].to_string();
    }

    // If it's an absolute path, try to make it relative to project_root
    if path.is_absolute() {
        if let Ok(rel) = path.strip_prefix(project_root) {
            return rel.to_string_lossy().to_string();
        }
    }

    // Default: normalize to .claude/plans/<filename>
    format!(".claude/plans/{filename}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn normalize_relative_path() {
        let root = PathBuf::from("/project");
        assert_eq!(
            normalize_plan_path(".claude/plans/test.md", &root),
            ".claude/plans/test.md"
        );
    }

    #[test]
    fn normalize_absolute_path_with_project_root() {
        let root = PathBuf::from("/project");
        assert_eq!(
            normalize_plan_path("/project/.claude/plans/test.md", &root),
            ".claude/plans/test.md"
        );
    }

    #[test]
    fn normalize_dotslash_path() {
        let root = PathBuf::from("/project");
        assert_eq!(
            normalize_plan_path("./.claude/plans/test.md", &root),
            ".claude/plans/test.md"
        );
    }

    #[test]
    fn normalize_bare_filename() {
        let root = PathBuf::from("/project");
        assert_eq!(
            normalize_plan_path("test.md", &root),
            ".claude/plans/test.md"
        );
    }

    #[test]
    fn normalize_absolute_outside_project() {
        let root = PathBuf::from("/project");
        // Absolute path outside project root falls back to filename
        assert_eq!(
            normalize_plan_path("/other/place/test.md", &root),
            ".claude/plans/test.md"
        );
    }
}
