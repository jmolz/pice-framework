use anyhow::Result;
use clap::Args;

#[derive(Args, Debug)]
pub struct StatusArgs {
    /// Output as JSON
    #[arg(long)]
    pub json: bool,
}

pub async fn run(args: &StatusArgs) -> Result<()> {
    let _ = args;
    println!("Not yet implemented -- coming in Phase 3");
    Ok(())
}
