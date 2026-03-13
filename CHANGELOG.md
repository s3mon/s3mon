# Changelog

## [0.5.0] - 2026-03-13

- Migrated from `rusoto` to the official [AWS SDK for Rust](https://github.com/awslabs/aws-sdk-rust) (`aws-sdk-s3`, `aws-config`)
- Replaced blocking `.sync()` calls with native `async/await` via `tokio` — resolves [#7](https://github.com/s3mon/s3mon/issues/7)
- Replaced `clap` v2 with `clap` v4 (builder API)
- Replaced `env_logger` with `tracing` + `tracing-subscriber`
- Updated GitHub Actions workflows (format, lint, check, test, coverage, deploy)
- Added RPM and DEB packaging support
