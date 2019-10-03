use chrono::prelude::{DateTime, Utc};
use rusoto_core::request::HttpClient;
use rusoto_core::Region;
use rusoto_s3::{ListObjectsV2Request, S3Client, S3};

use crate::auth;
use crate::config;

pub struct S3monS3 {
    s3: S3Client,
}

impl S3monS3 {
    pub fn new(config: config::Config) -> Self {
        let chain = auth::Auth::new(config.s3mon.access_key, config.s3mon.secret_key);

        let region = Region::Custom {
            // TODO
            name: "s3mon".to_owned(),
            endpoint: config.s3mon.endpoint.to_owned(),
        };

        S3monS3 {
            s3: rusoto_s3::S3Client::new_with(
                HttpClient::new().expect("failed to create request dispatcher"),
                chain,
                region,
            ),
        }
    }

    pub fn list_buckets(&self) {
        match self.s3.list_buckets().sync() {
            Ok(output) => match output.buckets {
                Some(s3_bucket_lists) => {
                    println!("Buckets:");
                    for bucket in s3_bucket_lists {
                        println!(
                            "Name: {}, CreationDate: {}",
                            bucket.name.unwrap_or_default(),
                            bucket.creation_date.unwrap_or_default()
                        );
                    }
                }
                None => println!("No buckets in account!"),
            },
            Err(error) => {
                println!("Error: {:?}", error);
            }
        }
    }

    pub fn get_objects(&self) {
        let list_obj_req = ListObjectsV2Request {
            bucket: "test".to_string(),
            max_keys: Some(10),
            ..Default::default()
        };

        if let Ok(result) = self.s3.list_objects_v2(list_obj_req).sync() {
            for f in result.contents {
                println!("file: {:?}", f);
            }
        }
    }

    pub fn objects(&self) -> String {
        let now = Utc::now();
        //        let until = now - chrono::Duration::minutes(1);
        let after = now - chrono::Duration::hours(12);
        //  let window = after.time()..until.time();
        // let today = now.format("%Y/%m/%d").to_string();
        // let date_path = format!("{}/{}", path, today);

        let objects = self
            .s3
            .list_objects_v2(ListObjectsV2Request {
                bucket: "test".to_owned(),
//                prefix: Some(prefix),
                ..ListObjectsV2Request::default()
            })
            .sync()
            .unwrap()
            .contents
            .unwrap_or_default()
            .into_iter()
            .filter(move |obj| {
                DateTime::parse_from_rfc3339(obj.last_modified.clone().unwrap_or_default().as_str())
                    .ok()
                    .into_iter()
                    .map(|parsed| parsed.time())
                    .any(|last_modified| {
                        println!("last_modified {}, after: {}", last_modified, after.time());
                        //after.time() < last_modified && last_modified < until.time()
                        after.time() < last_modified
                    })
            })
            .collect::<Vec<_>>();
        println!("found {} objects", objects.len());
         let mut string = String::new();
        for o in objects {
            string.push_str(format!("{} - {}\n", o.key.unwrap(), o.size.unwrap()).as_str());
        }
        return string
    }
}
