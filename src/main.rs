use clap::{App, Arg};
use serde_yaml;
use std::{
    thread,
    time::{Duration, Instant},
};

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

    let s3 = s3::S3monS3::new(yml);

    loop {
        let start = Instant::now();
        let wait_time = Duration::from_secs(30);
        if let Ok(objects) = s3.objects() {
            println!("{:?}", objects);
            //    slack::send_msg(objects);
        }
        let runtime = start.elapsed();
        if let Some(remaining) = wait_time.checked_sub(runtime) {
            thread::sleep(remaining);
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
