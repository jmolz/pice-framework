//! Tracing setup for the daemon.
//!
//! T11 stub: minimal `init()` that configures `tracing_subscriber` to write
//! to stderr (same as pice-cli for now). T21 replaces with
//! `tracing_appender::rolling::daily("~/.pice/logs", "daemon.log")` + a
//! non-blocking writer so daemon logs flush asynchronously and rotate daily.
//!
//! The CLI side's tracing already writes to stderr; the daemon must NOT
//! write to stderr/stdout once it runs detached (no terminal attached).
//! T21 switches to the file appender before any detached spawn.

/// Initialize the daemon's tracing subscriber.
///
/// T11 stub: stderr writer. T21 replaces with the rolling file appender.
pub fn init() -> anyhow::Result<()> {
    // tracing_subscriber::fmt::try_init returns Err if a subscriber is already
    // set. In inline mode the CLI will have already initialized tracing, so we
    // treat "already set" as success rather than a hard error.
    let _ = tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .with_writer(std::io::stderr)
        .try_init();
    Ok(())
}
