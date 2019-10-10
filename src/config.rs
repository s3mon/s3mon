use serde::Deserialize;
use std::collections::BTreeMap;

#[derive(Debug, Deserialize)]
pub struct Config {
    pub s3mon: Data,
}

#[derive(Debug, Deserialize)]
pub struct Data {
    pub endpoint: String,
    #[serde(default)]
    pub region: String,
    #[serde(default)]
    pub access_key: String,
    #[serde(default)]
    pub secret_key: String,
    pub buckets: BTreeMap<String, Vec<Object>>,
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
