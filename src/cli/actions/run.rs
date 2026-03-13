use crate::cli::actions::Action;
use crate::config;
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
        Action::Monitor { config } => {
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
                    tasks.push(tokio::spawn(async move {
                        println!("{}", check(&m, bucket, file).await);
                    }));
                }
            }

            for task in tasks {
                task.await.map_err(|e| anyhow::anyhow!("task error: {e}"))?;
            }

            Ok(())
        }
    }
}

async fn check(monitor: &s3::Monitor, bucket: String, file: config::Object) -> String {
    let mut output: Vec<String> = Vec::new();
    output.push(format!("s3mon,bucket={},prefix={}", bucket, file.prefix));

    let mut exist = false;
    let mut size_mismatch = false;
    let mut bucket_error = false;

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
            bucket_error = true;
        }
    }

    output.push(format!(
        "error={}i,exist={}i,size_mismatch={}i",
        i32::from(bucket_error),
        i32::from(exist),
        i32::from(size_mismatch),
    ));

    output.join(" ")
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
        assert_eq!(
            check(&monitor, "cubeta".to_string(), file).await,
            "s3mon,bucket=cubeta,prefix=E error=0i,exist=1i,size_mismatch=0i",
        );
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
        assert_eq!(
            check(&monitor, "cubeta".to_string(), file).await,
            "s3mon,bucket=cubeta,prefix=E error=0i,exist=1i,size_mismatch=1i",
        );
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
        assert_eq!(
            check(&monitor, "cubeta".to_string(), file).await,
            "s3mon,bucket=cubeta,prefix=E error=0i,exist=0i,size_mismatch=0i",
        );
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
        assert_eq!(
            check(&monitor, "cubeta".to_string(), file).await,
            "s3mon,bucket=cubeta,prefix=E error=1i,exist=0i,size_mismatch=0i",
        );
    }
}
