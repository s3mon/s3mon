pub mod run;

use crate::output::OutputFormat;
use std::path::PathBuf;

/// All possible actions the CLI can perform.
#[derive(Debug)]
pub enum Action {
    /// Monitor S3 buckets using the given configuration file.
    Monitor {
        config: PathBuf,
        format: OutputFormat,
        exit_on_check_failure: bool,
    },
}
