use serde::Deserialize;
use std::collections::BTreeMap;

#[derive(Debug, Deserialize, Default)]
pub struct Config {
    pub s3mon: Data,
}

#[derive(Debug, Deserialize, Default)]
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
    pub file: String,
    pub age: u32,
}
