//! `pice status` handler — show project state and recent evaluations.

use anyhow::Result;
use pice_core::cli::{CommandResponse, StatusRequest};
use pice_core::plan_parser::ParsedPlan;
use serde_json::json;

use crate::metrics;
use crate::orchestrator::StreamSink;
use crate::server::router::DaemonContext;

pub async fn run(
    req: StatusRequest,
    ctx: &DaemonContext,
    _sink: &dyn StreamSink,
) -> Result<CommandResponse> {
    let project_root = ctx.project_root();

    // Scan .claude/plans/ for plan files
    let plans_dir = project_root.join(".claude/plans");
    let mut plans = Vec::new();

    if plans_dir.is_dir() {
        if let Ok(entries) = std::fs::read_dir(&plans_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().and_then(|e| e.to_str()) != Some("md") {
                    continue;
                }

                let plan_info = match ParsedPlan::load(&path) {
                    Ok(plan) => {
                        let normalized = metrics::normalize_plan_path(&plan.path, project_root);
                        // Look up latest evaluation (non-fatal)
                        let eval = metrics::open_metrics_db(project_root)
                            .ok()
                            .flatten()
                            .and_then(|db| {
                                metrics::store::get_latest_evaluation(&db, &normalized)
                                    .ok()
                                    .flatten()
                            });

                        let mut info = json!({
                            "title": plan.title,
                            "path": normalized,
                            "has_contract": plan.contract.is_some(),
                            "tier": plan.tier(),
                        });

                        if let Some(eval) = eval {
                            info["last_eval"] = json!({
                                "passed": eval.passed,
                                "avg_score": eval.avg_score,
                                "timestamp": eval.timestamp,
                            });
                        }

                        info
                    }
                    Err(e) => {
                        // Malformed plans surface with parse_error (per rust-core.md)
                        let name = path
                            .file_name()
                            .map(|n| n.to_string_lossy().to_string())
                            .unwrap_or_default();
                        json!({
                            "title": name,
                            "path": path.to_string_lossy(),
                            "has_contract": false,
                            "parse_error": e.to_string(),
                        })
                    }
                };
                plans.push(plan_info);
            }
        }
    }

    // Git info (non-fatal)
    let git_info = get_git_info(project_root);

    if req.json {
        Ok(CommandResponse::Json {
            value: json!({
                "plans": plans,
                "git": git_info,
            }),
        })
    } else {
        let mut output = String::new();
        output.push_str("PICE Status\n");
        output.push_str("═══════════════════════════════════════\n\n");

        if let Some(branch) = git_info.get("branch").and_then(|b| b.as_str()) {
            output.push_str(&format!("Branch: {branch}\n\n"));
        }

        if plans.is_empty() {
            output.push_str("No plans found.\n");
        } else {
            output.push_str(&format!(
                "{:<30} {:>4}  {:>8}  {:>10}  {:>5}\n",
                "Plan", "Tier", "Contract", "Last Eval", "Score"
            ));
            output.push_str(&format!("{}\n", "─".repeat(70)));

            for plan in &plans {
                let title = plan["title"].as_str().unwrap_or("?");
                let tier = plan.get("tier").and_then(|t| t.as_u64()).unwrap_or(0);
                let contract = if plan["has_contract"].as_bool() == Some(true) {
                    "✓"
                } else {
                    "✗"
                };

                let (eval_str, score_str) = if let Some(eval) = plan.get("last_eval") {
                    let passed = eval["passed"].as_bool().unwrap_or(false);
                    let score = eval["avg_score"].as_f64().unwrap_or(0.0);
                    (
                        if passed { "PASS" } else { "FAIL" }.to_string(),
                        format!("{score:.1}"),
                    )
                } else if plan.get("parse_error").is_some() {
                    ("ERROR".to_string(), "-".to_string())
                } else {
                    ("-".to_string(), "-".to_string())
                };

                // Truncate title to 28 chars
                let display_title = if title.len() > 28 {
                    format!("{}…", &title[..27])
                } else {
                    title.to_string()
                };

                output.push_str(&format!(
                    "{:<30} {:>4}  {:>8}  {:>10}  {:>5}\n",
                    display_title, tier, contract, eval_str, score_str
                ));
            }
        }

        Ok(CommandResponse::Text { content: output })
    }
}

fn get_git_info(project_root: &std::path::Path) -> serde_json::Value {
    let branch = std::process::Command::new("git")
        .args(["branch", "--show-current"])
        .current_dir(project_root)
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string());

    let status = std::process::Command::new("git")
        .args(["status", "--short"])
        .current_dir(project_root)
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| {
            let text = String::from_utf8_lossy(&o.stdout);
            let lines: Vec<&str> = text.lines().collect();
            let staged = lines
                .iter()
                .filter(|l| l.starts_with('M') || l.starts_with('A') || l.starts_with('D'))
                .count();
            let unstaged = lines
                .iter()
                .filter(|l| {
                    l.chars().nth(1).map(|c| c != ' ').unwrap_or(false) && !l.starts_with('?')
                })
                .count();
            let untracked = lines.iter().filter(|l| l.starts_with("??")).count();
            json!({"staged": staged, "unstaged": unstaged, "untracked": untracked})
        })
        .unwrap_or_else(|| json!({}));

    let mut git = json!({});
    if let Some(b) = branch {
        git["branch"] = json!(b);
    }
    git["status"] = status;
    git
}
