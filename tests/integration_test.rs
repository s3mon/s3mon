#![allow(clippy::pedantic)]

mod helpers;

/// A freshly uploaded object is visible and not age-expired.
#[tokio::test]
async fn object_exists_and_fresh() -> anyhow::Result<()> {
    if !helpers::has_container_runtime() {
        return Ok(());
    }
    let env = helpers::start_minio().await?;
    env.create_bucket("test-fresh").await?;
    env.put_object("test-fresh", "data/file.txt", b"hello world")
        .await?;

    let stats = env
        .monitor
        .check_storage("test-fresh", "data/", 86400, 0)
        .await?;
    assert!(stats.exists, "expected exactly 1 fresh object");

    Ok(())
}

/// With age=0 the cutoff equals now, so any object already stored is considered expired.
#[tokio::test]
async fn object_exists_age_expired() -> anyhow::Result<()> {
    if !helpers::has_container_runtime() {
        return Ok(());
    }
    let env = helpers::start_minio().await?;
    env.create_bucket("test-expired").await?;
    env.put_object("test-expired", "data/file.txt", b"hello world")
        .await?;

    let stats = env
        .monitor
        .check_storage("test-expired", "data/", 0, 0)
        .await?;
    assert!(!stats.exists, "expected no objects with age=0");

    Ok(())
}

/// An object whose size is below the configured minimum triggers a size mismatch.
#[tokio::test]
async fn object_size_below_threshold() -> anyhow::Result<()> {
    if !helpers::has_container_runtime() {
        return Ok(());
    }
    let env = helpers::start_minio().await?;
    env.create_bucket("test-size").await?;
    // 11 bytes – intentionally smaller than the 1 024-byte threshold
    env.put_object("test-size", "data/file.txt", b"hello world")
        .await?;

    let stats = env
        .monitor
        .check_storage("test-size", "data/", 86400, 1024)
        .await?;
    assert!(stats.exists, "expected 1 object");
    assert!(!stats.any_large_enough, "size should be below threshold");

    Ok(())
}

/// A bucket that exists but contains no matching keys reports zero objects.
#[tokio::test]
async fn prefix_not_found() -> anyhow::Result<()> {
    if !helpers::has_container_runtime() {
        return Ok(());
    }
    let env = helpers::start_minio().await?;
    env.create_bucket("test-empty").await?;

    let stats = env
        .monitor
        .check_storage("test-empty", "missing/prefix/", 86400, 0)
        .await?;
    assert!(!stats.exists, "expected no objects for missing prefix");

    Ok(())
}

/// Two prefixes in the same bucket: one populated, one empty.
#[tokio::test]
async fn multiple_prefixes_one_missing() -> anyhow::Result<()> {
    if !helpers::has_container_runtime() {
        return Ok(());
    }
    let env = helpers::start_minio().await?;
    env.create_bucket("test-multi-prefix").await?;
    env.put_object("test-multi-prefix", "present/data.bin", b"payload")
        .await?;

    let present = env
        .monitor
        .check_storage("test-multi-prefix", "present/", 86400, 0)
        .await?;
    let missing = env
        .monitor
        .check_storage("test-multi-prefix", "absent/", 86400, 0)
        .await?;

    assert!(present.exists, "expected 1 object under 'present/'");
    assert!(!missing.exists, "expected 0 objects under 'absent/'");

    Ok(())
}

/// Two independent buckets are monitored separately.
#[tokio::test]
async fn multiple_buckets_independent() -> anyhow::Result<()> {
    if !helpers::has_container_runtime() {
        return Ok(());
    }
    let env = helpers::start_minio().await?;
    env.create_bucket("bucket-alpha").await?;
    env.create_bucket("bucket-beta").await?;

    env.put_object("bucket-alpha", "logs/app.log", b"log entry")
        .await?;
    env.put_object("bucket-beta", "backups/db.dump", b"backup content")
        .await?;

    let alpha = env
        .monitor
        .check_storage("bucket-alpha", "logs/", 86400, 0)
        .await?;
    let beta = env
        .monitor
        .check_storage("bucket-beta", "backups/", 86400, 0)
        .await?;

    assert!(alpha.exists, "expected 1 object in bucket-alpha");
    assert!(beta.exists, "expected 1 object in bucket-beta");

    Ok(())
}

#[tokio::test]
async fn execute_monitor_missing_prefix_is_ok_by_default() -> anyhow::Result<()> {
    if !helpers::has_container_runtime() {
        return Ok(());
    }
    let env = helpers::start_minio().await?;
    env.create_bucket("exec-default-ok").await?;

    let result = helpers::execute_monitor(
        &env,
        r#"---
s3mon:
  endpoint: __ENDPOINT__
  region: us-east-1
  access_key: minioadmin
  secret_key: minioadmin
  buckets:
    exec-default-ok:
      - prefix: missing/
        age: 86400
"#,
        false,
    )
    .await;

    assert!(result.is_ok(), "missing objects should not fail by default");

    Ok(())
}

#[tokio::test]
async fn execute_monitor_missing_prefix_fails_with_exit_on_check_failure() -> anyhow::Result<()> {
    if !helpers::has_container_runtime() {
        return Ok(());
    }
    let env = helpers::start_minio().await?;
    env.create_bucket("exec-missing-prefix").await?;

    let result = helpers::execute_monitor(
        &env,
        r#"---
s3mon:
  endpoint: __ENDPOINT__
  region: us-east-1
  access_key: minioadmin
  secret_key: minioadmin
  buckets:
    exec-missing-prefix:
      - prefix: missing/
        age: 86400
"#,
        true,
    )
    .await;

    assert!(
        result.is_err(),
        "missing objects should fail with the exit flag"
    );
    assert_eq!(
        result.err().map(|err| err.to_string()),
        Some("one or more checks failed".to_string())
    );

    Ok(())
}

#[tokio::test]
async fn execute_monitor_missing_bucket_fails_with_exit_on_check_failure() -> anyhow::Result<()> {
    if !helpers::has_container_runtime() {
        return Ok(());
    }
    let env = helpers::start_minio().await?;

    let result = helpers::execute_monitor(
        &env,
        r#"---
s3mon:
  endpoint: __ENDPOINT__
  region: us-east-1
  access_key: minioadmin
  secret_key: minioadmin
  buckets:
    bucket-does-not-exist:
      - prefix: missing/
        age: 86400
"#,
        true,
    )
    .await;

    assert!(
        result.is_err(),
        "S3 API errors should fail with the exit flag"
    );
    assert_eq!(
        result.err().map(|err| err.to_string()),
        Some("one or more checks failed".to_string())
    );

    Ok(())
}
