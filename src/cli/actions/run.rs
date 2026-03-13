use crate::cli::actions::Action;
use crate::config;
use crate::output::{CheckResult, OutputFormat, format_influxdb, format_prometheus};
use crate::s3;
use anyhow::Result;
use std::sync::Arc;

/// Execute the given action.
///
/// # Errors
///
/// Returns an error if the config file cannot be read or parsed, or if the
/// S3 client cannot be initialised.
pub async fn execute(action: &Action) -> Result<()> {
    match action {
        Action::Monitor { config, format } => {
            let file = std::fs::File::open(config)
                .map_err(|e| anyhow::anyhow!("cannot open config '{}': {e}", config.display()))?;

            let yml: config::Config = serde_yaml::from_reader(file)
                .map_err(|e| anyhow::anyhow!("error parsing config: {e}"))?;

            let monitor = Arc::new(s3::Monitor::new(&yml).await?);

            let mut tasks = vec![];

            for (bucket_name, files) in yml.s3mon.buckets {
                for file in files {
                    let m = Arc::clone(&monitor);
                    let bucket = bucket_name.clone();
                    tasks.push(tokio::spawn(async move { check(&m, bucket, file).await }));
                }
            }

            let mut results: Vec<CheckResult> = vec![];
            for task in tasks {
                results.push(task.await.map_err(|e| anyhow::anyhow!("task error: {e}"))?);
            }

            let output = match format {
                OutputFormat::Prometheus => format_prometheus(&results),
                OutputFormat::Influxdb => format_influxdb(&results),
            };

            print!("{output}");
            Ok(())
        }
    }
}

async fn check(monitor: &s3::Monitor, bucket: String, file: config::Object) -> CheckResult {
    let mut exist = false;
    let mut size_mismatch = false;
    let mut error = false;

    match monitor.objects(&bucket, &file.prefix, file.age).await {
        Ok(objects) => {
            if !objects.is_empty() {
                exist = true;
            }
            for o in &objects {
                if file.size > 0
                    && let Some(size) = o.size()
                    && size < file.size
                {
                    size_mismatch = true;
                }
            }
        }
        Err(e) => {
            eprintln!("Error: {e}");
            error = true;
        }
    }

    CheckResult {
        bucket,
        prefix: file.prefix,
        exist,
        error,
        size_mismatch,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use aws_sdk_s3::config::{Credentials, Region};
    use aws_smithy_runtime::client::http::test_util::{ReplayEvent, StaticReplayClient};
    use aws_smithy_types::body::SdkBody;
    use chrono::prelude::{SecondsFormat, Utc};

    fn make_monitor(status: u16, body: &str) -> s3::Monitor {
        let http_client = StaticReplayClient::new(vec![ReplayEvent::new(
            http::Request::builder()
                .body(SdkBody::empty())
                .map_err(|e| anyhow::anyhow!("{e}"))
                .unwrap_or_else(|_| unreachable!()),
            http::Response::builder()
                .status(status)
                .body(SdkBody::from(body))
                .map_err(|e| anyhow::anyhow!("{e}"))
                .unwrap_or_else(|_| unreachable!()),
        )]);

        let cfg = aws_sdk_s3::Config::builder()
            .behavior_version(aws_sdk_s3::config::BehaviorVersion::latest())
            .region(Region::new("us-east-1"))
            .credentials_provider(Credentials::new("test", "test", None, None, "test"))
            .http_client(http_client)
            .build();

        s3::Monitor {
            s3: aws_sdk_s3::Client::from_conf(cfg),
        }
    }

    #[tokio::test]
    async fn check_object() {
        let last_modified = Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true);
        let body = format!(
            r#"<?xml version="1.0" encoding="UTF-8"?>
            <ListBucketResult xmlns="http://s3.amazonaws.com/doc/2006-03-01/">
              <Name>cubeta</Name>
              <Prefix>E</Prefix>
              <KeyCount>1</KeyCount>
              <MaxKeys>3</MaxKeys>
              <IsTruncated>false</IsTruncated>
              <Contents>
                <Key>ExampleObject.txt</Key>
                <LastModified>{last_modified}</LastModified>
                <ETag>"599bab3ed2c697f1d26842727561fd94"</ETag>
                <Size>857</Size>
                <StorageClass>REDUCED_REDUNDANCY</StorageClass>
              </Contents>
            </ListBucketResult>"#
        );

        let monitor = Arc::new(make_monitor(200, &body));
        let file = config::Object {
            prefix: "E".to_string(),
            age: 30,
            size: 0,
        };
        let result = check(&monitor, "cubeta".to_string(), file).await;
        assert!(result.exist);
        assert!(!result.error);
        assert!(!result.size_mismatch);
    }

    #[tokio::test]
    async fn check_object_size_mismatch() {
        let last_modified = Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true);
        let body = format!(
            r#"<?xml version="1.0" encoding="UTF-8"?>
            <ListBucketResult xmlns="http://s3.amazonaws.com/doc/2006-03-01/">
              <Name>cubeta</Name>
              <Prefix>E</Prefix>
              <KeyCount>1</KeyCount>
              <MaxKeys>3</MaxKeys>
              <IsTruncated>false</IsTruncated>
              <Contents>
                <Key>ExampleObject.txt</Key>
                <LastModified>{last_modified}</LastModified>
                <ETag>"599bab3ed2c697f1d26842727561fd94"</ETag>
                <Size>857</Size>
                <StorageClass>REDUCED_REDUNDANCY</StorageClass>
              </Contents>
            </ListBucketResult>"#
        );

        let monitor = Arc::new(make_monitor(200, &body));
        let file = config::Object {
            prefix: "E".to_string(),
            age: 30,
            size: 1024,
        };
        let result = check(&monitor, "cubeta".to_string(), file).await;
        assert!(result.exist);
        assert!(!result.error);
        assert!(result.size_mismatch);
    }

    #[tokio::test]
    async fn check_object_age_expired() {
        let body = r#"<?xml version="1.0" encoding="UTF-8"?>
            <ListBucketResult xmlns="http://s3.amazonaws.com/doc/2006-03-01/">
              <Name>cubeta</Name>
              <Prefix>E</Prefix>
              <KeyCount>1</KeyCount>
              <MaxKeys>3</MaxKeys>
              <IsTruncated>false</IsTruncated>
              <Contents>
                <Key>ExampleObject.txt</Key>
                <LastModified>2019-10-14T08:52:23.231Z</LastModified>
                <ETag>"599bab3ed2c697f1d26842727561fd94"</ETag>
                <Size>857</Size>
                <StorageClass>REDUCED_REDUNDANCY</StorageClass>
              </Contents>
            </ListBucketResult>"#;

        let monitor = Arc::new(make_monitor(200, body));
        let file = config::Object {
            prefix: "E".to_string(),
            age: 30,
            size: 1024,
        };
        let result = check(&monitor, "cubeta".to_string(), file).await;
        assert!(!result.exist);
        assert!(!result.error);
        assert!(!result.size_mismatch);
    }

    #[tokio::test]
    async fn check_object_no_bucket() {
        let body = r#"<?xml version="1.0" encoding="UTF-8"?>
            <Error>
                <Code>NoSuchBucket</Code>
                <Message>The specified bucket does not exist</Message>
                <RequestId>4442587FB7D0A2F9</RequestId>
            </Error>"#;

        let monitor = Arc::new(make_monitor(404, body));
        let file = config::Object {
            prefix: "E".to_string(),
            age: 30,
            size: 512,
        };
        let result = check(&monitor, "cubeta".to_string(), file).await;
        assert!(!result.exist);
        assert!(result.error);
        assert!(!result.size_mismatch);
    }

    #[test]
    fn prometheus_output_fresh_object() {
        let results = vec![CheckResult {
            bucket: "cubeta".to_string(),
            prefix: "E".to_string(),
            exist: true,
            error: false,
            size_mismatch: false,
        }];
        let out = format_prometheus(&results);
        assert!(out.contains(r#"s3mon_object_exists{bucket="cubeta",prefix="E"} 1"#));
        assert!(out.contains(r#"s3mon_check_error{bucket="cubeta",prefix="E"} 0"#));
        assert!(out.contains(r#"s3mon_size_mismatch{bucket="cubeta",prefix="E"} 0"#));
    }

    #[test]
    fn influxdb_output_fresh_object() {
        let results = vec![CheckResult {
            bucket: "cubeta".to_string(),
            prefix: "E".to_string(),
            exist: true,
            error: false,
            size_mismatch: false,
        }];
        let out = format_influxdb(&results);
        assert!(out.contains("s3mon,bucket=cubeta,prefix=E error=0i,exist=1i,size_mismatch=0i"));
    }
}
