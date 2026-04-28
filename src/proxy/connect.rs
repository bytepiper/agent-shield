use anyhow::Result;
use bytes::Bytes;
use hyper::{Method, Request, Response, StatusCode};
use hyper_util::rt::TokioIo;
use std::sync::Arc;
use tokio::net::TcpStream;
use tracing::warn;

use crate::decision::{resolve_decision, synth_decision_response};
use crate::events::InterceptorEvent;
use crate::state::{domain_of, matches, State};
use crate::FullBody;

use super::{http, now_ts};

pub(crate) async fn handle(
    state: Arc<State>,
    req: Request<hyper::body::Incoming>,
) -> Result<Response<FullBody>> {
    if req.method() == Method::CONNECT {
        let host = req
            .uri()
            .authority()
            .map(|authority| authority.to_string())
            .unwrap_or_default();
        return connect(state, req, host).await;
    }
    Ok(Response::builder()
        .status(400)
        .body(Bytes::from("CONNECT only").into())?)
}

async fn connect(
    state: Arc<State>,
    req: Request<hyper::body::Incoming>,
    host: String,
) -> Result<Response<FullBody>> {
    let domain = domain_of(&host).to_string();
    let port: u16 = host
        .split(':')
        .nth(1)
        .and_then(|p| p.parse().ok())
        .unwrap_or(443);
    let session_id = state.id();

    if matches(&domain, &state.cfg.block)
        || (state.cfg.block_telemetry
            && (domain.contains("datadoghq.com") || domain.contains("sentry.io")))
    {
        state.log(InterceptorEvent {
            id: state.id(),
            ts: now_ts(),
            session_id: Some(session_id),
            phase: "connect.pre".into(),
            transport: "connect".into(),
            domain,
            action: "blocked".into(),
            ..Default::default()
        });
        return Ok(Response::builder()
            .status(403)
            .body(Bytes::from("blocked").into())?);
    }

    let connect_id = state.id();
    let mut connect_event = InterceptorEvent {
        id: connect_id,
        ts: now_ts(),
        session_id: Some(session_id),
        phase: "connect.pre".into(),
        transport: "connect".into(),
        domain: domain.clone(),
        action: "connect".into(),
        ..Default::default()
    };
    let connect_decision = resolve_decision(&state, &connect_event).await;
    if connect_decision.action == "block" {
        connect_event.decision_action = Some(connect_decision.action.clone());
        connect_event.decision_reason = connect_decision.reason.clone();
        state.log(connect_event);
        return synth_decision_response(
            &connect_decision,
            StatusCode::FORBIDDEN,
            "blocked by orchestrator",
        );
    }

    if matches(&domain, &state.cfg.pass) {
        connect_event.action = "passthrough".into();
        if connect_decision.action != "allow" {
            connect_event.decision_action = Some(connect_decision.action.clone());
            connect_event.decision_reason = connect_decision.reason.clone();
        }
        state.log(connect_event);
        tokio::spawn(async move {
            if let Ok(upgraded) = hyper::upgrade::on(req).await {
                state.log(InterceptorEvent {
                    id: state.id(),
                    ts: now_ts(),
                    session_id: Some(session_id),
                    phase: "session.open".into(),
                    transport: "tcp".into(),
                    direction: Some("out".into()),
                    domain: domain.clone(),
                    action: "session-open".into(),
                    ..Default::default()
                });
                if let Ok(mut server) = TcpStream::connect(format!("{domain}:{port}")).await {
                    let mut client = TokioIo::new(upgraded);
                    let _ = tokio::io::copy_bidirectional(&mut client, &mut server).await;
                }
                state.log(InterceptorEvent {
                    id: state.id(),
                    ts: now_ts(),
                    session_id: Some(session_id),
                    phase: "session.close".into(),
                    transport: "tcp".into(),
                    direction: Some("in".into()),
                    domain: domain.clone(),
                    action: "session-close".into(),
                    ..Default::default()
                });
            }
        });
        return Ok(Response::builder().status(200).body(Bytes::new().into())?);
    }

    connect_event.action = "mitm".into();
    if connect_decision.action != "allow" {
        connect_event.decision_action = Some(connect_decision.action.clone());
        connect_event.decision_reason = connect_decision.reason.clone();
    }
    state.log(connect_event);
    let state2 = state.clone();
    tokio::spawn(async move {
        if let Ok(upgraded) = hyper::upgrade::on(req).await {
            if let Err(err) = http::mitm(state2, upgraded, &domain, port, session_id).await {
                warn!("MITM {domain}: {err}");
            }
        }
    });
    Ok(Response::builder().status(200).body(Bytes::new().into())?)
}
