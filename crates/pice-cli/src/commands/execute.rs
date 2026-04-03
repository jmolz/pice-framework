use anyhow::{Context, Result};
use clap::Args;
use pice_protocol::{methods, SessionCreateParams, SessionDestroyParams, SessionSendParams};
use std::path::PathBuf;
use tracing::info;

use crate::config::PiceConfig;
use crate::engine::{orchestrator::ProviderOrchestrator, output, plan_parser, prompt};

#[derive(Args, Debug)]
pub struct ExecuteArgs {
    /// Path to the plan file to execute
    pub plan_path: PathBuf,

    /// Output as JSON
    #[arg(long)]
    pub json: bool,
}

pub async fn run(args: &ExecuteArgs) -> Result<()> {
    let project_root = std::env::current_dir()?;

    // 1. Load and parse the plan file
    let plan = plan_parser::ParsedPlan::load(&args.plan_path)?;

    if !args.json {
        println!("Executing: {}", plan.title);
        println!();
    }

    // 2. Load config
    let config_path = project_root.join(".pice/config.toml");
    let config = PiceConfig::load(&config_path).unwrap_or_else(|_| PiceConfig::default());

    // 3. Build execution prompt
    let exec_prompt = prompt::build_execute_prompt(&plan.content, &project_root)?;

    // 4. Start provider
    info!(provider = %config.provider.name, "starting provider for execution");
    let mut orchestrator = ProviderOrchestrator::start(&config.provider.name, &config).await?;

    // 5. Register notification handler for streaming output
    if !args.json {
        orchestrator.on_notification(Box::new(|method, params| {
            if method == methods::RESPONSE_CHUNK {
                if let Some(params) = params {
                    if let Some(text) = params.get("text").and_then(|t| t.as_str()) {
                        output::print_chunk(text);
                    }
                }
            }
        }));
    }

    // 6. Run session lifecycle — always shutdown provider even on failure
    let session_result = run_session(&mut orchestrator, &project_root, exec_prompt).await;
    if let Err(e) = orchestrator.shutdown().await {
        tracing::warn!("provider shutdown failed: {e}");
    }
    session_result?;

    // 7. Output
    if args.json {
        let output = serde_json::json!({
            "status": "complete",
            "plan": plan.title,
            "planPath": plan.path,
        });
        println!("{}", serde_json::to_string_pretty(&output)?);
    } else {
        println!("\n\nExecution complete.");
    }

    Ok(())
}

async fn run_session(
    orchestrator: &mut ProviderOrchestrator,
    project_root: &std::path::Path,
    exec_prompt: String,
) -> Result<()> {
    let create_params = serde_json::to_value(SessionCreateParams {
        working_directory: project_root.to_string_lossy().to_string(),
        model: None,
        system_prompt: None,
    })?;
    let create_result = orchestrator
        .request(methods::SESSION_CREATE, Some(create_params))
        .await?;
    let session_id = create_result["sessionId"]
        .as_str()
        .context("provider returned session/create without sessionId")?
        .to_string();

    let send_params = serde_json::to_value(SessionSendParams {
        session_id: session_id.clone(),
        message: exec_prompt,
    })?;
    orchestrator
        .request(methods::SESSION_SEND, Some(send_params))
        .await?;

    let destroy_params = serde_json::to_value(SessionDestroyParams {
        session_id: session_id.clone(),
    })?;
    orchestrator
        .request(methods::SESSION_DESTROY, Some(destroy_params))
        .await?;

    Ok(())
}
