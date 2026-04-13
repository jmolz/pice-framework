//! `pice init` handler — scaffold `.claude/` and `.pice/` directories.

use anyhow::Result;
use pice_core::cli::{CommandResponse, InitRequest};
use pice_core::config::PiceConfig;
use serde_json::json;
use tracing::info;

use crate::metrics;
use crate::orchestrator::StreamSink;
use crate::server::router::DaemonContext;
use crate::templates::extract_templates;

/// Initialize a project with PICE scaffolding.
///
/// 1. Extracts templates to `.claude/` and `.pice/`
/// 2. Validates the scaffolded config
/// 3. Initializes (or migrates) the metrics database
/// 4. Returns created/skipped file counts
pub async fn run(
    req: InitRequest,
    ctx: &DaemonContext,
    sink: &dyn StreamSink,
) -> Result<CommandResponse> {
    let project_root = ctx.project_root();

    let claude_dir = project_root.join(".claude");
    let pice_dir = project_root.join(".pice");

    if !req.json {
        sink.send_chunk("Scaffolding .claude/ directory...\n");
    }
    let claude_result = extract_templates(&claude_dir, "claude/", req.force)?;

    if !req.json {
        sink.send_chunk("Scaffolding .pice/ directory...\n");
    }
    let pice_result = extract_templates(&pice_dir, "pice/", req.force)?;

    // Verify the scaffolded config is valid
    let config_path = pice_dir.join("config.toml");
    if config_path.exists() {
        match PiceConfig::load(&config_path) {
            Ok(config) => {
                info!(
                    provider = %config.provider.name,
                    eval_model = %config.evaluation.primary.model,
                    "loaded config"
                );
            }
            Err(e) => {
                tracing::warn!("config.toml exists but failed to parse: {e}");
            }
        }
    }

    // Initialize or migrate the metrics database
    let metrics_db_path = metrics::resolve_metrics_db_path(project_root);
    if !metrics_db_path.exists() {
        if let Some(parent) = metrics_db_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        metrics::db::MetricsDb::open(&metrics_db_path)?;
        info!(path = %metrics_db_path.display(), "initialized metrics database");
    } else if req.force {
        // Run migrations on existing DB without destroying data
        metrics::db::MetricsDb::open(&metrics_db_path)?;
        info!(path = %metrics_db_path.display(), "migrated existing metrics database");
    }

    let total_created = claude_result.created.len() + pice_result.created.len();
    let total_skipped = claude_result.skipped.len() + pice_result.skipped.len();

    if req.json {
        let created: Vec<String> = claude_result
            .created
            .iter()
            .map(|f| format!(".claude/{f}"))
            .chain(pice_result.created.iter().map(|f| format!(".pice/{f}")))
            .collect();
        let skipped: Vec<String> = claude_result
            .skipped
            .iter()
            .map(|f| format!(".claude/{f}"))
            .chain(pice_result.skipped.iter().map(|f| format!(".pice/{f}")))
            .collect();
        Ok(CommandResponse::Json {
            value: json!({
                "created": created,
                "skipped": skipped,
                "totalCreated": total_created,
                "totalSkipped": total_skipped,
            }),
        })
    } else {
        let mut output = String::new();
        if total_created > 0 {
            output.push_str(&format!("\nCreated {} files:\n", total_created));
            for f in &claude_result.created {
                output.push_str(&format!("  .claude/{f}\n"));
            }
            for f in &pice_result.created {
                output.push_str(&format!("  .pice/{f}\n"));
            }
        }
        if total_skipped > 0 {
            output.push_str(&format!(
                "Skipped {} existing files (use --force to overwrite)\n",
                total_skipped
            ));
        }
        output.push_str("\nPICE initialized. Run `pice prime` to orient on your codebase.\n");
        Ok(CommandResponse::Text { content: output })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::orchestrator::NullSink;
    use crate::server::router::DaemonContext;

    #[tokio::test]
    async fn init_creates_claude_and_pice_directories() {
        let dir = tempfile::tempdir().unwrap();
        let ctx = DaemonContext::new_for_test_with_root("test-token", dir.path().to_path_buf());
        let req = InitRequest {
            force: false,
            json: false,
        };

        let resp = run(req, &ctx, &NullSink).await.unwrap();
        match &resp {
            CommandResponse::Text { content } => {
                assert!(
                    content.contains("PICE initialized"),
                    "should mention initialization, got: {content}"
                );
            }
            other => panic!("expected Text response, got: {other:?}"),
        }

        assert!(dir.path().join(".claude/commands/plan-feature.md").exists());
        assert!(dir
            .path()
            .join(".claude/templates/plan-template.md")
            .exists());
        assert!(dir.path().join(".claude/docs/PLAYBOOK.md").exists());
        assert!(dir.path().join(".pice/config.toml").exists());
        assert!(dir.path().join(".pice/metrics.db").exists());
    }

    #[tokio::test]
    async fn init_is_idempotent() {
        let dir = tempfile::tempdir().unwrap();
        let ctx = DaemonContext::new_for_test_with_root("test-token", dir.path().to_path_buf());
        let req = InitRequest {
            force: false,
            json: false,
        };

        run(req.clone(), &ctx, &NullSink).await.unwrap();

        // Modify a file
        let plan_path = dir.path().join(".claude/commands/plan-feature.md");
        std::fs::write(&plan_path, "custom content").unwrap();

        // Run again — should not overwrite
        run(req, &ctx, &NullSink).await.unwrap();

        let content = std::fs::read_to_string(&plan_path).unwrap();
        assert_eq!(content, "custom content");
    }

    #[tokio::test]
    async fn init_force_overwrites() {
        let dir = tempfile::tempdir().unwrap();
        let ctx = DaemonContext::new_for_test_with_root("test-token", dir.path().to_path_buf());

        // First init (no force)
        let req = InitRequest {
            force: false,
            json: false,
        };
        run(req, &ctx, &NullSink).await.unwrap();

        // Modify a file
        let plan_path = dir.path().join(".claude/commands/plan-feature.md");
        std::fs::write(&plan_path, "custom content").unwrap();

        // Force init should overwrite
        let req = InitRequest {
            force: true,
            json: false,
        };
        run(req, &ctx, &NullSink).await.unwrap();

        let content = std::fs::read_to_string(&plan_path).unwrap();
        assert_ne!(content, "custom content");
    }

    #[tokio::test]
    async fn init_json_output() {
        let dir = tempfile::tempdir().unwrap();
        let ctx = DaemonContext::new_for_test_with_root("test-token", dir.path().to_path_buf());
        let req = InitRequest {
            force: false,
            json: true,
        };

        let resp = run(req, &ctx, &NullSink).await.unwrap();
        match &resp {
            CommandResponse::Json { value } => {
                assert!(
                    value["totalCreated"].as_u64().unwrap() > 0,
                    "should have created files"
                );
                assert!(
                    value["created"].as_array().unwrap().len() > 0,
                    "created array should not be empty"
                );
            }
            other => panic!("expected Json response in json mode, got: {other:?}"),
        }

        assert!(dir.path().join(".claude/commands/plan-feature.md").exists());
        assert!(dir.path().join(".pice/config.toml").exists());
    }

    #[tokio::test]
    async fn init_second_run_reports_skipped_in_json() {
        let dir = tempfile::tempdir().unwrap();
        let ctx = DaemonContext::new_for_test_with_root("test-token", dir.path().to_path_buf());

        // First run
        let req = InitRequest {
            force: false,
            json: true,
        };
        run(req.clone(), &ctx, &NullSink).await.unwrap();

        // Second run — everything skipped
        let resp = run(req, &ctx, &NullSink).await.unwrap();
        match &resp {
            CommandResponse::Json { value } => {
                assert_eq!(
                    value["totalCreated"].as_u64().unwrap(),
                    0,
                    "second run should create nothing"
                );
                assert!(
                    value["totalSkipped"].as_u64().unwrap() > 0,
                    "second run should skip existing files"
                );
            }
            other => panic!("expected Json response, got: {other:?}"),
        }
    }
}
