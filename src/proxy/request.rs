use anyhow::Result;
use http_body_util::BodyExt;
use hyper::{Request, Response, StatusCode};
use std::time::Instant;

use crate::decision::{
    apply_decision_headers, decision_body_bytes, resolve_decision, synth_decision_response,
};
use crate::events::{inline_body, HeaderKv, InterceptorEvent};
use crate::scanner::Scanner;
use crate::state::{content_type, headers_to_pairs, should_save_request, State};
use crate::FullBody;

use super::now_ts;

pub(crate) struct PreparedRequest {
    pub id: u64,
    pub started_at: Instant,
    pub client_upgrade: hyper::upgrade::OnUpgrade,
    pub method: String,
    pub path: String,
    pub uri: String,
    pub parts: http::request::Parts,
    pub body_bytes: Vec<u8>,
    pub req_size: usize,
    pub save_req: bool,
    pub req_headers: Vec<HeaderKv>,
    pub req_body_file: Option<String>,
}

pub(crate) enum RequestPrep {
    Proceed(PreparedRequest),
    Respond(Response<FullBody>),
}

pub(crate) async fn prepare_request(
    state: &std::sync::Arc<State>,
    mut req: Request<hyper::body::Incoming>,
    domain: &str,
    session_id: u64,
) -> Result<RequestPrep> {
    let id = state.id();
    let started_at = Instant::now();
    let client_upgrade = hyper::upgrade::on(&mut req);
    let method = req.method().to_string();
    let path = req.uri().path().to_string();
    let uri = req.uri().to_string();
    let (mut parts, body) = req.into_parts();
    let req_headers = headers_to_pairs(&parts.headers);
    let body_bytes = body.collect().await?.to_bytes().to_vec();
    let req_content_type = content_type(&parts.headers);
    let decoded_body = State::decomp(&body_bytes);
    let req_size = body_bytes.len();
    let save_req = should_save_request(&method, &path, &parts.headers, req_size);

    let mut req_body_file = None;
    if save_req {
        req_body_file = Some(state.save(id, "req", &body_bytes));
    }

    let mut alerts = Vec::new();
    if state.cfg.scan_secrets && method == "POST" {
        alerts = state.scanner.scan(&State::decomp(&body_bytes));
    }
    if Scanner::blocks(&alerts) {
        state.log(InterceptorEvent {
            id,
            ts: now_ts(),
            session_id: Some(session_id),
            phase: "http.request".into(),
            transport: "http".into(),
            direction: Some("out".into()),
            method: Some(method),
            url: Some(uri),
            domain: domain.into(),
            content_type: req_content_type.clone(),
            req_bytes: Some(req_size),
            action: "blocked-secret".into(),
            alerts,
            req_headers,
            req_body_file,
            req_body: inline_body(&decoded_body),
            ..Default::default()
        });
        return Ok(RequestPrep::Respond(
            Response::builder()
                .status(403)
                .body(bytes::Bytes::from("secret detected").into())?,
        ));
    }
    if state.cfg.block_telemetry && path.contains("/event_logging") {
        state.log(InterceptorEvent {
            id,
            ts: now_ts(),
            session_id: Some(session_id),
            phase: "http.request".into(),
            transport: "http".into(),
            direction: Some("out".into()),
            method: Some(method),
            url: Some(uri),
            domain: domain.into(),
            content_type: req_content_type.clone(),
            req_bytes: Some(req_size),
            action: "blocked-telemetry".into(),
            req_headers,
            req_body_file,
            req_body: inline_body(&decoded_body),
            ..Default::default()
        });
        return Ok(RequestPrep::Respond(
            Response::builder()
                .status(200)
                .body(bytes::Bytes::from(r#"{"status":"ok"}"#).into())?,
        ));
    }

    let mut outbound_body = body_bytes.clone();
    let mut request_event = InterceptorEvent {
        id,
        ts: now_ts(),
        session_id: Some(session_id),
        phase: "http.request".into(),
        transport: "http".into(),
        direction: Some("out".into()),
        method: Some(method.clone()),
        url: Some(uri.clone()),
        domain: domain.into(),
        content_type: req_content_type,
        req_bytes: Some(req_size),
        action: "request".into(),
        alerts: alerts.clone(),
        req_headers: req_headers.clone(),
        req_body_file: req_body_file.clone(),
        req_body: inline_body(&decoded_body),
        ..Default::default()
    };
    let request_decision = resolve_decision(state, &request_event).await;
    if request_decision.action != "allow" {
        request_event.decision_action = Some(request_decision.action.clone());
        request_event.decision_reason = request_decision.reason.clone();
    }
    match request_decision.action.as_str() {
        "block" => {
            state.log(request_event);
            return Ok(RequestPrep::Respond(synth_decision_response(
                &request_decision,
                StatusCode::FORBIDDEN,
                "blocked by orchestrator",
            )?));
        }
        "modify" | "replace" => {
            if let Some(body) = decision_body_bytes(&request_decision)? {
                outbound_body = body;
                if save_req {
                    req_body_file = Some(state.save(id, "req", &outbound_body));
                }
            }
            apply_decision_headers(&mut parts.headers, &request_decision);
            if let Ok(value) = http::header::HeaderValue::from_str(&outbound_body.len().to_string())
            {
                parts.headers.insert(http::header::CONTENT_LENGTH, value);
            }
            request_event.content_type = content_type(&parts.headers);
            request_event.req_bytes = Some(outbound_body.len());
            request_event.req_headers = headers_to_pairs(&parts.headers);
            request_event.req_body_file = req_body_file.clone();
            request_event.req_body = inline_body(&State::decomp(&outbound_body));
        }
        _ => {}
    }
    state.log(request_event);

    Ok(RequestPrep::Proceed(PreparedRequest {
        id,
        started_at,
        client_upgrade,
        method,
        path,
        uri,
        parts,
        body_bytes: outbound_body,
        req_size,
        save_req,
        req_headers,
        req_body_file,
    }))
}
