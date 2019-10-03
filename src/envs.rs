use std::{env, process};

pub fn get_env(e: &str) -> String {
    let value = match e {
        "SLACK_WEBHOOK_URL" => env::var(e).unwrap_or_else(|e| {
            println!("could not find {}: {}", "SLACK_WEBHOOK_URL", e);
            process::exit(1);
        }),
        _ => "??".into(),
    };
    return value;
}
