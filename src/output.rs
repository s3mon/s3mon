/// The output format used when printing metrics.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum OutputFormat {
    /// Prometheus text exposition format (default).
    #[default]
    Prometheus,
    /// `InfluxDB` line protocol.
    Influxdb,
}

/// Result of a single (bucket, prefix) monitoring check.
#[derive(Debug)]
pub struct CheckResult {
    pub bucket: String,
    pub prefix: String,
    pub suffix: String,
    pub exist: bool,
    pub error: bool,
    pub size_mismatch: bool,
}

/// Escape a string for use as a Prometheus label value.
/// Escapes `\`, `"`, and newlines as required by the exposition format spec.
fn escape_label(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
}

/// Escape a string for use as an `InfluxDB` line-protocol tag value.
/// Escapes commas, equals signs, and spaces.
fn escape_tag(s: &str) -> String {
    s.replace(',', "\\,")
        .replace('=', "\\=")
        .replace(' ', "\\ ")
}

use std::fmt::Write as _;

fn prometheus_labels(r: &CheckResult) -> String {
    if r.suffix.is_empty() {
        format!(
            "bucket=\"{}\",prefix=\"{}\"",
            escape_label(&r.bucket),
            escape_label(&r.prefix),
        )
    } else {
        format!(
            "bucket=\"{}\",prefix=\"{}\",suffix=\"{}\"",
            escape_label(&r.bucket),
            escape_label(&r.prefix),
            escape_label(&r.suffix),
        )
    }
}

fn influx_tags(r: &CheckResult) -> String {
    if r.suffix.is_empty() {
        format!(
            "bucket={},prefix={}",
            escape_tag(&r.bucket),
            escape_tag(&r.prefix),
        )
    } else {
        format!(
            "bucket={},prefix={},suffix={}",
            escape_tag(&r.bucket),
            escape_tag(&r.prefix),
            escape_tag(&r.suffix),
        )
    }
}

/// Format results as Prometheus text exposition format.
///
/// All series for a metric family are grouped under a single `# HELP` / `# TYPE`
/// header, as required by the Prometheus specification.
/// Results are sorted by (bucket, prefix) for deterministic output.
#[must_use]
pub fn format_prometheus(results: &[CheckResult]) -> String {
    let mut sorted: Vec<&CheckResult> = results.iter().collect();
    sorted.sort_by(|a, b| {
        a.bucket
            .cmp(&b.bucket)
            .then(a.prefix.cmp(&b.prefix))
            .then(a.suffix.cmp(&b.suffix))
    });

    let mut out = String::new();

    out.push_str("# HELP s3mon_object_exists Object exists within the configured age window\n");
    out.push_str("# TYPE s3mon_object_exists gauge\n");
    for r in &sorted {
        let _ = writeln!(
            out,
            "s3mon_object_exists{{{}}} {}",
            prometheus_labels(r),
            i32::from(r.exist),
        );
    }

    out.push_str("# HELP s3mon_check_error S3 API call failed\n");
    out.push_str("# TYPE s3mon_check_error gauge\n");
    for r in &sorted {
        let _ = writeln!(
            out,
            "s3mon_check_error{{{}}} {}",
            prometheus_labels(r),
            i32::from(r.error),
        );
    }

    out.push_str("# HELP s3mon_size_mismatch Object size is below the configured minimum\n");
    out.push_str("# TYPE s3mon_size_mismatch gauge\n");
    for r in &sorted {
        let _ = writeln!(
            out,
            "s3mon_size_mismatch{{{}}} {}",
            prometheus_labels(r),
            i32::from(r.size_mismatch),
        );
    }

    out
}

/// Format results as `InfluxDB` line protocol.
///
/// Each (bucket, prefix) pair produces one line with three integer fields:
/// `error`, `exist`, and `size_mismatch`.
/// Results are sorted by (bucket, prefix) for deterministic output.
#[must_use]
pub fn format_influxdb(results: &[CheckResult]) -> String {
    let mut sorted: Vec<&CheckResult> = results.iter().collect();
    sorted.sort_by(|a, b| {
        a.bucket
            .cmp(&b.bucket)
            .then(a.prefix.cmp(&b.prefix))
            .then(a.suffix.cmp(&b.suffix))
    });

    let mut lines: Vec<String> = sorted
        .iter()
        .map(|r| {
            format!(
                "s3mon,{} error={}i,exist={}i,size_mismatch={}i",
                influx_tags(r),
                i32::from(r.error),
                i32::from(r.exist),
                i32::from(r.size_mismatch),
            )
        })
        .collect();
    lines.push(String::new()); // trailing newline
    lines.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn results() -> Vec<CheckResult> {
        vec![
            CheckResult {
                bucket: "bucket_B".to_string(),
                prefix: "foo/".to_string(),
                suffix: String::new(),
                exist: false,
                error: true,
                size_mismatch: false,
            },
            CheckResult {
                bucket: "bucket_A".to_string(),
                prefix: "test/".to_string(),
                suffix: String::new(),
                exist: true,
                error: false,
                size_mismatch: false,
            },
        ]
    }

    #[test]
    fn test_prometheus_sorted_and_grouped() {
        let out = format_prometheus(&results());
        // bucket_A should appear before bucket_B after sorting
        let lines: Vec<&str> = out.lines().collect();
        let exists_a = lines
            .iter()
            .find(|l| l.contains("s3mon_object_exists") && l.contains("bucket_A"))
            .copied();
        let exists_b = lines
            .iter()
            .find(|l| l.contains("s3mon_object_exists") && l.contains("bucket_B"))
            .copied();
        assert_eq!(
            exists_a,
            Some(r#"s3mon_object_exists{bucket="bucket_A",prefix="test/"} 1"#)
        );
        assert_eq!(
            exists_b,
            Some(r#"s3mon_object_exists{bucket="bucket_B",prefix="foo/"} 0"#)
        );
    }

    #[test]
    fn test_prometheus_has_help_and_type_headers() {
        let out = format_prometheus(&results());
        assert!(out.contains("# HELP s3mon_object_exists"));
        assert!(out.contains("# TYPE s3mon_object_exists gauge"));
        assert!(out.contains("# HELP s3mon_check_error"));
        assert!(out.contains("# TYPE s3mon_check_error gauge"));
        assert!(out.contains("# HELP s3mon_size_mismatch"));
        assert!(out.contains("# TYPE s3mon_size_mismatch gauge"));
    }

    #[test]
    fn test_influxdb_format() {
        let out = format_influxdb(&results());
        // After sort: bucket_A first
        let mut lines = out.lines();
        assert_eq!(
            lines.next(),
            Some("s3mon,bucket=bucket_A,prefix=test/ error=0i,exist=1i,size_mismatch=0i")
        );
        assert_eq!(
            lines.next(),
            Some("s3mon,bucket=bucket_B,prefix=foo/ error=1i,exist=0i,size_mismatch=0i")
        );
    }

    #[test]
    fn test_escape_label_special_chars() {
        let r = vec![CheckResult {
            bucket: r#"buck"et"#.to_string(),
            prefix: "pre\\fix".to_string(),
            suffix: ".log".to_string(),
            exist: true,
            error: false,
            size_mismatch: false,
        }];
        let out = format_prometheus(&r);
        assert!(out.contains(r#"bucket="buck\"et""#));
        assert!(out.contains(r#"prefix="pre\\fix""#));
        assert!(out.contains(r#"suffix=".log""#));
    }

    #[test]
    fn test_sorting_uses_suffix_when_bucket_and_prefix_match() {
        let results = vec![
            CheckResult {
                bucket: "bucket".to_string(),
                prefix: "logs/".to_string(),
                suffix: ".zst".to_string(),
                exist: true,
                error: false,
                size_mismatch: false,
            },
            CheckResult {
                bucket: "bucket".to_string(),
                prefix: "logs/".to_string(),
                suffix: ".log".to_string(),
                exist: true,
                error: false,
                size_mismatch: false,
            },
        ];

        let out = format_prometheus(&results);
        let lines: Vec<&str> = out
            .lines()
            .filter(|line| line.starts_with("s3mon_object_exists"))
            .collect();

        assert_eq!(
            lines,
            vec![
                r#"s3mon_object_exists{bucket="bucket",prefix="logs/",suffix=".log"} 1"#,
                r#"s3mon_object_exists{bucket="bucket",prefix="logs/",suffix=".zst"} 1"#,
            ]
        );
    }

    #[test]
    fn test_empty_results() {
        assert_eq!(
            format_prometheus(&[]),
            "# HELP s3mon_object_exists Object exists within the configured age window\n\
             # TYPE s3mon_object_exists gauge\n\
             # HELP s3mon_check_error S3 API call failed\n\
             # TYPE s3mon_check_error gauge\n\
             # HELP s3mon_size_mismatch Object size is below the configured minimum\n\
             # TYPE s3mon_size_mismatch gauge\n"
        );
        assert_eq!(format_influxdb(&[]), "");
    }

    #[test]
    fn test_both_formats_encode_identical_values() {
        // The same logical data must be represented consistently in both formats.
        let results = vec![
            CheckResult {
                bucket: "bucket_A".to_string(),
                prefix: "daily/".to_string(),
                suffix: String::new(),
                exist: true,
                error: false,
                size_mismatch: false,
            },
            CheckResult {
                bucket: "bucket_B".to_string(),
                prefix: "logs/".to_string(),
                suffix: String::new(),
                exist: false,
                error: true,
                size_mismatch: false,
            },
            CheckResult {
                bucket: "bucket_C".to_string(),
                prefix: "data/".to_string(),
                suffix: ".log".to_string(),
                exist: true,
                error: false,
                size_mismatch: true,
            },
        ];

        let prom = format_prometheus(&results);
        let influx = format_influxdb(&results);

        // bucket_A: exist=1, error=0, size_mismatch=0
        assert!(prom.contains(r#"s3mon_object_exists{bucket="bucket_A",prefix="daily/"} 1"#));
        assert!(prom.contains(r#"s3mon_check_error{bucket="bucket_A",prefix="daily/"} 0"#));
        assert!(prom.contains(r#"s3mon_size_mismatch{bucket="bucket_A",prefix="daily/"} 0"#));
        assert!(
            influx
                .contains("s3mon,bucket=bucket_A,prefix=daily/ error=0i,exist=1i,size_mismatch=0i")
        );

        // bucket_B: exist=0, error=1, size_mismatch=0
        assert!(prom.contains(r#"s3mon_object_exists{bucket="bucket_B",prefix="logs/"} 0"#));
        assert!(prom.contains(r#"s3mon_check_error{bucket="bucket_B",prefix="logs/"} 1"#));
        assert!(prom.contains(r#"s3mon_size_mismatch{bucket="bucket_B",prefix="logs/"} 0"#));
        assert!(
            influx
                .contains("s3mon,bucket=bucket_B,prefix=logs/ error=1i,exist=0i,size_mismatch=0i")
        );

        // bucket_C: exist=1, error=0, size_mismatch=1
        assert!(
            prom.contains(
                r#"s3mon_object_exists{bucket="bucket_C",prefix="data/",suffix=".log"} 1"#
            )
        );
        assert!(
            prom.contains(r#"s3mon_check_error{bucket="bucket_C",prefix="data/",suffix=".log"} 0"#)
        );
        assert!(
            prom.contains(
                r#"s3mon_size_mismatch{bucket="bucket_C",prefix="data/",suffix=".log"} 1"#
            )
        );
        assert!(influx.contains(
            "s3mon,bucket=bucket_C,prefix=data/,suffix=.log error=0i,exist=1i,size_mismatch=1i"
        ));
    }
}
