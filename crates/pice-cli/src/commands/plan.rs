use anyhow::{Context, Result};
use clap::Args;
use pice_protocol::{methods, SessionCreateParams, SessionDestroyParams, SessionSendParams};
use tracing::info;

use crate::config::PiceConfig;
use crate::engine::{orchestrator::ProviderOrchestrator, output, prompt};

#[derive(Args, Debug)]
pub struct PlanArgs {
    /// Description of what to plan
    pub description: String,

    /// Output as JSON
    #[arg(long)]
    pub json: bool,
}

pub async fn run(args: &PlanArgs) -> Result<()> {
    let project_root = std::env::current_dir()?;

    // 1. Load config
    let config_path = project_root.join(".pice/config.toml");
    let config = PiceConfig::load(&config_path).unwrap_or_else(|_| PiceConfig::default());

    // 2. Build planning prompt
    let plan_prompt = prompt::build_plan_prompt(&args.description, &project_root)?;

    if !args.json {
        println!("Planning: {}", args.description);
        println!();
    }

    // 3. Start provider
    info!(provider = %config.provider.name, "starting provider for planning");
    let mut orchestrator = ProviderOrchestrator::start(&config.provider.name, &config).await?;

    // 4. Register notification handler for streaming output
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

    // 5. Run session lifecycle — always shutdown provider even on failure
    let session_result = run_session(&mut orchestrator, &project_root, plan_prompt).await;
    if let Err(e) = orchestrator.shutdown().await {
        tracing::warn!("provider shutdown failed: {e}");
    }
    session_result?;

    // 6. Output
    if args.json {
        let output = serde_json::json!({
            "status": "complete",
            "description": args.description,
        });
        println!("{}", serde_json::to_string_pretty(&output)?);
    } else {
        println!("\n\nPlanning session complete.");
    }

    Ok(())
}

async fn run_session(
    orchestrator: &mut ProviderOrchestrator,
    project_root: &std::path::Path,
    plan_prompt: String,
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
        message: plan_prompt,
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
