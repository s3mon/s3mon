#![allow(clippy::pedantic)]

mod helpers;

/// A freshly uploaded object is visible and not age-expired.
#[tokio::test]
#[ignore = "requires Docker or Podman (run with --include-ignored)"]
async fn object_exists_and_fresh() -> anyhow::Result<()> {
    let env = helpers::start_minio().await?;
    env.create_bucket("test-fresh").await?;
    env.put_object("test-fresh", "data/file.txt", b"hello world")
        .await?;

    let objects = env.monitor.objects("test-fresh", "data/", 86400).await?;
    assert_eq!(objects.len(), 1, "expected exactly 1 fresh object");

    Ok(())
}

/// With age=0 the cutoff equals now, so any object already stored is considered expired.
#[tokio::test]
#[ignore = "requires Docker or Podman (run with --include-ignored)"]
async fn object_exists_age_expired() -> anyhow::Result<()> {
    let env = helpers::start_minio().await?;
    env.create_bucket("test-expired").await?;
    env.put_object("test-expired", "data/file.txt", b"hello world")
        .await?;

    let objects = env.monitor.objects("test-expired", "data/", 0).await?;
    assert!(objects.is_empty(), "expected no objects with age=0");

    Ok(())
}

/// An object whose size is below the configured minimum triggers a size mismatch.
#[tokio::test]
#[ignore = "requires Docker or Podman (run with --include-ignored)"]
async fn object_size_below_threshold() -> anyhow::Result<()> {
    let env = helpers::start_minio().await?;
    env.create_bucket("test-size").await?;
    // 11 bytes – intentionally smaller than the 1 024-byte threshold
    env.put_object("test-size", "data/file.txt", b"hello world")
        .await?;

    let objects = env.monitor.objects("test-size", "data/", 86400).await?;
    assert_eq!(objects.len(), 1, "expected 1 object");

    let first = objects
        .first()
        .ok_or_else(|| anyhow::anyhow!("no objects returned"))?;
    let actual_size = first
        .size()
        .ok_or_else(|| anyhow::anyhow!("object has no size metadata"))?;

    assert!(
        actual_size < 1024,
        "size should be below threshold (got {actual_size})"
    );

    Ok(())
}

/// A bucket that exists but contains no matching keys reports zero objects.
#[tokio::test]
#[ignore = "requires Docker or Podman (run with --include-ignored)"]
async fn prefix_not_found() -> anyhow::Result<()> {
    let env = helpers::start_minio().await?;
    env.create_bucket("test-empty").await?;

    let objects = env
        .monitor
        .objects("test-empty", "missing/prefix/", 86400)
        .await?;
    assert!(objects.is_empty(), "expected no objects for missing prefix");

    Ok(())
}

/// Two prefixes in the same bucket: one populated, one empty.
#[tokio::test]
#[ignore = "requires Docker or Podman (run with --include-ignored)"]
async fn multiple_prefixes_one_missing() -> anyhow::Result<()> {
    let env = helpers::start_minio().await?;
    env.create_bucket("test-multi-prefix").await?;
    env.put_object("test-multi-prefix", "present/data.bin", b"payload")
        .await?;

    let present = env
        .monitor
        .objects("test-multi-prefix", "present/", 86400)
        .await?;
    let missing = env
        .monitor
        .objects("test-multi-prefix", "absent/", 86400)
        .await?;

    assert_eq!(present.len(), 1, "expected 1 object under 'present/'");
    assert!(missing.is_empty(), "expected 0 objects under 'absent/'");

    Ok(())
}

/// Two independent buckets are monitored separately.
#[tokio::test]
#[ignore = "requires Docker or Podman (run with --include-ignored)"]
async fn multiple_buckets_independent() -> anyhow::Result<()> {
    let env = helpers::start_minio().await?;
    env.create_bucket("bucket-alpha").await?;
    env.create_bucket("bucket-beta").await?;

    env.put_object("bucket-alpha", "logs/app.log", b"log entry")
        .await?;
    env.put_object("bucket-beta", "backups/db.dump", b"backup content")
        .await?;

    let alpha = env.monitor.objects("bucket-alpha", "logs/", 86400).await?;
    let beta = env
        .monitor
        .objects("bucket-beta", "backups/", 86400)
        .await?;

    assert_eq!(alpha.len(), 1, "expected 1 object in bucket-alpha");
    assert_eq!(beta.len(), 1, "expected 1 object in bucket-beta");

    Ok(())
}
