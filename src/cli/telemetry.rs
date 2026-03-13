use anyhow::Result;
use tracing::Level;
use tracing_subscriber::{EnvFilter, Registry, fmt, layer::SubscriberExt};

/// Initialise the tracing subscriber.
///
/// When no verbosity level is supplied the subscriber is configured at ERROR
/// level (effectively silent unless `RUST_LOG` overrides it).
///
/// # Errors
///
/// Returns an error if the global subscriber cannot be set.
pub fn init(verbosity_level: Option<Level>) -> Result<()> {
    let verbosity_level = verbosity_level.unwrap_or(Level::ERROR);

    let fmt_layer = fmt::layer().with_target(false).with_writer(std::io::stderr);

    let filter = EnvFilter::builder()
        .with_default_directive(verbosity_level.into())
        .from_env_lossy();

    let subscriber = Registry::default().with(fmt_layer).with(filter);

    tracing::subscriber::set_global_default(subscriber)
        .map_err(|e| anyhow::anyhow!("failed to set tracing subscriber: {e}"))?;

    Ok(())
}
