use anyhow::Result;
use clap::Args;
use std::path::PathBuf;

#[derive(Args, Debug)]
pub struct EvaluateArgs {
    /// Path to the plan file to evaluate against
    pub plan_path: PathBuf,
}

pub fn run(args: &EvaluateArgs) -> Result<()> {
    let _ = args;
    println!("Not yet implemented -- coming in Phase 2");
    Ok(())
}
