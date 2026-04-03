use anyhow::Result;
use clap::Args;

#[derive(Args, Debug)]
pub struct PrimeArgs {}

pub async fn run(args: &PrimeArgs) -> Result<()> {
    let _ = args;
    println!("Not yet implemented -- coming in Phase 3");
    Ok(())
}
