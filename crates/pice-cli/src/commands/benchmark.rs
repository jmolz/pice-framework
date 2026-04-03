use anyhow::Result;
use clap::Args;

#[derive(Args, Debug)]
pub struct BenchmarkArgs {
    /// Output as JSON
    #[arg(long)]
    pub json: bool,
}

pub async fn run(args: &BenchmarkArgs) -> Result<()> {
    let _ = args;
    println!("Not yet implemented -- coming in Phase 4");
    Ok(())
}
