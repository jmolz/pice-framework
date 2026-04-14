//! Workflow file loading — embedded defaults, project file, user file, resolve().
//!
//! Resolution order: framework → project → user. Each level is merged into the
//! previous via [`merge::merge_with_floor`]. Schema version `"0.2"` is checked
//! on every loaded config before any merge happens — a mismatch is a hard error.

use anyhow::{anyhow, Context, Result};
use std::path::{Path, PathBuf};

use crate::workflow::merge::{merge_with_floor, overlay as overlay_merge};
use crate::workflow::schema::WorkflowConfig;
use crate::workflow::SCHEMA_VERSION;

const FRAMEWORK_WORKFLOW_YAML: &str = include_str!("../../../../templates/pice/workflow.yaml");

/// The framework-default workflow, embedded in the binary at build time.
///
/// Panics only if the embedded YAML is malformed, which is asserted by the
/// `embedded_defaults_parses` unit test. This is the sole exception to the
/// no-unwrap rule in pice-core: the embedded string is build-time data.
pub fn embedded_defaults() -> WorkflowConfig {
    serde_yaml::from_str(FRAMEWORK_WORKFLOW_YAML)
        .expect("embedded framework workflow.yaml must parse (build-time asserted)")
}

/// Load `<project_root>/.pice/workflow.yaml`. Returns `Ok(None)` if absent.
pub fn load_project(project_root: &Path) -> Result<Option<WorkflowConfig>> {
    let path = project_root.join(".pice").join("workflow.yaml");
    load_from_path_optional(&path)
}

/// Load `~/.pice/workflow.yaml`. Returns `Ok(None)` if absent.
pub fn load_user() -> Result<Option<WorkflowConfig>> {
    let home = home_dir()?;
    let path = home.join(".pice").join("workflow.yaml");
    load_from_path_optional(&path)
}

/// Resolve the effective workflow for a project by merging framework →
/// project → user. Framework defaults always apply; project and user layers
/// are optional. Floor violations at either merge step are returned as errors.
pub fn resolve(project_root: &Path) -> Result<WorkflowConfig> {
    let framework = embedded_defaults();
    let project = load_project(project_root)?;
    let user = load_user()?;

    // Framework → project: simple overlay. PRDv2 floor semantics apply only
    // to project → user (lines 903–918).
    let after_project = match project {
        Some(p) => overlay_merge(framework, p),
        None => framework,
    };

    let effective = match user {
        Some(u) => merge_with_floor(after_project, u).context("merging user workflow.yaml")?,
        None => after_project,
    };

    Ok(effective)
}

fn load_from_path_optional(path: &Path) -> Result<Option<WorkflowConfig>> {
    if !path.exists() {
        return Ok(None);
    }
    let content = std::fs::read_to_string(path)
        .with_context(|| format!("failed to read workflow from {}", path.display()))?;
    let cfg: WorkflowConfig = serde_yaml::from_str(&content)
        .with_context(|| format!("failed to parse YAML at {}", path.display()))?;
    check_schema_version(&cfg, path)?;
    Ok(Some(cfg))
}

fn check_schema_version(cfg: &WorkflowConfig, path: &Path) -> Result<()> {
    if cfg.schema_version != SCHEMA_VERSION {
        return Err(anyhow!(
            "workflow at {} declares schema_version \"{}\"; expected \"{}\". \
             Upgrade the file by bumping `schema_version` and reviewing fields \
             against PRDv2 § Feature 4.",
            path.display(),
            cfg.schema_version,
            SCHEMA_VERSION
        ));
    }
    Ok(())
}

/// Resolve the user's home directory via environment variables.
/// Mirrors [`layers::manifest::home_dir`] — HOME on Unix, USERPROFILE on
/// Windows, with cross-fallback. Kept crate-local to avoid a public re-export
/// of an infrastructural helper.
fn home_dir() -> Result<PathBuf> {
    std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .map(PathBuf::from)
        .context("could not determine home directory (neither HOME nor USERPROFILE is set)")
}

// ─── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn write(path: &Path, s: &str) {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        std::fs::write(path, s).unwrap();
    }

    #[test]
    fn embedded_defaults_parses() {
        let cfg = embedded_defaults();
        assert_eq!(cfg.schema_version, "0.2");
        assert_eq!(cfg.defaults.tier, 2);
        assert!((cfg.defaults.min_confidence - 0.90).abs() < 1e-9);
    }

    #[test]
    fn load_project_absent_returns_none() {
        let tmp = tempdir().unwrap();
        let out = load_project(tmp.path()).unwrap();
        assert!(out.is_none());
    }

    #[test]
    fn load_project_valid() {
        let tmp = tempdir().unwrap();
        let yaml = r#"
schema_version: "0.2"
defaults:
  tier: 3
  min_confidence: 0.95
  max_passes: 7
  model: opus
  budget_usd: 4.0
  cost_cap_behavior: halt
"#;
        write(&tmp.path().join(".pice/workflow.yaml"), yaml);
        let cfg = load_project(tmp.path()).unwrap().unwrap();
        assert_eq!(cfg.defaults.tier, 3);
        assert_eq!(cfg.defaults.model, "opus");
    }

    #[test]
    fn load_project_bad_schema_version() {
        let tmp = tempdir().unwrap();
        let yaml = r#"
schema_version: "0.1"
defaults:
  tier: 2
  min_confidence: 0.9
  max_passes: 5
  model: sonnet
  budget_usd: 2.0
  cost_cap_behavior: halt
"#;
        write(&tmp.path().join(".pice/workflow.yaml"), yaml);
        let err = load_project(tmp.path()).unwrap_err().to_string();
        assert!(
            err.contains("0.1"),
            "error should cite the invalid version: {err}"
        );
        assert!(
            err.contains("0.2"),
            "error should cite the expected version: {err}"
        );
    }

    #[test]
    fn load_project_malformed_yaml() {
        let tmp = tempdir().unwrap();
        write(
            &tmp.path().join(".pice/workflow.yaml"),
            "schema_version: \"0.2\"\ndefaults: [this is not a map",
        );
        let err = load_project(tmp.path()).unwrap_err().to_string();
        assert!(
            err.contains("parse"),
            "error should mention parse failure: {err}"
        );
    }

    #[test]
    fn load_user_missing_home_var_errors() {
        let prev_home = std::env::var_os("HOME");
        let prev_profile = std::env::var_os("USERPROFILE");
        // SAFETY: test mutates process env; serial only within this test.
        unsafe {
            std::env::remove_var("HOME");
            std::env::remove_var("USERPROFILE");
        }
        let err = load_user().unwrap_err().to_string();
        // SAFETY: restore env before other tests run.
        unsafe {
            if let Some(v) = prev_home {
                std::env::set_var("HOME", v);
            }
            if let Some(v) = prev_profile {
                std::env::set_var("USERPROFILE", v);
            }
        }
        assert!(err.contains("home"), "error should mention home: {err}");
    }

    #[test]
    fn resolve_no_overrides_returns_framework() {
        let tmp = tempdir().unwrap();
        let cfg = resolve(tmp.path()).unwrap();
        assert_eq!(cfg, embedded_defaults());
    }

    #[test]
    fn resolve_project_overrides_framework() {
        let tmp = tempdir().unwrap();
        // Project strictens every floor-guarded field (tier, min_confidence,
        // max_passes up; budget_usd down) — all valid direction.
        let yaml = r#"
schema_version: "0.2"
defaults:
  tier: 3
  min_confidence: 0.95
  max_passes: 7
  model: opus
  budget_usd: 1.5
  cost_cap_behavior: halt
"#;
        write(&tmp.path().join(".pice/workflow.yaml"), yaml);
        let cfg = resolve(tmp.path()).unwrap();
        assert_eq!(cfg.defaults.tier, 3);
        assert_eq!(cfg.defaults.model, "opus");
        assert!((cfg.defaults.budget_usd - 1.5).abs() < 1e-9);
    }
}
