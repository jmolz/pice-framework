use anyhow::Result;
use clap::Args;

#[derive(Args, Debug)]
pub struct MetricsArgs {
    /// Output as JSON
    #[arg(long)]
    pub json: bool,

    /// Output as CSV
    #[arg(long)]
    pub csv: bool,
}

pub fn run(args: &MetricsArgs) -> Result<()> {
    let _ = args;
    println!("Not yet implemented -- coming in Phase 4");
    Ok(())
}
