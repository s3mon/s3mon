use clap::{App, Arg};
use serde_yaml;
use std::sync::Arc;
use std::thread;

mod auth;
mod config;
mod envs;
mod s3;
mod slack;

fn main() {
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
    let config = matches
        .value_of("config")
        .expect("Unable to open configuration file");

    // parse config file
    let file = std::fs::File::open(&config).expect("Unable to open file");
    let yml: config::Config = match serde_yaml::from_reader(file) {
        Err(err) => {
            println!("Error: {}", err);
            return;
        }
        Ok(yml) => yml,
    };

    let s3 = Arc::new(s3::S3monS3::new(&yml));

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
    for child in children {
        // Wait for the thread to finish. Returns a result.
        let _ = child.join();
    }
}

fn check(s3: Arc<s3::S3monS3>, bucket: String, file: config::Object) {
    let mut output: Vec<String> = Vec::new();
    output.push(format!("{},prefix={}", bucket, file.prefix));
    let mut exist = false;
    let mut size_mismatch = false;
    if let Ok(objects) = s3.objects(bucket, file.prefix, file.age) {
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
    output.push(format!("exist={}", exist));
    if size_mismatch {
        output.push("size_mismatch=1".to_string());
    }
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
