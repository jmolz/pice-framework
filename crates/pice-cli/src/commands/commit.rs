use anyhow::Result;
use clap::Args;

#[derive(Args, Debug)]
pub struct CommitArgs {}

pub fn run(args: &CommitArgs) -> Result<()> {
    let _ = args;
    println!("Not yet implemented -- coming in Phase 3");
    Ok(())
}
