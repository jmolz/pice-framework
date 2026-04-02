use anyhow::Result;
use clap::Args;
use std::path::PathBuf;

#[derive(Args, Debug)]
pub struct ExecuteArgs {
    /// Path to the plan file to execute
    pub plan_path: PathBuf,
}

pub fn run(args: &ExecuteArgs) -> Result<()> {
    let _ = args;
    println!("Not yet implemented -- coming in Phase 2");
    Ok(())
}
