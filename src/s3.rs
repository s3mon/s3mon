use crate::auth;
use crate::config;
use chrono::prelude::{DateTime, Utc};
use rusoto_core::region::ParseRegionError;
use rusoto_core::request::HttpClient;
use rusoto_core::Region;
use rusoto_s3::{ListObjectsV2Request, Object, S3Client, S3};

pub struct S3monS3 {
    pub s3: S3Client,
}

impl S3monS3 {
    pub fn new(config: &config::Config) -> Result<Self, ParseRegionError> {
        let chain = auth::Auth::new(
            config.s3mon.access_key.to_string(),
            config.s3mon.secret_key.to_string(),
        );

        let region = if config.s3mon.endpoint.is_empty() && !config.s3mon.region.is_empty() {
            config.s3mon.region.parse()?
        } else {
            Region::Custom {
                name: config.s3mon.region.to_owned(),
                endpoint: config.s3mon.endpoint.to_owned(),
            }
        };

        Ok(S3monS3 {
            s3: rusoto_s3::S3Client::new_with(
                HttpClient::new().expect("failed to create request dispatcher"),
                chain,
                region,
            ),
        })
    }

    pub fn objects(&self, bucket: String, prefix: String, age: i64) -> Result<Vec<Object>, String> {
        let now = Utc::now();
        let age = now - chrono::Duration::seconds(age);

        let list_objects_req = ListObjectsV2Request {
            bucket,
            prefix: Some(prefix),
            ..Default::default()
        };

        let objects = match self.s3.list_objects_v2(list_objects_req).sync() {
            // loop over the results parsing the last_modified and converting
            // to unix timestamp and then return only objects < the defined age
            Ok(result) => result
                .contents
                .unwrap_or_default()
                .into_iter()
                .filter(move |obj| {
                    DateTime::parse_from_rfc3339(
                        obj.last_modified.clone().unwrap_or_default().as_str(),
                    )
                    .ok()
                    .into_iter()
                    .map(|parsed| parsed.timestamp())
                    .any(|last_modified| last_modified > age.timestamp())
                })
                .collect::<Vec<_>>(),
            Err(e) => return Err(e.to_string()),
        };
        Ok(objects)
    }
}
