use clap::{App, Arg};
use serde_yaml;

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

    let s3 = s3::S3monS3::new(&yml);

    for bucket in yml.s3mon.buckets {
        for file in bucket.1 {
            if let Ok(objects) = s3.objects(bucket.0.to_string(), file.prefix, file.age) {
                for o in objects {
                    if file.size > 0 {
                        if let Some(size) = o.size {
                            println!("{}", size);
                        }
                    }
                    if let Some(key) = o.key {
                        println!("key: {}", key);
                    }
                    if let Some(lm) = o.last_modified {
                        println!("lm: {}", lm);
                    }
                    if let Some(size) = o.size {
                        println!("size: {}", size);
                    }
                }
            }
        }
    }
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
