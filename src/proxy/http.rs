use anyhow::Result;
use hyper::server::conn::http1 as server_http1;
use hyper::service::service_fn;
use hyper::{Request, Response, StatusCode};
use hyper_util::rt::TokioIo;
use std::sync::Arc;
use tokio_rustls::TlsAcceptor;

use crate::events::InterceptorEvent;
use crate::state::State;
use crate::FullBody;

use super::now_ts;
use super::request::{prepare_request, RequestPrep};
use super::response::{handle_standard_response, ResponseContext};
use super::upgrade::{handle_switching_protocols, UpgradeContext};
use super::upstream::send_https;

pub(crate) async fn mitm(
    state: Arc<State>,
    upgraded: hyper::upgrade::Upgraded,
    domain: &str,
    port: u16,
    session_id: u64,
) -> Result<()> {
    let (certs, key) = state.ca.issue(domain)?;
    let server_config = rustls::ServerConfig::builder()
        .with_no_client_auth()
        .with_single_cert(certs, key)?;
    let acceptor = TlsAcceptor::from(Arc::new(server_config));
    let tls = acceptor.accept(TokioIo::new(upgraded)).await?;
    state.log(InterceptorEvent {
        id: state.id(),
        ts: now_ts(),
        session_id: Some(session_id),
        phase: "session.open".into(),
        transport: "tls".into(),
        direction: Some("out".into()),
        domain: domain.to_string(),
        action: "session-open".into(),
        ..Default::default()
    });

    let domain_owned = domain.to_string();
    let state2 = state.clone();
    server_http1::Builder::new()
        .preserve_header_case(true)
        .serve_connection(
            TokioIo::new(tls),
            service_fn(move |req| {
                let state = state2.clone();
                let domain = domain_owned.clone();
                async move { mitm_req(state, req, &domain, port, session_id).await }
            }),
        )
        .with_upgrades()
        .await?;
    state.log(InterceptorEvent {
        id: state.id(),
        ts: now_ts(),
        session_id: Some(session_id),
        phase: "session.close".into(),
        transport: "tls".into(),
        direction: Some("in".into()),
        domain: domain.to_string(),
        action: "session-close".into(),
        ..Default::default()
    });
    Ok(())
}

async fn mitm_req(
    state: Arc<State>,
    req: Request<hyper::body::Incoming>,
    domain: &str,
    port: u16,
    session_id: u64,
) -> Result<Response<FullBody>> {
    let request = match prepare_request(&state, req, domain, session_id).await? {
        RequestPrep::Proceed(request) => request,
        RequestPrep::Respond(resp) => return Ok(resp),
    };
    let id = request.id;
    let started_at = request.started_at;
    let client_upgrade = request.client_upgrade;
    let method = request.method;
    let path = request.path;
    let uri = request.uri;
    let parts = request.parts;
    let body_bytes = request.body_bytes;
    let req_size = request.req_size;
    let save_req = request.save_req;
    let req_headers = request.req_headers;
    let req_body_file = request.req_body_file;

    let resp = send_https(domain, port, parts, &body_bytes).await?;
    let status = resp.status().as_u16();

    if resp.status() == StatusCode::SWITCHING_PROTOCOLS {
        return handle_switching_protocols(
            state,
            resp,
            client_upgrade,
            UpgradeContext {
                id,
                session_id,
                domain: domain.to_string(),
                method,
                uri,
                status,
                req_size,
                req_headers,
                req_body_file,
                body_bytes,
                save_req,
            },
            started_at,
        )
        .await;
    }
    handle_standard_response(
        state,
        resp,
        ResponseContext {
            id,
            session_id,
            domain: domain.to_string(),
            method,
            path,
            uri,
            status,
            req_size,
            req_headers,
            req_body_file,
            save_req,
        },
        started_at,
    )
    .await
}
