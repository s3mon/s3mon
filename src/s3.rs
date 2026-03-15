use crate::config;
use anyhow::Result;
use aws_credential_types::Credentials;
use aws_sdk_s3::Client;
use aws_smithy_http_client::Builder as HttpClientBuilder;
use aws_smithy_http_client::tls;
use chrono::prelude::Utc;

pub struct Monitor {
    pub s3: Client,
}

pub struct CheckStats {
    pub exists: bool,
    pub any_large_enough: bool,
}

impl Monitor {
    /// Create a new S3 monitor client from the given configuration.
    ///
    /// Credential resolution order:
    /// 1. If `access_key` and `secret_key` are both non-empty in the config,
    ///    those static credentials are used directly.
    /// 2. Otherwise the AWS default credential chain is used (environment
    ///    variables, instance profiles, etc.).
    ///
    /// # Errors
    ///
    /// Returns an error if the AWS configuration cannot be loaded.
    pub async fn new(config: &config::Config) -> Result<Self> {
        let http_client = HttpClientBuilder::new()
            .tls_provider(tls::Provider::Rustls(
                tls::rustls_provider::CryptoMode::Ring,
            ))
            .build_https();

        let mut cfg_builder =
            aws_config::defaults(aws_config::BehaviorVersion::latest()).http_client(http_client);

        if !config.s3mon.access_key.is_empty() && !config.s3mon.secret_key.is_empty() {
            let creds = Credentials::new(
                &config.s3mon.access_key,
                &config.s3mon.secret_key,
                None,
                None,
                "s3mon-config",
            );
            cfg_builder = cfg_builder.credentials_provider(creds);
        }

        if !config.s3mon.region.is_empty() {
            cfg_builder = cfg_builder.region(aws_config::Region::new(config.s3mon.region.clone()));
        }

        let aws_cfg = cfg_builder.load().await;

        let mut s3_builder = aws_sdk_s3::Config::from(&aws_cfg).to_builder();

        if !config.s3mon.endpoint.is_empty() {
            s3_builder = s3_builder
                .endpoint_url(&config.s3mon.endpoint)
                .force_path_style(true);
        }

        Ok(Self {
            s3: Client::from_conf(s3_builder.build()),
        })
    }

    /// Check objects in `bucket` under `prefix` that are newer than `age` seconds.
    ///
    /// # Errors
    ///
    /// Returns an error if the S3 API call fails.
    pub async fn check_storage(
        &self,
        bucket: &str,
        prefix: &str,
        suffix: &str,
        age: i64,
        min_size: i64,
    ) -> Result<CheckStats> {
        let cutoff = (Utc::now()
            - chrono::Duration::try_seconds(age)
                .ok_or_else(|| anyhow::anyhow!("invalid age value: {age}"))?)
        .timestamp();

        let mut exists = false;
        let mut any_large_enough = false;

        let mut paginator = self
            .s3
            .list_objects_v2()
            .bucket(bucket)
            .prefix(prefix)
            .into_paginator()
            .send();

        while let Some(page) = paginator.next().await {
            let page = page.map_err(|e| anyhow::anyhow!("{e}"))?;
            for obj in page.contents() {
                let matches_suffix =
                    suffix.is_empty() || obj.key().is_some_and(|key| key.ends_with(suffix));

                if matches_suffix && obj.last_modified().is_some_and(|lm| lm.secs() > cutoff) {
                    exists = true;
                    if min_size <= 0 || obj.size().is_some_and(|s| s >= min_size) {
                        any_large_enough = true;
                    }
                }
            }
            // Optimization: if we already found a large enough object, and we only care
            // about existence and size, we can stop if we don't need to count them.
            if exists && (min_size <= 0 || any_large_enough) {
                break;
            }
        }

        Ok(CheckStats {
            exists,
            any_large_enough,
        })
    }
}
