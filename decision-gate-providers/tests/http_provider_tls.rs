// decision-gate-providers/tests/http_provider_tls.rs
// ============================================================================
// Module: HTTP Provider TLS Guardrail Tests
// Description: Ensure TLS validation fails closed for invalid certificates.
// Purpose: Guard against accidental trust relaxation in HTTP provider.
// Threat Models: TM-HTTP-002 (TLS)
// ============================================================================

//! TLS guardrail tests for the HTTP provider.

#![allow(
    clippy::panic,
    clippy::print_stdout,
    clippy::print_stderr,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::use_debug,
    clippy::dbg_macro,
    clippy::panic_in_result_fn,
    clippy::unwrap_in_result,
    reason = "Test-only assertions and helpers are permitted."
)]

mod common;

use std::collections::BTreeSet;
use std::io::Read;
use std::io::Write;
use std::net::TcpListener;
use std::sync::Arc;
use std::thread;

use decision_gate_core::EvidenceProvider;
use decision_gate_core::EvidenceQuery;
use decision_gate_core::ProviderId;
use decision_gate_providers::HttpProvider;
use decision_gate_providers::HttpProviderConfig;
use rcgen::generate_simple_self_signed;
use rustls::ServerConfig;
use rustls::ServerConnection;
use rustls::StreamOwned;
use rustls::pki_types::CertificateDer;
use rustls::pki_types::PrivateKeyDer;
use rustls::pki_types::PrivatePkcs8KeyDer;
use serde_json::json;

use crate::common::sample_context;

fn start_tls_server() -> (std::net::SocketAddr, thread::JoinHandle<()>) {
    let _ = rustls::crypto::aws_lc_rs::default_provider().install_default();
    let rcgen::CertifiedKey {
        cert,
        signing_key,
    } = generate_simple_self_signed(vec!["localhost".to_string()]).unwrap();
    let cert_der = CertificateDer::from(cert);
    let key_der = PrivateKeyDer::from(PrivatePkcs8KeyDer::from(signing_key.serialize_der()));
    let config = ServerConfig::builder()
        .with_no_client_auth()
        .with_single_cert(vec![cert_der], key_der)
        .unwrap();
    let config = Arc::new(config);

    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    let handle = thread::spawn(move || {
        if let Ok((tcp, _)) = listener.accept() {
            let conn = ServerConnection::new(config).unwrap();
            let mut stream = StreamOwned::new(conn, tcp);
            let mut buf = [0u8; 1024];
            let _ = stream.read(&mut buf);
            let _ = stream.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\nOK");
            let _ = stream.flush();
        }
    });

    (addr, handle)
}

#[test]
fn http_tls_rejects_self_signed_cert() {
    let (addr, handle) = start_tls_server();
    let url = format!("https://localhost:{}", addr.port());

    let mut allowed_hosts = BTreeSet::new();
    allowed_hosts.insert("localhost".to_string());
    let provider = HttpProvider::new(HttpProviderConfig {
        allowed_hosts: Some(allowed_hosts),
        timeout_ms: 2_000,
        ..HttpProviderConfig::default()
    })
    .unwrap();

    let query = EvidenceQuery {
        provider_id: ProviderId::new("http"),
        check_id: "status".to_string(),
        params: Some(json!({"url": url})),
    };

    let result = provider.query(&query, &sample_context());
    handle.join().unwrap();

    assert!(result.is_err(), "self-signed cert should be rejected");
    let err = format!("{:?}", result.unwrap_err());
    assert!(
        err.contains("http request failed") || err.contains("certificate") || err.contains("tls"),
        "unexpected error: {err}"
    );
}
