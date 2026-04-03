use anyhow::Result;
use clap::Args;

#[derive(Args, Debug)]
pub struct ReviewArgs {}

pub async fn run(args: &ReviewArgs) -> Result<()> {
    let _ = args;
    println!("Not yet implemented -- coming in Phase 3");
    Ok(())
}
