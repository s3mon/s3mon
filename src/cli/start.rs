use crate::cli::{actions::Action, commands, dispatch, telemetry};
use anyhow::Result;

const fn get_verbosity_level(verbose_count: u8) -> Option<tracing::Level> {
    match verbose_count {
        0 => None,
        1 => Some(tracing::Level::INFO),
        2 => Some(tracing::Level::DEBUG),
        _ => Some(tracing::Level::TRACE),
    }
}

/// Main entry point for the CLI — parses arguments and returns the resolved Action.
///
/// # Errors
///
/// Returns an error if argument parsing, telemetry init, or dispatch fails.
pub fn start() -> Result<Action> {
    let matches = commands::new().get_matches();
    let verbosity_level = get_verbosity_level(matches.get_count("verbose"));
    telemetry::init(verbosity_level)?;
    let action = dispatch::handler(&matches)?;
    Ok(action)
}
