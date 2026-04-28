use anyhow::Result;
use bytes::Bytes;
use hyper::{Response, StatusCode};
use std::sync::Arc;
use tracing::warn;

use crate::decision::{
    apply_decision_headers, decision_body_bytes, resolve_decision, synth_decision_response,
};
use crate::events::{inline_body, HeaderKv, InterceptorEvent};
use crate::state::{content_type, headers_to_pairs, should_save_response, State};
use crate::streaming::filter_sse_events;
use crate::FullBody;

use super::now_ts;

pub(crate) struct ResponseContext {
    pub id: u64,
    pub session_id: u64,
    pub domain: String,
    pub method: String,
    pub path: String,
    pub uri: String,
    pub status: u16,
    pub req_size: usize,
    pub req_headers: Vec<HeaderKv>,
    pub req_body_file: Option<String>,
    pub save_req: bool,
}

pub(crate) async fn handle_standard_response(
    state: Arc<State>,
    resp: Response<hyper::body::Incoming>,
    ctx: ResponseContext,
    started_at: std::time::Instant,
) -> Result<Response<FullBody>> {
    let (mut resp_parts, resp_body) = resp.into_parts();
    let mut resp_bytes = http_body_util::BodyExt::collect(resp_body)
        .await?
        .to_bytes()
        .to_vec();
    let mut decoded_resp = State::decomp(&resp_bytes);
    let mut resp_size = resp_bytes.len();
    let duration_ms = started_at.elapsed().as_millis() as u64;
    let save_resp = should_save_response(&ctx.path, &resp_parts.headers, resp_size, ctx.save_req);

    let mut resp_body_file = None;
    if save_resp {
        resp_body_file = Some(state.save(ctx.id, "resp", &resp_bytes));
    }

    let mut response_event = InterceptorEvent {
        id: ctx.id,
        ts: now_ts(),
        session_id: Some(ctx.session_id),
        phase: "http.response".into(),
        transport: "http".into(),
        direction: Some("in".into()),
        method: Some(ctx.method.clone()),
        url: Some(ctx.uri.clone()),
        domain: ctx.domain.clone(),
        status: Some(ctx.status),
        content_type: content_type(&resp_parts.headers),
        req_bytes: Some(ctx.req_size),
        resp_bytes: Some(resp_size),
        action: "response".into(),
        duration_ms: Some(duration_ms),
        req_headers: ctx.req_headers,
        resp_headers: headers_to_pairs(&resp_parts.headers),
        req_body_file: ctx.req_body_file,
        resp_body_file: resp_body_file.clone(),
        resp_body: inline_body(&decoded_resp),
        ..Default::default()
    };
    let response_decision = resolve_decision(&state, &response_event).await;
    if response_decision.action != "allow" {
        response_event.decision_action = Some(response_decision.action.clone());
        response_event.decision_reason = response_decision.reason.clone();
    }
    match response_decision.action.as_str() {
        "block" => {
            state.log(response_event);
            return synth_decision_response(
                &response_decision,
                StatusCode::FORBIDDEN,
                "blocked by orchestrator",
            );
        }
        "modify" | "replace" => {
            if let Some(body) = decision_body_bytes(&response_decision)? {
                resp_bytes = body;
                decoded_resp = State::decomp(&resp_bytes);
                resp_size = resp_bytes.len();
                if save_resp {
                    resp_body_file = Some(state.save(ctx.id, "resp", &resp_bytes));
                }
            }
            apply_decision_headers(&mut resp_parts.headers, &response_decision);
            resp_parts.headers.remove(http::header::TRANSFER_ENCODING);
            if let Ok(value) = http::header::HeaderValue::from_str(&resp_bytes.len().to_string()) {
                resp_parts
                    .headers
                    .insert(http::header::CONTENT_LENGTH, value);
            }
            if let Some(status) = response_decision
                .status
                .and_then(|code| StatusCode::from_u16(code).ok())
            {
                resp_parts.status = status;
            }
            response_event.status = Some(resp_parts.status.as_u16());
            response_event.content_type = content_type(&resp_parts.headers);
            response_event.resp_bytes = Some(resp_bytes.len());
            response_event.resp_headers = headers_to_pairs(&resp_parts.headers);
            response_event.resp_body_file = resp_body_file.clone();
            response_event.resp_body = inline_body(&decoded_resp);
        }
        _ => {}
    }

    let resp_content_type = content_type(&resp_parts.headers);
    if resp_content_type
        .as_deref()
        .map(|ct| ct.to_ascii_lowercase().contains("text/event-stream"))
        .unwrap_or(false)
    {
        match filter_sse_events(
            &state,
            ctx.session_id,
            &ctx.domain,
            &ctx.method,
            &ctx.uri,
            resp_parts.status.as_u16(),
            &State::decomp(&resp_bytes),
        )
        .await
        {
            Ok(filtered) => {
                resp_bytes = filtered;
                decoded_resp = State::decomp(&resp_bytes);
                resp_size = resp_bytes.len();
                if save_resp {
                    resp_body_file = Some(state.save(ctx.id, "resp", &resp_bytes));
                }
                resp_parts.headers.remove(http::header::TRANSFER_ENCODING);
                if let Ok(value) =
                    http::header::HeaderValue::from_str(&resp_bytes.len().to_string())
                {
                    resp_parts
                        .headers
                        .insert(http::header::CONTENT_LENGTH, value);
                }
            }
            Err(err) => warn!("sse capture {}: {err}", ctx.domain),
        }
    }

    response_event.content_type = content_type(&resp_parts.headers);
    response_event.resp_bytes = Some(resp_size);
    response_event.resp_body_file = resp_body_file.clone();
    response_event.resp_body = inline_body(&decoded_resp);
    response_event.resp_headers = headers_to_pairs(&resp_parts.headers);
    state.log(response_event);

    let mut response = Response::builder().status(resp_parts.status);
    for (key, value) in &resp_parts.headers {
        response = response.header(key, value);
    }
    Ok(response.body(Bytes::from(resp_bytes).into())?)
}
