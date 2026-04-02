mod commands;
mod config;
mod provider;
mod templates;

use clap::{Parser, Subcommand};
use clap_complete::{generate, Shell};

#[derive(Parser, Debug)]
#[command(
    name = "pice",
    version,
    about = "PICE CLI -- structured AI coding workflow orchestrator",
    long_about = "Orchestrate AI coding sessions through the Plan-Implement-Contract-Evaluate methodology.\n\nPICE CLI manages the lifecycle, state, and measurement -- the AI assistant does the coding."
)]
struct Cli {
    /// Enable verbose output
    #[arg(long, global = true)]
    verbose: bool,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Scaffold .claude/ and .pice/ directories with PICE framework files
    Init(commands::init::InitArgs),

    /// Orient on the codebase and get recommended next actions
    Prime(commands::prime::PrimeArgs),

    /// Create a plan with contract for a feature or change
    Plan(commands::plan::PlanArgs),

    /// Implement from a plan file in a fresh session
    Execute(commands::execute::ExecuteArgs),

    /// Run adversarial evaluation against a plan's contract
    Evaluate(commands::evaluate::EvaluateArgs),

    /// Run code review and regression suite
    Review(commands::review::ReviewArgs),

    /// Create a standardized git commit
    Commit(commands::commit::CommitArgs),

    /// Capture session state for the next session
    Handoff(commands::handoff::HandoffArgs),

    /// Display active plans and workflow state
    Status(commands::status::StatusArgs),

    /// Aggregate and display quality metrics
    Metrics(commands::metrics::MetricsArgs),

    /// Before/after workflow effectiveness comparison
    Benchmark(commands::benchmark::BenchmarkArgs),

    /// Generate shell completions
    Completions {
        /// Shell to generate completions for
        shell: Shell,
    },
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    // Set up tracing
    let filter = if cli.verbose { "debug" } else { "info" };
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| filter.into()),
        )
        .with_writer(std::io::stderr)
        .init();

    match &cli.command {
        Commands::Init(args) => commands::init::run(args),
        Commands::Prime(args) => commands::prime::run(args),
        Commands::Plan(args) => commands::plan::run(args),
        Commands::Execute(args) => commands::execute::run(args),
        Commands::Evaluate(args) => commands::evaluate::run(args),
        Commands::Review(args) => commands::review::run(args),
        Commands::Commit(args) => commands::commit::run(args),
        Commands::Handoff(args) => commands::handoff::run(args),
        Commands::Status(args) => commands::status::run(args),
        Commands::Metrics(args) => commands::metrics::run(args),
        Commands::Benchmark(args) => commands::benchmark::run(args),
        Commands::Completions { shell } => {
            let mut cmd = <Cli as clap::CommandFactory>::command();
            generate(*shell, &mut cmd, "pice", &mut std::io::stdout());
            Ok(())
        }
    }
}
