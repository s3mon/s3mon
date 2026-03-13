# s3mon

[![Test & Build](https://github.com/s3mon/s3mon/actions/workflows/build.yml/badge.svg)](https://github.com/s3mon/s3mon/actions/workflows/build.yml)
[![codecov](https://codecov.io/gh/s3mon/s3mon/branch/main/graph/badge.svg)](https://codecov.io/gh/s3mon/s3mon)
[![Crates.io](https://img.shields.io/crates/v/s3mon.svg)](https://crates.io/crates/s3mon)
[![License](https://img.shields.io/crates/l/s3mon.svg)](https://github.com/s3mon/s3mon/blob/master/LICENSE)

`s3mon` checks that expected objects exist in S3 (or any S3-compatible storage) and
prints the results as metrics to **stdout**.

It is designed to be **run on demand** — typically from a cron job — and exits
immediately after checking all configured bucket/prefix pairs.  There is no
long-running process, no open port, and no persistent state: every invocation is
self-contained.

## How it works

For each configured bucket/prefix pair, `s3mon`:

1. Lists all objects under that prefix using `ListObjectsV2`
2. Filters out objects older than the configured `age` (seconds)
3. Optionally checks that at least one object meets a minimum `size` (bytes)
4. Collects the result for every pair, then prints them all at once

All checks run concurrently — one async task per bucket/prefix pair.

## Output formats

`s3mon` supports two output formats selected with the `-f` / `--format` flag.

### Prometheus (default)

```
# HELP s3mon_object_exists Object exists within the configured age window
# TYPE s3mon_object_exists gauge
s3mon_object_exists{bucket="bucket_A",prefix="daily/"} 1
s3mon_object_exists{bucket="bucket_B",prefix="logs/"}  0
# HELP s3mon_check_error S3 API call failed
# TYPE s3mon_check_error gauge
s3mon_check_error{bucket="bucket_A",prefix="daily/"} 0
s3mon_check_error{bucket="bucket_B",prefix="logs/"}  1
# HELP s3mon_size_mismatch Object size is below the configured minimum
# TYPE s3mon_size_mismatch gauge
s3mon_size_mismatch{bucket="bucket_A",prefix="daily/"} 0
s3mon_size_mismatch{bucket="bucket_B",prefix="logs/"}  0
```

### InfluxDB line protocol (`--format influxdb`)

```
s3mon,bucket=bucket_A,prefix=daily/ error=0i,exist=1i,size_mismatch=0i
s3mon,bucket=bucket_B,prefix=logs/  error=1i,exist=0i,size_mismatch=0i
```

### Metric fields

| Metric / Field  | Value `1` means …                                         |
|-----------------|-----------------------------------------------------------|
| `object_exists` | At least one object newer than `age` was found            |
| `check_error`   | S3 API call failed (missing bucket, auth error, etc.)     |
| `size_mismatch` | Found object(s) but all are smaller than `size`           |

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
s3mon -c config.yml [--format prometheus|influxdb]
```

```
Options:
  -c, --config <FILE>         Path to configuration YAML file [required]
  -f, --format <FORMAT>       Output format: prometheus (default) or influxdb
  -v, --verbose               Increase log verbosity (-v INFO, -vv DEBUG, -vvv TRACE)
  -h, --help                  Print help
  -V, --version               Print version
```

Log output goes to **stderr**; metric output goes to **stdout**, so they can be
redirected independently:

```sh
s3mon -c config.yml 2>/var/log/s3mon.log
```

## Configuration

```yaml
# /etc/s3mon.yml
s3mon:
  endpoint: s3.provider.tld   # omit when using AWS (set region instead)
  region: eu-central-1        # AWS region, or any label for custom endpoints
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

If `access_key` and `secret_key` are both set, those static credentials are used.
Otherwise `s3mon` falls back to the standard AWS credential chain:

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

---

## Integrating with monitoring systems

`s3mon` is deliberately a run-and-exit tool.  Wire it into whatever collection
pipeline you already have — no extra processes or ports required.

Two main integration patterns exist.  Choose based on what you already run:

| You have | Best path | Format |
|---|---|---|
| node_exporter on the host | textfile collector | `prometheus` (default) |
| vmagent on the host, no node_exporter | direct push to vmagent | `influxdb` |
| vmagent + node_exporter | either works; textfile is simpler | `prometheus` |

---

### Path A — node_exporter textfile collector

**How it works:**

```
cron
 └─ s3mon --format prometheus → s3mon.prom (file on disk)
                                      ↓
                          node_exporter textfile collector
                                      ↓  (HTTP scrape)
                                  vmagent
                                      ↓  (remote_write)
                              Cortex / VictoriaMetrics
```

node_exporter's textfile collector watches a directory for `*.prom` files and
merges them into its own `/metrics` endpoint on the next scrape.  vmagent
(which you already have scraping node_exporter) picks them up automatically —
**no extra vmagent config required**.

**1. Enable the textfile collector** by pointing node_exporter at a directory:

```sh
node_exporter --collector.textfile.directory=/var/lib/node_exporter/textfile_collector
```

Or in a systemd unit drop-in (`/etc/systemd/system/node_exporter.service.d/textfile.conf`):

```ini
[Service]
ExecStart=
ExecStart=/usr/local/bin/node_exporter \
  --collector.textfile.directory=/var/lib/node_exporter/textfile_collector
```

**2. Write the cron job** (`/etc/cron.d/s3mon`):

```cron
*/5 * * * * root s3mon -c /etc/s3mon.yml \
  > /var/lib/node_exporter/textfile_collector/s3mon.prom.tmp \
  && mv /var/lib/node_exporter/textfile_collector/s3mon.prom.tmp \
        /var/lib/node_exporter/textfile_collector/s3mon.prom
```

The write-to-tmp-then-`mv` pattern is intentional: `rename(2)` is atomic on
POSIX filesystems, so node_exporter never reads a partially-written file.
The `&&` means if `s3mon` fails the old `.prom` is preserved intact.

**3. Verify** — after the first cron run, check node_exporter exposes the metrics:

```sh
curl -s http://localhost:9100/metrics | grep s3mon
```

---

### Path B — push directly to vmagent

**How it works:**

```
cron
 └─ s3mon --format influxdb | curl → vmagent :8429/influx/write
                                            ↓  (remote_write)
                                    Cortex / VictoriaMetrics
```

No node_exporter required.  vmagent accepts InfluxDB line protocol on its
ingestion endpoint and forwards to your configured `remoteWrite` target.
vmagent also buffers writes to disk if the backend is temporarily unavailable.

**Cron job** (`/etc/cron.d/s3mon`):

```cron
*/5 * * * * root s3mon -c /etc/s3mon.yml --format influxdb \
  | curl -sf --max-time 10 \
         -X POST http://localhost:8429/influx/write \
         --data-binary @-
```

`--max-time 10` prevents curl from hanging indefinitely if vmagent is
unreachable.  `-sf` makes curl silent and treats HTTP errors as failures
(so cron can log them).

**Verify** — check vmagent received the data:

```sh
# vmagent self-metrics: look for ingested lines
curl -s http://localhost:8429/metrics | grep influx

# or query directly from VictoriaMetrics / Cortex
curl -s 'http://victoriametrics:8428/api/v1/query?query=s3mon_object_exists'
```

**Adding a label to identify the host** (useful when pushing from multiple machines):

```cron
*/5 * * * * root s3mon -c /etc/s3mon.yml --format influxdb \
  | curl -sf --max-time 10 \
         -X POST "http://localhost:8429/influx/write?extra_label=host=$(hostname -s)" \
         --data-binary @-
```

vmagent's `extra_label` query parameter injects an additional label on every
series it receives — the equivalent of node_exporter's automatic `instance`
label.

---

### Path C — push to VictoriaMetrics single-node (no vmagent)

```cron
*/5 * * * * root s3mon -c /etc/s3mon.yml --format influxdb \
  | curl -sf --max-time 10 \
         -X POST http://victoriametrics:8428/influx/write \
         --data-binary @-
```

---

### Other integrations

**InfluxDB:**

```cron
*/5 * * * * root s3mon -c /etc/s3mon.yml --format influxdb \
  | curl -sf --max-time 10 \
         -X POST "http://influxdb:8086/write?db=monitoring" \
         --data-binary @-
```

**Telegraf exec input:**

```toml
[[inputs.exec]]
  commands = ["/usr/local/bin/s3mon -c /etc/s3mon/config.yml --format influxdb"]
  timeout = "30s"
  data_format = "influx"
```

---

## Grafana dashboard

An example dashboard is provided at
[`contrib/grafana/s3mon-dashboard.json`](contrib/grafana/s3mon-dashboard.json).

Import it via **Dashboards → Import → Upload JSON file** in Grafana.  The
dashboard works with any Prometheus-compatible datasource (Prometheus,
VictoriaMetrics, Thanos, Cortex, Mimir).

It includes:

- **Summary row** — stat panels for total existing objects, missing objects,
  API errors, and size mismatches
- **Status table** — colour-coded per-bucket / per-prefix status at a glance
- **Trends row** — time-series graphs for `object_exists`, `check_error`, and
  `size_mismatch` over the selected time window
- **Variables** — filter by datasource, scrape job, and bucket

---

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
