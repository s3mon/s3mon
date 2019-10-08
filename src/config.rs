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
    pub age: u32,
}
