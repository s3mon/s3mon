use clap::{App, Arg};
use env_logger;
use serde_yaml;
use std::sync::Arc;
use std::{process, thread};

mod auth;
mod config;
mod s3;

fn main() {
    // RUST_LOG=debug
    let _ = env_logger::try_init();

    // cli options
    let matches = App::new("s3mon")
        .version(env!("CARGO_PKG_VERSION"))
        .arg(
            Arg::with_name("config")
                .help("config.yml")
                .long("config")
                .short("c")
                .required(false)
                .value_name("FILE")
                .takes_value(true)
                .validator(is_file),
        )
        .get_matches();

    // Gets a value for config if supplied by user, or defaults to "default.conf"
    let config = matches.value_of("config").unwrap_or_else(|| {
        eprintln!("Unable to open configuration file, use (\"-h for help\")");
        process::exit(1);
    });

    // parse config file
    let file = std::fs::File::open(&config).expect("Unable to open file");
    let yml: config::Config = match serde_yaml::from_reader(file) {
        Err(e) => {
            eprintln!("Error parsing configuration file: {}", e);
            process::exit(1);
        }
        Ok(yml) => yml,
    };

    // create an S3 Client
    let s3 = match s3::S3monS3::new(&yml) {
        Ok(s3) => Arc::new(s3),
        Err(e) => {
            eprintln!("Error: {}", e);
            process::exit(1);
        }
    };

    // store all threads
    let mut children = vec![];

    for bucket in yml.s3mon.buckets {
        let bucket_name = bucket.0.to_string();
        for file in bucket.1 {
            let thread_s3 = Arc::clone(&s3);
            let bucket = bucket_name.clone();
            children.push(thread::spawn(|| {
                println!("{}", check(thread_s3, bucket, file));
            }));
        }
    }

    // Wait for all the threads to finish
    for child in children {
        let _ = child.join();
    }
}

fn check(s3: Arc<s3::S3monS3>, bucket: String, file: config::Object) -> String {
    // create InfluxDB line protocol
    // https://docs.influxdata.com/influxdb/v1.7/write_protocols/line_protocol_tutorial/
    let mut output: Vec<String> = Vec::new();
    output.push(format!("s3mon,bucket={},prefix={}", bucket, file.prefix));

    let mut exist = false;
    let mut size_mismatch = false;
    let mut bucket_error = false;

    // query the bucket
    match s3.objects(bucket, file.prefix, file.age) {
        Ok(objects) => {
            if objects.len() > 0 {
                exist = true;
            }
            for o in objects {
                if file.size > 0 {
                    if let Some(size) = o.size {
                        if size < file.size {
                            size_mismatch = true;
                        }
                    }
                }
            }
        }
        Err(e) => {
            eprintln!("Error: {}", e);
            bucket_error = true;
        }
    }

    output.push(format!(
        "error={}i,exist={}i,size_mismatch={}i",
        bucket_error as i32, exist as i32, size_mismatch as i32,
    ));

    return output.join(" ");
}

fn is_file(s: String) -> Result<(), String> {
    let metadata = match std::fs::metadata(&s) {
        Err(err) => return Err(err.to_string()),
        Ok(metadata) => metadata,
    };
    if !metadata.is_file() {
        return Err(String::from(format!("cannot read file: {}", s)));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_config() -> Result<(), serde_yaml::Error> {
        let yml = r#"
---
s3mon:
  endpoint: endpoint
  region: region
  access_key: ACCESS_KEY_ID
  secret_key: SECRET_ACCESS_KEY
  buckets:
    bucket_A:
      - prefix: foo
        age: 43200
      - prefix: bar
      - prefix: baz
        size: 1024
        "#;
        let mut buckets = std::collections::BTreeMap::new();
        buckets.insert(
            "bucket_A".to_string(),
            vec![
                config::Object {
                    prefix: "foo".to_string(),
                    age: 43200,
                    size: 0,
                },
                config::Object {
                    prefix: "bar".to_string(),
                    age: 86400,
                    size: 0,
                },
                config::Object {
                    prefix: "baz".to_string(),
                    age: 86400,
                    size: 1024,
                },
            ],
        );
        let cfg = config::Config {
            s3mon: config::Data {
                endpoint: "endpoint".to_string(),
                region: "region".to_string(),
                access_key: "ACCESS_KEY_ID".to_string(),
                secret_key: "SECRET_ACCESS_KEY".to_string(),
                buckets: buckets,
            },
        };
        let y: config::Config = serde_yaml::from_str(yml)?;
        assert_eq!(cfg, y);
        Ok(())
    }

    #[test]
    fn check_object() {
        use chrono::prelude::{SecondsFormat, Utc};
        use rusoto_core::Region;
        use rusoto_mock::{MockCredentialsProvider, MockRequestDispatcher};
        use rusoto_s3::S3Client;

        let last_modified = Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true);

        let mock = MockRequestDispatcher::with_status(200).with_body(
            format!(
                r#"<?xml version="1.0" encoding="UTF-8"?>
                <ListBucketResult xmlns="http://s3.amazonaws.com/doc/2006-03-01/">
                  <Name>cubeta</Name>
                  <Prefix>E</Prefix>
                  <StartAfter>ExampleGuide.pdf</StartAfter>
                  <KeyCount>1</KeyCount>
                  <MaxKeys>3</MaxKeys>
                  <IsTruncated>false</IsTruncated>
                  <Contents>
                    <Key>ExampleObject.txt</Key>
                    <LastModified>{}</LastModified>
                    <ETag>"599bab3ed2c697f1d26842727561fd94"</ETag>
                    <Size>857</Size>
                    <StorageClass>REDUCED_REDUNDANCY</StorageClass>
                  </Contents>
                </ListBucketResult>
            "#,
                last_modified
            )
            .as_str(),
        );
        let client = Arc::new(s3::S3monS3 {
            s3: S3Client::new_with(mock, MockCredentialsProvider, Region::UsEast1),
        });
        // test finding file & prefix
        let file = config::Object {
            prefix: "E".to_string(),
            age: 30,
            size: 0,
        };
        assert_eq!(
            check(client.clone(), "cubeta".to_string(), file),
            "s3mon,bucket=cubeta,prefix=E error=0i,exist=1i,size_mismatch=0i",
        );
    }

    #[test]
    fn check_object_size_mismatch() {
        use chrono::prelude::{SecondsFormat, Utc};
        use rusoto_core::Region;
        use rusoto_mock::{MockCredentialsProvider, MockRequestDispatcher};
        use rusoto_s3::S3Client;

        let last_modified = Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true);

        let mock = MockRequestDispatcher::with_status(200).with_body(
            format!(
                r#"<?xml version="1.0" encoding="UTF-8"?>
                <ListBucketResult xmlns="http://s3.amazonaws.com/doc/2006-03-01/">
                  <Name>cubeta</Name>
                  <Prefix>E</Prefix>
                  <StartAfter>ExampleGuide.pdf</StartAfter>
                  <KeyCount>1</KeyCount>
                  <MaxKeys>3</MaxKeys>
                  <IsTruncated>false</IsTruncated>
                  <Contents>
                    <Key>ExampleObject.txt</Key>
                    <LastModified>{}</LastModified>
                    <ETag>"599bab3ed2c697f1d26842727561fd94"</ETag>
                    <Size>857</Size>
                    <StorageClass>REDUCED_REDUNDANCY</StorageClass>
                  </Contents>
                </ListBucketResult>
            "#,
                last_modified
            )
            .as_str(),
        );
        let client = Arc::new(s3::S3monS3 {
            s3: S3Client::new_with(mock, MockCredentialsProvider, Region::UsEast1),
        });
        // test finding file & prefix
        let file = config::Object {
            prefix: "E".to_string(),
            age: 30,
            size: 1024,
        };
        assert_eq!(
            check(client.clone(), "cubeta".to_string(), file),
            "s3mon,bucket=cubeta,prefix=E error=0i,exist=1i,size_mismatch=1i",
        );
    }

    #[test]
    fn check_object_age_expired() {
        use rusoto_core::Region;
        use rusoto_mock::{MockCredentialsProvider, MockRequestDispatcher};
        use rusoto_s3::S3Client;

        let mock = MockRequestDispatcher::with_status(200).with_body(
            r#"<?xml version="1.0" encoding="UTF-8"?>
                <ListBucketResult xmlns="http://s3.amazonaws.com/doc/2006-03-01/">
                  <Name>cubeta</Name>
                  <Prefix>E</Prefix>
                  <StartAfter>ExampleGuide.pdf</StartAfter>
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
                </ListBucketResult>
            "#,
        );
        let client = Arc::new(s3::S3monS3 {
            s3: S3Client::new_with(mock, MockCredentialsProvider, Region::UsEast1),
        });
        // test finding file & prefix
        let file = config::Object {
            prefix: "E".to_string(),
            age: 30,
            size: 1024,
        };
        assert_eq!(
            check(client.clone(), "cubeta".to_string(), file),
            "s3mon,bucket=cubeta,prefix=E error=0i,exist=0i,size_mismatch=0i",
        );
    }

    #[test]
    fn check_object_no_bucket() {
        use rusoto_core::Region;
        use rusoto_mock::{MockCredentialsProvider, MockRequestDispatcher};
        use rusoto_s3::S3Client;

        let mock = MockRequestDispatcher::with_status(404).with_body(
            r#"<?xml version="1.0" encoding="UTF-8"?>
	    <Error>
		<Code>NoSuchBucket</Code>
		<Message>The specified bucket does not exist</Message>
		<RequestId>4442587FB7D0A2F9</RequestId>
	    </Error>"#,
        );
        let client = Arc::new(s3::S3monS3 {
            s3: S3Client::new_with(mock, MockCredentialsProvider, Region::UsEast1),
        });
        // test finding file & prefix
        let file = config::Object {
            prefix: "E".to_string(),
            age: 30,
            size: 512,
        };

        assert_eq!(
            check(client.clone(), "cubeta".to_string(), file),
            "s3mon,bucket=cubeta,prefix=E error=1i,exist=0i,size_mismatch=0i",
        );
    }
}
