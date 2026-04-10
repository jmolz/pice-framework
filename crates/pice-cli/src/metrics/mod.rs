pub mod aggregator;
pub mod db;
pub mod store;
pub mod telemetry;

use anyhow::Result;
use std::path::Path;

use pice_core::config::PiceConfig;

// Re-export the path normalization helper from pice-core so existing callers
// (commands/execute.rs, commands/evaluate.rs) can keep using
// `metrics::normalize_plan_path(..)` unchanged. The implementation lives in
// `pice_core::paths` and is shared with pice-daemon.
pub use pice_core::paths::normalize_plan_path;

/// Open the metrics database for a project.
/// Returns None if the DB file doesn't exist (project not initialized).
pub fn open_metrics_db(project_root: &Path) -> Result<Option<db::MetricsDb>> {
    let config_path = project_root.join(".pice/config.toml");
    let config = PiceConfig::load(&config_path).unwrap_or_else(|_| PiceConfig::default());
    let db_path = project_root.join(&config.metrics.db_path);
    if !db_path.exists() {
        return Ok(None);
    }
    Ok(Some(db::MetricsDb::open(&db_path)?))
}

/// Resolve the configured metrics DB path for a project (for init).
pub fn resolve_metrics_db_path(project_root: &Path) -> std::path::PathBuf {
    let config_path = project_root.join(".pice/config.toml");
    let config = PiceConfig::load(&config_path).unwrap_or_else(|_| PiceConfig::default());
    project_root.join(&config.metrics.db_path)
}
