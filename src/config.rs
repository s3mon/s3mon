use serde::Deserialize;
use std::collections::BTreeMap;

#[derive(Debug, Deserialize)]
pub struct Config {
    pub s3mon: Data,
}

#[derive(Debug, Deserialize)]
pub struct Data {
    pub endpoint: String,
    pub access_key: String,
    pub secret_key: String,
    pub buckets: BTreeMap<String, Vec<Object>>,
    #[serde(default)]
    pub region: String,
}

#[derive(Debug, Deserialize)]
pub struct Object {
    pub prefix: String,
    #[serde(default = "default_age")]
    pub age: i64,
    #[serde(default)]
    pub size: i64,
}

fn default_age() -> i64 {
    86400
}
