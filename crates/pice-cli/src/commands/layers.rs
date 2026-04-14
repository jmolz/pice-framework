//! `pice layers` command — manage layer detection and configuration.

use anyhow::Result;
use clap::{Args, Subcommand};
use pice_core::cli::{CommandRequest, LayersRequest, LayersSubcommand};

#[derive(Args, Debug, Clone)]
pub struct LayersArgs {
    #[command(subcommand)]
    pub subcommand: LayersCommand,

    /// Output as JSON
    #[arg(long, global = true)]
    pub json: bool,
}

#[derive(Subcommand, Debug, Clone)]
pub enum LayersCommand {
    /// Run layer detection and print proposed layers.toml
    Detect {
        /// Write to .pice/layers.toml
        #[arg(long)]
        write: bool,
        /// Overwrite existing layers.toml
        #[arg(long)]
        force: bool,
    },
    /// Show current layer configuration
    List,
    /// Warn about unlayered files
    Check,
    /// Show layer dependency graph
    Graph,
}

impl From<LayersArgs> for LayersRequest {
    fn from(args: LayersArgs) -> Self {
        let subcommand = match args.subcommand {
            LayersCommand::Detect { write, force } => LayersSubcommand::Detect { write, force },
            LayersCommand::List => LayersSubcommand::List,
            LayersCommand::Check => LayersSubcommand::Check,
            LayersCommand::Graph => LayersSubcommand::Graph,
        };
        LayersRequest {
            subcommand,
            json: args.json,
        }
    }
}

pub async fn run(args: &LayersArgs) -> Result<()> {
    let req = CommandRequest::Layers(args.clone().into());
    let resp = crate::adapter::dispatch(req).await?;
    super::render_response(resp)
}
