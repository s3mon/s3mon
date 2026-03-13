use crate::cli::actions::Action;
use anyhow::Result;
use clap::ArgMatches;
use std::path::PathBuf;

/// Convert [`ArgMatches`] into a typed [`Action`].
///
/// # Errors
///
/// Returns an error if the config file path is missing or the file cannot be read.
pub fn handler(matches: &ArgMatches) -> Result<Action> {
    let config = matches
        .get_one::<String>("config")
        .ok_or_else(|| anyhow::anyhow!("--config is required"))?;

    let path = PathBuf::from(config);

    let metadata = std::fs::metadata(&path)
        .map_err(|e| anyhow::anyhow!("cannot access config file '{}': {}", path.display(), e))?;

    if !metadata.is_file() {
        anyhow::bail!("'{}' is not a regular file", path.display());
    }

    Ok(Action::Monitor { config: path })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cli::commands;

    #[test]
    fn test_handler_monitor() {
        let matches = commands::new().get_matches_from(vec!["s3mon", "-c", "example.yml"]);
        let action = handler(&matches);
        assert!(action.is_ok());
        if let Ok(Action::Monitor { config }) = action {
            assert_eq!(config, PathBuf::from("example.yml"));
        }
    }

    #[test]
    fn test_handler_invalid_path() {
        let matches =
            commands::new().get_matches_from(vec!["s3mon", "-c", "/nonexistent/path/config.yml"]);
        let result = handler(&matches);
        assert!(result.is_err());
    }
}
