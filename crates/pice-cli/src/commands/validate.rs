//! `pice validate` command — validate `.pice/` configuration.

use anyhow::Result;
use clap::Args;
use pice_core::cli::{CommandRequest, ValidateRequest};

#[derive(Args, Debug, Clone)]
pub struct ValidateArgs {
    /// Output as JSON (machine-readable `ValidationReport`)
    #[arg(long)]
    pub json: bool,

    /// Also query the provider for its supported model list and validate
    /// `model` names in workflow.yaml against it.
    #[arg(long)]
    pub check_models: bool,
}

impl From<ValidateArgs> for ValidateRequest {
    fn from(args: ValidateArgs) -> Self {
        Self {
            json: args.json,
            check_models: args.check_models,
        }
    }
}

pub async fn run(args: &ValidateArgs) -> Result<()> {
    let req = CommandRequest::Validate(args.clone().into());
    let resp = crate::adapter::dispatch(req).await?;
    super::render_response(resp)
}
