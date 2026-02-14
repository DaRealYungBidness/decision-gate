// system-tests/tests/helpers/infra.rs
// ============================================================================
// Module: System Test Infrastructure
// Description: S3 fixtures for object-store system-tests.
// Purpose: Provide isolated object storage for runpack export verification.
// Dependencies: testcontainers, aws-sdk-s3
// ============================================================================

//! ## Overview
//! S3 fixtures for object-store system-tests.
//! Purpose: Provide isolated object storage for runpack export verification.
//! Invariants:
//! - System-test execution is deterministic and fail-closed.
//! - Inputs are treated as untrusted unless explicitly mocked.
//!
//! Security posture: system-test inputs are untrusted; see `Docs/security/threat_model.md`.

use std::env;

use aws_config::BehaviorVersion;
use aws_config::Region;
use aws_sdk_s3::Client;
use testcontainers::ContainerAsync;
use testcontainers::GenericImage;
use testcontainers::ImageExt;
use testcontainers::core::IntoContainerPort;
use testcontainers::runners::AsyncRunner;

pub struct S3Fixture {
    pub endpoint: String,
    pub bucket: String,
    pub region: String,
    pub access_key: String,
    pub secret_key: String,
    pub force_path_style: bool,
    _container: Option<ContainerAsync<GenericImage>>,
}

impl S3Fixture {
    pub async fn start() -> Result<Self, String> {
        if let (Ok(endpoint), Ok(bucket)) = (
            env::var("DECISION_GATE_SYSTEM_S3_ENDPOINT"),
            env::var("DECISION_GATE_SYSTEM_S3_BUCKET"),
        ) {
            let region = env::var("DECISION_GATE_SYSTEM_S3_REGION")
                .unwrap_or_else(|_| "us-east-1".to_string());
            let access_key = env::var("DECISION_GATE_SYSTEM_S3_ACCESS_KEY")
                .unwrap_or_else(|_| "minioadmin".to_string());
            let secret_key = env::var("DECISION_GATE_SYSTEM_S3_SECRET_KEY")
                .unwrap_or_else(|_| "minioadmin".to_string());
            let force_path_style = env::var("DECISION_GATE_SYSTEM_S3_FORCE_PATH_STYLE").is_ok();
            let fixture = Self {
                endpoint,
                bucket,
                region,
                access_key,
                secret_key,
                force_path_style,
                _container: None,
            };
            fixture.seed_bucket().await?;
            return Ok(fixture);
        }

        ensure_docker_available()?;
        let access_key = "minioadmin".to_string();
        let secret_key = "minioadmin".to_string();
        let region = "us-east-1".to_string();
        let bucket = "decision-gate-system-tests".to_string();
        let args = vec![
            "server".to_string(),
            "/data".to_string(),
            "--console-address".to_string(),
            ":9001".to_string(),
        ];
        let container = GenericImage::new("minio/minio", "latest")
            .with_exposed_port(9000.tcp())
            .with_entrypoint("/usr/bin/minio")
            .with_env_var("MINIO_ROOT_USER", access_key.clone())
            .with_env_var("MINIO_ROOT_PASSWORD", secret_key.clone())
            .with_env_var("MINIO_REGION", region.clone())
            .with_cmd(args)
            .start()
            .await
            .map_err(|err| format!("failed to start minio container: {err}"))?;
        let port = container
            .get_host_port_ipv4(9000.tcp())
            .await
            .map_err(|err| format!("failed to resolve minio port: {err}"))?;
        let endpoint = format!("http://127.0.0.1:{port}");
        let fixture = Self {
            endpoint,
            bucket,
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
                "system-tests",
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
        let client: Client = self.client().await?;
        let _ = client.create_bucket().bucket(self.bucket.clone()).send().await;
        Ok(())
    }
}

fn ensure_docker_available() -> Result<(), String> {
    let output = std::process::Command::new("docker")
        .arg("info")
        .output()
        .map_err(|err| format!("docker info failed: {err}"))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("docker info failed: {stderr}"));
    }
    Ok(())
}
