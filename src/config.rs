use serde::Deserialize;
use std::collections::BTreeMap;

#[derive(Debug, Deserialize, PartialEq)]
pub struct Config {
    pub s3mon: Data,
}

#[derive(Debug, Deserialize, PartialEq)]
pub struct Data {
    #[serde(default)]
    pub endpoint: String,
    #[serde(default)]
    pub region: String,
    #[serde(default)]
    pub access_key: String,
    #[serde(default)]
    pub secret_key: String,
    pub buckets: BTreeMap<String, Vec<Object>>,
}

#[derive(Debug, Deserialize, PartialEq)]
pub struct Object {
    pub prefix: String,
    #[serde(default = "default_age")]
    pub age: i64,
    #[serde(default)]
    pub size: i64,
}

const fn default_age() -> i64 {
    86400
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_config() -> Result<(), serde_yaml::Error> {
        let yml = r"
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
        ";
        let mut buckets = std::collections::BTreeMap::new();
        buckets.insert(
            "bucket_A".to_string(),
            vec![
                Object {
                    prefix: "foo".to_string(),
                    age: 43200,
                    size: 0,
                },
                Object {
                    prefix: "bar".to_string(),
                    age: 86400,
                    size: 0,
                },
                Object {
                    prefix: "baz".to_string(),
                    age: 86400,
                    size: 1024,
                },
            ],
        );
        let expected = Config {
            s3mon: Data {
                endpoint: "endpoint".to_string(),
                region: "region".to_string(),
                access_key: "ACCESS_KEY_ID".to_string(),
                secret_key: "SECRET_ACCESS_KEY".to_string(),
                buckets,
            },
        };
        let parsed: Config = serde_yaml::from_str(yml)?;
        assert_eq!(expected, parsed);
        Ok(())
    }
}
