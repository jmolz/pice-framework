use anyhow::Result;
use clap::Args;

#[derive(Args, Debug)]
pub struct PlanArgs {
    /// Description of what to plan
    pub description: String,
}

pub fn run(args: &PlanArgs) -> Result<()> {
    let _ = args;
    println!("Not yet implemented -- coming in Phase 2");
    Ok(())
}
