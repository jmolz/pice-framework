use anyhow::Result;
use clap::Args;
use pice_core::cli::{CommandRequest, InitRequest};

#[derive(Args, Debug, Clone)]
pub struct InitArgs {
    /// Overwrite existing files instead of skipping them
    #[arg(long)]
    pub force: bool,

    /// Output as JSON
    #[arg(long)]
    pub json: bool,
}

impl From<InitArgs> for InitRequest {
    fn from(args: InitArgs) -> Self {
        InitRequest {
            force: args.force,
            json: args.json,
        }
    }
}

pub async fn run(args: &InitArgs) -> Result<()> {
    let req = CommandRequest::Init(args.clone().into());
    let resp = crate::adapter::dispatch(req).await?;
    super::render_response(resp)
}
