# s3mon

[![Test & Build](https://github.com/s3mon/s3mon/actions/workflows/build.yml/badge.svg)](https://github.com/s3mon/s3mon/actions/workflows/build.yml)
[![codecov](https://codecov.io/gh/s3mon/s3mon/branch/main/graph/badge.svg)](https://codecov.io/gh/s3mon/s3mon)
[![Crates.io](https://img.shields.io/crates/v/s3mon.svg)](https://crates.io/crates/s3mon)
[![License](https://img.shields.io/crates/l/s3mon.svg)](https://github.com/s3mon/s3mon/blob/master/LICENSE)

`s3mon` checks that expected objects exist in S3 (or any S3-compatible storage)
and outputs results in **InfluxDB line protocol**, making it easy to feed into
monitoring pipelines (Telegraf, InfluxDB, Grafana, etc.).

## How it works

For each configured bucket/prefix pair, `s3mon`:

1. Lists all objects under that prefix using `ListObjectsV2`
2. Filters out objects older than the configured `age` (seconds)
3. Optionally checks that at least one object meets a minimum `size` (bytes)
4. Prints one line of InfluxDB line protocol per prefix to stdout

Each output line looks like:

```
s3mon,bucket=<bucket>,prefix=<prefix> error=0i,exist=1i,size_mismatch=0i
```

| Field          | Value | Meaning                                              |
|----------------|-------|------------------------------------------------------|
| `error`        | `1`   | S3 API call failed (bucket missing, auth error, etc) |
| `exist`        | `1`   | At least one object newer than `age` was found       |
| `size_mismatch`| `1`   | Found object(s) but all are smaller than `size`      |

All checks run concurrently — one async task per bucket/prefix pair.

## Installation

```sh
cargo install s3mon
```

Or build from source:

```sh
git clone https://github.com/s3mon/s3mon
cd s3mon
cargo build --release
# binary at target/release/s3mon
```

## Usage

```
s3mon -c config.yml
```

```
Options:
  -c, --config <FILE>   Path to configuration YAML file [required]
  -v, --verbose         Increase log verbosity (-v INFO, -vv DEBUG, -vvv TRACE)
  -h, --help            Print help
  -V, --version         Print version
```

Log output goes to **stderr**; metric output goes to **stdout**, so they can be
redirected independently:

```sh
s3mon -c config.yml > metrics.txt
s3mon -c config.yml 2>s3mon.log
```

## Configuration

```yaml
# config.yml
s3mon:
  endpoint: s3.provider.tld   # omit when using AWS (set region instead)
  region: eu-central-1        # AWS region, or an arbitrary name for custom endpoints
  access_key: ACCESS_KEY_ID   # leave empty to use the AWS default credential chain
  secret_key: SECRET_ACCESS_KEY
  buckets:
    bucket_A:
      - prefix: backups/daily   # S3 key prefix to look for
        age: 86400              # max age in seconds (default: 86400 = 24 h)
        size: 30720             # minimum expected size in bytes (0 = skip check)
    bucket_B:
      - prefix: foo
        age: 43200
        size: 1024
      - prefix: path/to/logs/   # multiple prefixes per bucket are supported
        age: 43200
```

### Fields

| Field        | Required | Default | Description                                              |
|--------------|----------|---------|----------------------------------------------------------|
| `endpoint`   | No       | —       | Custom S3-compatible endpoint (MinIO, DigitalOcean, etc) |
| `region`     | No       | —       | AWS region name, or any label for custom endpoints       |
| `access_key` | No       | —       | Static credentials; falls back to AWS default chain      |
| `secret_key` | No       | —       | Static credentials; falls back to AWS default chain      |
| `prefix`     | **Yes**  | —       | S3 key prefix to search under                            |
| `age`        | No       | `86400` | Maximum age of acceptable objects, in seconds            |
| `size`       | No       | `0`     | Minimum acceptable object size in bytes (`0` = disabled) |

### Credential resolution

If `access_key` and `secret_key` are both set in the config file, those static
credentials are used. Otherwise `s3mon` falls back to the standard AWS
credential chain:

1. `AWS_ACCESS_KEY_ID` / `AWS_SECRET_ACCESS_KEY` environment variables
2. `~/.aws/credentials` file
3. IAM instance profile / ECS task role / etc.

### AWS example

```yaml
s3mon:
  region: us-east-1
  buckets:
    my-backup-bucket:
      - prefix: daily/
        age: 86400
        size: 1024
```

### S3-compatible storage (MinIO, DigitalOcean Spaces, etc.)

```yaml
s3mon:
  endpoint: https://minio.example.com
  region: us-east-1       # required but can be any non-empty string
  access_key: minioadmin
  secret_key: minioadmin
  buckets:
    my-bucket:
      - prefix: backups/
        age: 3600
```

## Integrating with Telegraf

Use the [`exec` input plugin](https://github.com/influxdata/telegraf/tree/master/plugins/inputs/exec)
to collect metrics:

```toml
[[inputs.exec]]
  commands = ["/usr/local/bin/s3mon -c /etc/s3mon/config.yml"]
  timeout = "30s"
  data_format = "influx"
```

## Docker

```sh
docker build -t s3mon .
docker run --rm -v /path/to/config.yml:/config.yml s3mon /s3mon -c /config.yml
```

## Environment variables

| Variable   | Description                                    |
|------------|------------------------------------------------|
| `RUST_LOG` | Override log level (`error`, `warn`, `info`, `debug`, `trace`) |

## Changelog

See [CHANGELOG.md](CHANGELOG.md).
