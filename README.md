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
s3mon -c config.yml [--format prometheus|influxdb] [--exit-on-check-failure]
```

```
Options:
  -c, --config <FILE>         Path to configuration YAML file [required]
  -f, --format <FORMAT>       Output format: prometheus (default) or influxdb
      --exit-on-check-failure Exit with status 1 if any check is missing, errors, or size-mismatched
  -v, --verbose               Increase log verbosity (-v INFO, -vv DEBUG, -vvv TRACE)
  -h, --help                  Print help
  -V, --version               Print version
```

Log output goes to **stderr**; metric output goes to **stdout**, so they can be
redirected independently:

```sh
s3mon -c config.yml 2>/var/log/s3mon.log
```

If you want cron or systemd timers to alert on missing objects, S3 API errors,
or size mismatches, add `--exit-on-check-failure`.  `s3mon` will still print
the metrics first, then exit with status `1`.

## Configuration

```yaml
# /etc/s3mon.yml
s3mon:
  endpoint: https://s3.provider.tld # full URL; omit when using AWS
  region: eu-central-1              # set for AWS and custom endpoints
  access_key: ACCESS_KEY_ID   # leave empty to use the AWS default credential chain
  secret_key: SECRET_ACCESS_KEY
  buckets:
    bucket_A:
      - prefix: backups/daily   # S3 key prefix to look for
        suffix: .log            # optional key suffix filter, matched client-side
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
| `endpoint`   | No       | —       | Full custom S3-compatible endpoint URL, including scheme |
| `region`     | No       | —       | Region/signing label; set it for AWS and custom endpoints |
| `access_key` | No       | —       | Static credentials; falls back to AWS default chain      |
| `secret_key` | No       | —       | Static credentials; falls back to AWS default chain      |
| `prefix`     | **Yes**  | —       | S3 key prefix to search under                            |
| `suffix`     | No       | `""`    | Optional key suffix to match after the prefix listing    |
| `age`        | No       | `86400` | Maximum age of acceptable objects, in seconds            |
| `size`       | No       | `0`     | Minimum acceptable object size in bytes (`0` = disabled) |

For S3-compatible vendors, `endpoint` should include the scheme, for example
`https://minio.example.com`. `region` is still needed as a non-empty value for
request signing; many vendors accept any label such as `us-east-1` or `CH`.
`suffix` is applied client-side after the S3 `prefix` listing, so the most
efficient setup is still to choose the narrowest useful prefix.

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
  region: us-east-1       # required, but many vendors accept any non-empty string
  access_key: minioadmin
  secret_key: minioadmin
  buckets:
    my-bucket:
      - prefix: postgresql-
        suffix: .log
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
| vmagent on the host, no node_exporter | direct push to vmagent | `prometheus` or `influxdb` |
| vmagent + node_exporter | either works; textfile is simpler | `prometheus` |

---

### Path A — node_exporter textfile collector

This is the recommended way to get `s3mon` metrics into Prometheus.
Prometheus is pull-based: `s3mon` writes a `.prom` file, `node_exporter`
serves it on `/metrics`, and Prometheus scrapes `node_exporter`.  You do not
POST Prometheus text format directly to the Prometheus server.

**How it works:**

```
cron
 └─ s3mon --format prometheus → s3mon.prom (file on disk)
                                      ↓
                          node_exporter textfile collector
                                      ↓  (HTTP scrape)
                          Prometheus / vmagent scraper
                                      ↓
                        Prometheus TSDB / remote_write
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

**4. Add the Prometheus scrape job** (`prometheus.yml`):

```yaml
scrape_configs:
  - job_name: node
    static_configs:
      - targets:
          - localhost:9100
```

If Prometheus is already scraping `node_exporter`, `s3mon` metrics appear
automatically once the `.prom` file exists.  No special `s3mon` scrape job is
needed because the metrics are exposed as part of the normal `node_exporter`
target.

**5. Verify in Prometheus**:

```promql
s3mon_object_exists
```

You should see one series per configured `(bucket, prefix)` pair, with labels
such as `bucket`, `prefix`, `instance`, and `job`.

---

### Path B — push directly to vmagent in Prometheus format

**How it works:**

```
cron
 └─ s3mon --format prometheus | curl → vmagent :8429/api/v1/import/prometheus
                                              ↓  (remote_write)
                                      Cortex / VictoriaMetrics
```

This keeps the default `prometheus` output format and pushes it straight to
`vmagent`, which accepts Prometheus exposition text on its import endpoint.
Use this when you want an HTTP push flow but do not want to switch `s3mon` to
InfluxDB line protocol.

**Cron job** (`/etc/cron.d/s3mon`):

```cron
*/5 * * * * root s3mon -c /etc/s3mon.yml \
  | curl -sf --max-time 10 \
         -X POST http://localhost:8429/api/v1/import/prometheus \
         --data-binary @-
```

`--data-binary @-` preserves the Prometheus payload exactly as written by
`s3mon`.  `-sf` makes curl silent and treats HTTP errors as failures, while
`--max-time 10` prevents cron jobs from hanging indefinitely.

**Email alerts for both vmagent and check failures**

If you want cron mail when either:

- `vmagent` is unavailable, or
- `s3mon` finds a missing object / S3 error / size mismatch,

do not rely on a plain pipeline by itself, because the shell usually returns
the exit status from `curl`, not from `s3mon`.  Use a wrapper script instead.
This example is copy-pasteable and sends an HTML email with the relevant logs:

```sh
#!/bin/sh
set -u

ERROR_EMAIL="ops@example.com"
CONFIG_FILE="/etc/s3mon.yml"
VMAGENT_URL="http://localhost:8429/api/v1/import/prometheus"
SENDMAIL_BIN="/usr/sbin/sendmail"
HOSTNAME="$(hostname -f 2>/dev/null || hostname)"

metrics_file="$(mktemp)"
s3mon_stderr="$(mktemp)"
curl_stderr="$(mktemp)"

cleanup() {
  rm -f "$metrics_file" "$s3mon_stderr" "$curl_stderr"
}
trap cleanup EXIT

html_escape() {
  sed \
    -e 's/&/\&amp;/g' \
    -e 's/</\&lt;/g' \
    -e 's/>/\&gt;/g'
}

s3mon_status=0
curl_status=0

s3mon -c "$CONFIG_FILE" --exit-on-check-failure \
  >"$metrics_file" \
  2>"$s3mon_stderr" || s3mon_status=$?

if [ -s "$metrics_file" ]; then
  curl -sf --max-time 10 \
    -X POST "$VMAGENT_URL" \
    --data-binary @"$metrics_file" \
    2>"$curl_stderr" || curl_status=$?
fi

if [ "$s3mon_status" -ne 0 ] || [ "$curl_status" -ne 0 ]; then
  subject="ALERT: s3mon failure on $HOSTNAME"
  email_headers=$(
    cat <<EOF
To: $ERROR_EMAIL
Subject: $subject
Mime-Version: 1.0
Content-Type: text/html; charset=utf-8

<html><head><style>
body { font-family: sans-serif; }
pre { font-family: monospace; white-space: pre-wrap; margin: 0; }
</style></head><body>
EOF
  )

  {
    printf '%s\n' "$email_headers"
    printf '<h2>%s</h2>\n' "$subject"
    printf '<p><strong>Host:</strong> %s</p>\n' "$HOSTNAME"
    printf '<p><strong>Config:</strong> %s</p>\n' "$CONFIG_FILE"
    printf '<p><strong>s3mon exit code:</strong> %s<br>\n' "$s3mon_status"
    printf '<strong>curl exit code:</strong> %s</p>\n' "$curl_status"

    printf '<h3>s3mon stderr</h3><pre>'
    if [ -s "$s3mon_stderr" ]; then
      html_escape <"$s3mon_stderr"
    else
      printf 'no stderr output'
    fi
    printf '</pre>\n'

    printf '<h3>curl stderr</h3><pre>'
    if [ -s "$curl_stderr" ]; then
      html_escape <"$curl_stderr"
    else
      printf 'no stderr output'
    fi
    printf '</pre>\n'

    printf '<h3>metrics payload</h3><pre>'
    if [ -s "$metrics_file" ]; then
      html_escape <"$metrics_file"
    else
      printf 'no metrics were produced'
    fi
    printf '</pre>\n'

    printf '</body></html>\n'
  } | "$SENDMAIL_BIN" -t

  exit 1
fi
```

Example cron entry:

```cron
*/5 * * * * root /usr/local/bin/s3mon-vmagent-alert.sh
```

The script returns `1` if either the push fails or any `s3mon` check failed,
and sends one HTML email with the `s3mon` stderr, `curl` stderr, and the
generated metrics payload.

**Verify** — check vmagent received the data:

```sh
curl -s http://localhost:8429/metrics | grep vmagent_http_requests_total
curl -s 'http://victoriametrics:8428/api/v1/query?query=s3mon_object_exists'
```

---

### Path C — push directly to vmagent in InfluxDB format

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

### Path D — push to VictoriaMetrics single-node (no vmagent)

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
