use clap::{App, Arg};
use env_logger;
use serde_yaml;
use std::sync::Arc;
use std::{process, thread};

mod auth;
mod config;
mod s3;
mod slack;

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
        Err(err) => {
            eprintln!("Error parsing configuration file: {}", err);
            process::exit(1);
        }
        Ok(yml) => yml,
    };

    // create an S3 Client
    let s3 = Arc::new(s3::S3monS3::new(&yml));

    // store all threads
    let mut children = vec![];

    for bucket in yml.s3mon.buckets {
        let bucket_name = bucket.0.to_string();
        for file in bucket.1 {
            let thread_s3 = Arc::clone(&s3);
            let bucket = bucket_name.clone();
            children.push(thread::spawn(|| {
                check(thread_s3, bucket, file);
            }));
        }
    }

    // Wait for all the threads to finish
    for child in children {
        let _ = child.join();
    }
}

fn check(s3: Arc<s3::S3monS3>, bucket: String, file: config::Object) {
    // create InfluxDB line protocol
    // https://docs.influxdata.com/influxdb/v1.7/write_protocols/line_protocol_tutorial/
    let mut output: Vec<String> = Vec::new();
    output.push(format!("{},prefix={}", bucket, file.prefix));

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
        "error={} exist={} size_mismatch={}",
        bucket_error, exist, size_mismatch
    ));

    println!("{}", output.join(" "));
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
}
