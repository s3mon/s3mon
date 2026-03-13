# Changelog

## [0.5.0] - 2026-03-13

- Migrated from `rusoto` to the official [AWS SDK for Rust](https://github.com/awslabs/aws-sdk-rust) (`aws-sdk-s3`, `aws-config`).
- Replaced blocking `.sync()` calls with native `async/await` via `tokio` — resolves [#7](https://github.com/s3mon/s3mon/issues/7).
- Replaced `clap` v2 with `clap` v4 (builder API).
- Replaced `env_logger` with `tracing` + `tracing-subscriber`.
- Added S3 pagination to correctly handle buckets/prefixes with more than 1000 objects.
- Fixed `size_mismatch` logic: it now correctly reports a mismatch only if *all* objects in the age window are below the minimum size.
- Optimized memory usage by processing S3 objects as a stream instead of collecting them into a list.
- Improved error reporting by including bucket and prefix context in log messages.
- Updated GitHub Actions workflows (format, lint, check, test, coverage, deploy) and opted into Node.js 24 for the coverage workflow.
- Enabled integration tests by default when running `cargo test`.
- Added RPM and DEB packaging support.
- Removed legacy CI configuration files (`.travis.yml`, `.circleci`, `.cirrus.yml`).

