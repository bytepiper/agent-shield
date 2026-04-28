use anyhow::Result;
use bytes::Bytes;
use http::request::Parts;
use hyper::Request;
use hyper::Response;
use hyper_util::rt::TokioIo;
use rustls::pki_types::ServerName;
use std::sync::Arc;
use tokio::net::TcpStream;
use tokio_rustls::TlsConnector;

pub(crate) async fn send_https(
    domain: &str,
    port: u16,
    parts: Parts,
    body_bytes: &[u8],
) -> Result<Response<hyper::body::Incoming>> {
    let tcp = TcpStream::connect(format!("{domain}:{port}")).await?;
    let mut roots = rustls::RootCertStore::empty();
    for cert in rustls_native_certs::load_native_certs().certs {
        let _ = roots.add(cert);
    }
    let client_config = rustls::ClientConfig::builder()
        .with_root_certificates(roots)
        .with_no_client_auth();
    let connector = TlsConnector::from(Arc::new(client_config));
    let server_name = ServerName::try_from(domain.to_string())?;
    let tls = connector.connect(server_name, tcp).await?;

    let path_and_query = parts
        .uri
        .path_and_query()
        .map(|pq| pq.as_str())
        .unwrap_or("/");
    let mut builder = Request::builder()
        .method(parts.method)
        .uri(format!("https://{domain}:{port}{path_and_query}"));
    for (key, value) in &parts.headers {
        if key
            .as_str()
            .eq_ignore_ascii_case("sec-websocket-extensions")
        {
            continue;
        }
        builder = builder.header(key, value);
    }
    let upstream_req = builder.body(http_body_util::Full::new(Bytes::copy_from_slice(
        body_bytes,
    )))?;

    let (mut tx, conn) = hyper::client::conn::http1::handshake(TokioIo::new(tls)).await?;
    tokio::spawn(async move {
        let _ = conn.with_upgrades().await;
    });
    Ok(tx.send_request(upstream_req).await?)
}
