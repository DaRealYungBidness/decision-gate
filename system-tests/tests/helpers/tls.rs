// system-tests/tests/helpers/tls.rs
// ============================================================================
// Module: TLS Test Fixtures
// Description: Generate ephemeral TLS assets for system-tests.
// Purpose: Avoid committing private keys while enabling TLS/mTLS coverage.
// Dependencies: rcgen, tempfile
// ============================================================================

//! ## Overview
//! Generate ephemeral TLS assets for system-tests.
//! Purpose: Avoid committing private keys while enabling TLS/mTLS coverage.
//! Invariants:
//! - System-test execution is deterministic and fail-closed.
//! - Inputs are treated as untrusted unless explicitly mocked.
//! Security posture: system-test inputs are untrusted; see `Docs/security/threat_model.md`.

use std::fs;
use std::path::PathBuf;

use rcgen::BasicConstraints;
use rcgen::Certificate;
use rcgen::CertificateParams;
use rcgen::DistinguishedName;
use rcgen::DnType;
use rcgen::IsCa;
use rcgen::Issuer;
use rcgen::KeyPair;
use tempfile::TempDir;

pub struct GeneratedTls {
    _tempdir: TempDir,
    pub ca_pem: PathBuf,
    pub server_cert: PathBuf,
    pub server_key: PathBuf,
    pub client_identity: PathBuf,
}

pub fn generate_tls_fixtures() -> Result<GeneratedTls, Box<dyn std::error::Error>> {
    let tempdir = tempfile::Builder::new().prefix("dg-tls").tempdir()?;
    let (ca, issuer) = generate_ca()?;
    let (server, server_key_pair) = generate_server_cert(&issuer)?;
    let (client, client_key_pair) = generate_client_cert(&issuer)?;

    let ca_pem = tempdir.path().join("ca.pem");
    let server_cert = tempdir.path().join("server.pem");
    let server_key_path = tempdir.path().join("server.key");
    let client_identity = tempdir.path().join("client.identity.pem");

    fs::write(&ca_pem, ca.pem())?;
    fs::write(&server_cert, server.pem())?;
    fs::write(&server_key_path, server_key_pair.serialize_pem())?;

    let client_cert = client.pem();
    let client_key = client_key_pair.serialize_pem();
    let mut identity = String::with_capacity(client_cert.len() + client_key.len());
    identity.push_str(&client_cert);
    identity.push_str(&client_key);
    fs::write(&client_identity, identity)?;

    Ok(GeneratedTls {
        _tempdir: tempdir,
        ca_pem,
        server_cert,
        server_key: server_key_path,
        client_identity,
    })
}

fn generate_ca() -> Result<(Certificate, Issuer<'static, KeyPair>), Box<dyn std::error::Error>> {
    let key = KeyPair::generate()?;
    let mut params = CertificateParams::default();
    params.is_ca = IsCa::Ca(BasicConstraints::Unconstrained);
    params.distinguished_name = distinguished_name("Decision Gate Test CA");
    let cert = params.self_signed(&key)?;
    let issuer = Issuer::new(params, key);
    Ok((cert, issuer))
}

fn generate_server_cert(
    issuer: &Issuer<'_, KeyPair>,
) -> Result<(Certificate, KeyPair), Box<dyn std::error::Error>> {
    let key = KeyPair::generate()?;
    let mut params =
        CertificateParams::new(vec!["localhost".to_string(), "127.0.0.1".to_string()])?;
    params.distinguished_name = distinguished_name("Decision Gate Test Server");
    params.is_ca = IsCa::NoCa;
    let cert = params.signed_by(&key, issuer)?;
    Ok((cert, key))
}

fn generate_client_cert(
    issuer: &Issuer<'_, KeyPair>,
) -> Result<(Certificate, KeyPair), Box<dyn std::error::Error>> {
    let key = KeyPair::generate()?;
    let mut params = CertificateParams::default();
    params.distinguished_name = distinguished_name("Decision Gate Test Client");
    params.is_ca = IsCa::NoCa;
    let cert = params.signed_by(&key, issuer)?;
    Ok((cert, key))
}

fn distinguished_name(common_name: &str) -> DistinguishedName {
    let mut name = DistinguishedName::new();
    name.push(DnType::CommonName, common_name);
    name
}
