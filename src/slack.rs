use crate::envs::get_env;
use slack_hook::{PayloadBuilder, Slack};

pub fn send_msg(msg: String) {
    let slack_url = get_env("SLACK_WEBHOOK_URL");

    let slack = Slack::new(&*slack_url).unwrap();


    let p =  PayloadBuilder::new()
        .text("No files found")
        .channel("#backups")
        .username("s3mon");

    let p = if msg.is_empty() {
        p.text("No files found")
            .icon_emoji(":warning:")
    } else {
        p.text(msg)
    }.build().unwrap();

    match slack.send(&p) {
        Ok(()) => println!("msg sent"),
        Err(x) => println!("ERR: {:?}", x),
    }
}
