use clap::{
    Arg, ArgAction, ColorChoice, Command,
    builder::styling::{AnsiColor, Effects, Styles},
};

pub mod built_info {
    #![allow(clippy::doc_markdown)]
    include!(concat!(env!("OUT_DIR"), "/built.rs"));
}

/// Build the CLI command structure using the clap builder API.
#[must_use]
pub fn new() -> Command {
    let styles = Styles::styled()
        .header(AnsiColor::Yellow.on_default() | Effects::BOLD)
        .usage(AnsiColor::Green.on_default() | Effects::BOLD)
        .literal(AnsiColor::Blue.on_default() | Effects::BOLD)
        .placeholder(AnsiColor::Green.on_default());

    let git_hash = built_info::GIT_COMMIT_HASH.unwrap_or("unknown");
    let long_version: &'static str =
        Box::leak(format!("{} - {}", env!("CARGO_PKG_VERSION"), git_hash).into_boxed_str());

    Command::new(env!("CARGO_PKG_NAME"))
        .version(env!("CARGO_PKG_VERSION"))
        .long_version(long_version)
        .author(env!("CARGO_PKG_AUTHORS"))
        .about(env!("CARGO_PKG_DESCRIPTION"))
        .color(ColorChoice::Auto)
        .styles(styles)
        .arg(
            Arg::new("config")
                .short('c')
                .long("config")
                .value_name("FILE")
                .help("Path to configuration YAML file")
                .required(true),
        )
        .arg(
            Arg::new("verbose")
                .short('v')
                .long("verbose")
                .help("Increase log verbosity (-v INFO, -vv DEBUG, -vvv TRACE)")
                .action(ArgAction::Count),
        )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cli_structure() {
        let cmd = new();
        assert_eq!(cmd.get_name(), env!("CARGO_PKG_NAME"));
    }

    #[test]
    fn test_parse_config_flag() {
        let matches = new().get_matches_from(vec!["s3mon", "-c", "example.yml"]);
        assert_eq!(
            matches.get_one::<String>("config").map(String::as_str),
            Some("example.yml")
        );
    }

    #[test]
    fn test_verbose_count() {
        let matches = new().get_matches_from(vec!["s3mon", "-vv", "-c", "example.yml"]);
        assert_eq!(matches.get_count("verbose"), 2);
    }

    #[test]
    fn test_long_version_includes_git_hash() {
        let cmd = new();
        let long_version = cmd.get_long_version().unwrap_or("").to_string();
        assert!(long_version.contains(env!("CARGO_PKG_VERSION")));
        assert!(long_version.contains(" - "));
    }
}
