#![allow(clippy::pedantic)]
#![allow(dead_code)]

use aws_smithy_types::byte_stream::ByteStream;
use s3mon::{
    cli::actions::{self, Action},
    config::{Config, Data},
    output::OutputFormat,
    s3::Monitor,
};
use std::collections::BTreeMap;
use std::sync::OnceLock;
use testcontainers::{ContainerAsync, runners::AsyncRunner};
use testcontainers_modules::minio::MinIO;

/// Holds a running MinIO container and a `Monitor` wired to it.
/// The container is stopped when this value is dropped.
pub struct MinioEnv {
    pub monitor: Monitor,
    pub endpoint: String,
    _container: ContainerAsync<MinIO>,
}

/// Detect the available container runtime and configure `DOCKER_HOST` /
/// `TESTCONTAINERS_RYUK_DISABLED` accordingly.  Runs at most once per
/// process (guarded by `OnceLock`).
///
/// Priority:
///   1. `DOCKER_HOST` already set in the environment → leave it alone.
///   2. `/var/run/docker.sock` present → Docker daemon running (e.g. GitHub
///      Actions); testcontainers picks this up automatically, nothing to do.
///   3. `$XDG_RUNTIME_DIR/podman/podman.sock` present → rootless Podman.
///   4. `/var/run/podman/podman.sock` present → rootful Podman.
///
/// For Podman we also set `TESTCONTAINERS_RYUK_DISABLED=true` because the
/// Ryuk reaper container requires privileged access that rootless Podman
/// does not grant.  Cleanup still happens via the `Drop` on `ContainerAsync`.
fn configure_container_runtime() {
    static INIT: OnceLock<()> = OnceLock::new();
    INIT.get_or_init(|| {
        // 1. Already explicitly configured.
        if std::env::var("DOCKER_HOST").is_ok() {
            return;
        }

        // 2. Docker socket — testcontainers default, nothing to set.
        if std::path::Path::new("/var/run/docker.sock").exists() {
            return;
        }

        // 3. Rootless Podman via XDG_RUNTIME_DIR.
        if let Ok(xdg) = std::env::var("XDG_RUNTIME_DIR") {
            let socket = format!("{xdg}/podman/podman.sock");
            if std::path::Path::new(&socket).exists() {
                // SAFETY: single-threaded initialisation guarded by OnceLock;
                // no other thread reads these vars concurrently.
                unsafe {
                    std::env::set_var("DOCKER_HOST", format!("unix://{socket}"));
                    std::env::set_var("TESTCONTAINERS_RYUK_DISABLED", "true");
                }
                return;
            }
        }

        // 4. Rootful Podman.
        if std::path::Path::new("/var/run/podman/podman.sock").exists() {
            // SAFETY: same as above.
            unsafe {
                std::env::set_var("DOCKER_HOST", "unix:///var/run/podman/podman.sock");
                std::env::set_var("TESTCONTAINERS_RYUK_DISABLED", "true");
            }
        }
    });
}

/// Returns true if a Docker/Podman runtime appears to be available.
pub fn has_container_runtime() -> bool {
    if std::env::var("DOCKER_HOST").is_ok() {
        return true;
    }
    if std::path::Path::new("/var/run/docker.sock").exists() {
        return true;
    }
    if let Ok(xdg) = std::env::var("XDG_RUNTIME_DIR")
        && std::path::Path::new(&format!("{xdg}/podman/podman.sock")).exists()
    {
        return true;
    }
    if std::path::Path::new("/var/run/podman/podman.sock").exists() {
        return true;
    }
    false
}

/// Start a fresh MinIO container and return a [`MinioEnv`] ready for use.
pub async fn start_minio() -> anyhow::Result<MinioEnv> {
    configure_container_runtime();

    let container = MinIO::default().start().await?;
    let port = container.get_host_port_ipv4(9000).await?;
    let endpoint = format!("http://127.0.0.1:{port}");

    let config = Config {
        s3mon: Data {
            endpoint: endpoint.clone(),
            region: "us-east-1".to_string(),
            access_key: "minioadmin".to_string(),
            secret_key: "minioadmin".to_string(),
            buckets: BTreeMap::new(),
        },
    };

    let monitor = Monitor::new(&config).await?;

    Ok(MinioEnv {
        monitor,
        endpoint,
        _container: container,
    })
}

impl MinioEnv {
    /// Create a bucket in the running MinIO instance.
    pub async fn create_bucket(&self, name: &str) -> anyhow::Result<()> {
        self.monitor.s3.create_bucket().bucket(name).send().await?;
        Ok(())
    }

    /// Upload a static byte slice as an object.
    pub async fn put_object(
        &self,
        bucket: &str,
        key: &str,
        body: &'static [u8],
    ) -> anyhow::Result<()> {
        self.monitor
            .s3
            .put_object()
            .bucket(bucket)
            .key(key)
            .body(ByteStream::from_static(body))
            .send()
            .await?;
        Ok(())
    }
}

struct TempConfigFile {
    path: std::path::PathBuf,
}

impl TempConfigFile {
    fn new(contents: &str) -> anyhow::Result<Self> {
        let path = std::env::temp_dir().join(format!(
            "s3mon-test-{}-{}.yml",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)?
                .as_nanos()
        ));
        std::fs::write(&path, contents)?;
        Ok(Self { path })
    }

    fn path(&self) -> &std::path::Path {
        &self.path
    }
}

impl Drop for TempConfigFile {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
    }
}

pub async fn execute_monitor(
    env: &MinioEnv,
    config_body: &str,
    exit_on_check_failure: bool,
) -> anyhow::Result<()> {
    let config_file = TempConfigFile::new(&config_body.replace("__ENDPOINT__", &env.endpoint))?;

    let action = Action::Monitor {
        config: config_file.path().to_path_buf(),
        format: OutputFormat::Prometheus,
        exit_on_check_failure,
    };

    actions::run::execute(&action).await
}
