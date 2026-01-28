// enterprise-system-tests/tests/helpers/infra.rs
// ============================================================================
// Module: Enterprise Test Infrastructure
// Description: Postgres and S3 fixtures for enterprise system-tests.
// Purpose: Provide isolated storage backends for integration coverage.
// Dependencies: testcontainers, aws-sdk-s3
// ============================================================================

use std::env;
use std::process::Command;
use std::time::Duration;

use aws_config::BehaviorVersion;
use aws_config::Region;
use aws_sdk_s3::Client;
use aws_sdk_s3::types::ServerSideEncryption;
use decision_gate_store_enterprise::postgres_store::PostgresStore;
use decision_gate_store_enterprise::postgres_store::PostgresStoreConfig;
use once_cell::sync::Lazy;
use testcontainers::Container;
use testcontainers::GenericImage;
use testcontainers::RunnableImage;
use testcontainers::clients::Cli;

static DOCKER: Lazy<Cli> = Lazy::new(Cli::default);

pub struct PostgresFixture {
    pub url: String,
    _container: Option<Container<'static, GenericImage>>,
}

impl PostgresFixture {
    pub fn start() -> Result<Self, String> {
        if let Ok(url) = env::var("DECISION_GATE_ENTERPRISE_PG_URL") {
            return Ok(Self {
                url,
                _container: None,
            });
        }
        ensure_docker_available()?;
        let image = GenericImage::new("postgres", "15-alpine")
            .with_env_var("POSTGRES_USER", "dg")
            .with_env_var("POSTGRES_PASSWORD", "dg")
            .with_env_var("POSTGRES_DB", "decision_gate")
            .with_exposed_port(5432);
        let container = DOCKER.run(image);
        let port = container.get_host_port_ipv4(5432);
        let url = format!("postgres://dg:dg@127.0.0.1:{port}/decision_gate");
        Ok(Self {
            url,
            _container: Some(container),
        })
    }
}

pub struct S3Fixture {
    pub endpoint: String,
    pub bucket: String,
    pub region: String,
    pub access_key: String,
    pub secret_key: String,
    pub force_path_style: bool,
    _container: Option<Container<'static, GenericImage>>,
}

impl S3Fixture {
    pub async fn start() -> Result<Self, String> {
        if let (Ok(endpoint), Ok(bucket)) = (
            env::var("DECISION_GATE_ENTERPRISE_S3_ENDPOINT"),
            env::var("DECISION_GATE_ENTERPRISE_S3_BUCKET"),
        ) {
            let region = env::var("DECISION_GATE_ENTERPRISE_S3_REGION")
                .unwrap_or_else(|_| "us-east-1".to_string());
            let access_key = env::var("DECISION_GATE_ENTERPRISE_S3_ACCESS_KEY")
                .unwrap_or_else(|_| "minioadmin".to_string());
            let secret_key = env::var("DECISION_GATE_ENTERPRISE_S3_SECRET_KEY")
                .unwrap_or_else(|_| "minioadmin".to_string());
            return Ok(Self {
                endpoint,
                bucket,
                region,
                access_key,
                secret_key,
                force_path_style: true,
                _container: None,
            });
        }

        ensure_docker_available()?;
        let access_key = "minioadmin".to_string();
        let secret_key = "minioadmin".to_string();
        let region = "us-east-1".to_string();
        let bucket = "decision-gate-test".to_string();
        let kms_secret_key = "minio-kms:MDEyMzQ1Njc4OWFiY2RlZjAxMjM0NTY3ODlhYmNkZWY=".to_string();
        let image = GenericImage::new("minio/minio", "latest")
            .with_env_var("MINIO_ROOT_USER", access_key.clone())
            .with_env_var("MINIO_ROOT_PASSWORD", secret_key.clone())
            .with_env_var("MINIO_REGION", region.clone())
            .with_env_var("MINIO_OBJECT_LOCK", "on")
            .with_env_var("MINIO_KMS_SECRET_KEY", kms_secret_key.clone())
            .with_env_var("MINIO_KMS_AUTO_ENCRYPTION", "on")
            .with_exposed_port(9000)
            .with_entrypoint("/usr/bin/minio");
        let args = vec![
            "server".to_string(),
            "/data".to_string(),
            "--console-address".to_string(),
            ":9001".to_string(),
        ];
        let container = DOCKER.run(RunnableImage::from((image, args)));
        let port = container.get_host_port_ipv4(9000);
        let endpoint = format!("http://127.0.0.1:{port}");

        let fixture = Self {
            endpoint: endpoint.clone(),
            bucket: bucket.clone(),
            region,
            access_key,
            secret_key,
            force_path_style: true,
            _container: Some(container),
        };
        fixture.seed_bucket().await?;
        Ok(fixture)
    }

    pub async fn client(&self) -> Result<Client, String> {
        super::env::set_var("AWS_EC2_METADATA_DISABLED", "true");
        let config = aws_config::defaults(BehaviorVersion::latest())
            .region(Region::new(self.region.clone()))
            .endpoint_url(self.endpoint.clone())
            .credentials_provider(aws_sdk_s3::config::Credentials::new(
                self.access_key.clone(),
                self.secret_key.clone(),
                None,
                None,
                "enterprise-system-tests",
            ))
            .load()
            .await;
        let mut builder = aws_sdk_s3::config::Builder::from(&config);
        if self.force_path_style {
            builder = builder.force_path_style(true);
        }
        Ok(Client::from_conf(builder.build()))
    }

    async fn seed_bucket(&self) -> Result<(), String> {
        let client = self.client().await?;
        let _ = client
            .create_bucket()
            .bucket(self.bucket.clone())
            .object_lock_enabled_for_bucket(true)
            .send()
            .await;
        Ok(())
    }
}

pub fn wait_for_postgres_blocking(url: &str) -> Result<(), String> {
    let start = std::time::Instant::now();
    let timeout = Duration::from_secs(30);
    let mut last_error = "unknown error".to_string();
    loop {
        if start.elapsed() > timeout {
            return Err(format!("postgres readiness timeout: {last_error}"));
        }
        match postgres::Client::connect(url, postgres::NoTls) {
            Ok(mut client) => match client.simple_query("SELECT 1") {
                Ok(_) => return Ok(()),
                Err(err) => {
                    last_error = err.to_string();
                }
            },
            Err(err) => {
                last_error = err.to_string();
            }
        }
        std::thread::sleep(Duration::from_millis(100));
    }
}

pub async fn ensure_bucket_policy_enforces_sse(
    client: &Client,
    bucket: &str,
) -> Result<(), String> {
    let policy = format!(
        "{{\"Version\":\"2012-10-17\",\"Statement\":[{{\"Sid\":\"RequireSSE\",\"Effect\":\"Deny\",\
         \"Principal\":\"*\",\"Action\":\"s3:PutObject\",\"Resource\":\"arn:aws:s3:::{bucket}/*\",\
         \"Condition\":{{\"StringNotEquals\":{{\"s3:x-amz-server-side-encryption\":\"AES256\"\
         }}}}}}]}}"
    );
    client
        .put_bucket_policy()
        .bucket(bucket)
        .policy(policy)
        .send()
        .await
        .map_err(|err| format!("failed to set bucket policy: {err}"))?;
    Ok(())
}

pub async fn head_object_sse(
    client: &Client,
    bucket: &str,
    key: &str,
) -> Result<Option<ServerSideEncryption>, String> {
    let output = client
        .head_object()
        .bucket(bucket)
        .key(key)
        .send()
        .await
        .map_err(|err| format!("head_object failed: {err}"))?;
    Ok(output.server_side_encryption().cloned())
}

fn ensure_docker_available() -> Result<(), String> {
    let output = Command::new("docker")
        .args(["version", "--format", "{{.Server.Version}}"])
        .output()
        .map_err(|err| match err.kind() {
            std::io::ErrorKind::NotFound => {
                "docker CLI not found in PATH; install Docker Desktop or provide external services \
                 via DECISION_GATE_ENTERPRISE_PG_URL / DECISION_GATE_ENTERPRISE_S3_*"
                    .to_string()
            }
            _ => format!("failed to run docker CLI: {err}"),
        })?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!(
            "docker daemon unavailable (start Docker Desktop or configure external services via \
             DECISION_GATE_ENTERPRISE_PG_URL / DECISION_GATE_ENTERPRISE_S3_*). stderr: {stderr}"
        ));
    }
    Ok(())
}

pub fn io_error(err: impl std::fmt::Display) -> std::io::Error {
    std::io::Error::new(std::io::ErrorKind::Other, err.to_string())
}

fn join_thread<T>(join: std::thread::JoinHandle<T>) -> Result<T, std::io::Error> {
    match tokio::runtime::Handle::try_current() {
        Ok(_) => tokio::task::block_in_place(|| join.join()),
        Err(_) => join.join(),
    }
    .map_err(|_| io_error("postgres worker panicked"))
}

pub async fn with_postgres_client<T, F>(url: &str, f: F) -> Result<T, std::io::Error>
where
    T: Send + 'static,
    F: FnOnce(&mut postgres::Client) -> Result<T, std::io::Error> + Send + 'static,
{
    let url = url.to_string();
    let join = std::thread::spawn(move || {
        let mut client = postgres::Client::connect(&url, postgres::NoTls).map_err(io_error)?;
        f(&mut client)
    });
    join_thread(join)?
}

pub async fn with_postgres_clients<T, F>(
    source_url: &str,
    dest_url: &str,
    f: F,
) -> Result<T, std::io::Error>
where
    T: Send + 'static,
    F: FnOnce(&mut postgres::Client, &mut postgres::Client) -> Result<T, std::io::Error>
        + Send
        + 'static,
{
    let source_url = source_url.to_string();
    let dest_url = dest_url.to_string();
    let join = std::thread::spawn(move || {
        let mut source =
            postgres::Client::connect(&source_url, postgres::NoTls).map_err(io_error)?;
        let mut dest = postgres::Client::connect(&dest_url, postgres::NoTls).map_err(io_error)?;
        f(&mut source, &mut dest)
    });
    join_thread(join)?
}

pub fn build_postgres_store_blocking(
    config: PostgresStoreConfig,
) -> Result<PostgresStore, std::io::Error> {
    let start = std::time::Instant::now();
    let timeout = Duration::from_secs(20);
    let mut last_error = "unknown error".to_string();
    loop {
        if start.elapsed() > timeout {
            return Err(io_error(format!("postgres store init timeout: {last_error}")));
        }
        match PostgresStore::new(&config) {
            Ok(store) => return Ok(store),
            Err(err) => {
                last_error = err.to_string();
                std::thread::sleep(Duration::from_millis(200));
            }
        }
    }
}

pub async fn build_postgres_store(
    config: PostgresStoreConfig,
) -> Result<PostgresStore, std::io::Error> {
    let join = std::thread::spawn(move || build_postgres_store_blocking(config));
    join_thread(join)?
}

pub async fn wait_for_postgres(url: &str) -> Result<(), String> {
    let url = url.to_string();
    let join = std::thread::spawn(move || wait_for_postgres_blocking(&url));
    let result = join_thread(join).map_err(|err| err.to_string())?;
    result
}
