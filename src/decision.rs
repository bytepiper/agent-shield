use anyhow::Result;
use base64::Engine as _;
use bytes::Bytes;
use http::HeaderMap;
use hyper::{Response, StatusCode};
use tracing::warn;

use crate::bus::DecisionEnvelope;
use crate::events::InterceptorEvent;
use crate::state::State;
use crate::FullBody;

pub(crate) async fn resolve_decision(
    state: &std::sync::Arc<State>,
    event: &InterceptorEvent,
) -> DecisionEnvelope {
    match state.decision_transport.decide(event).await {
        Ok(decision) if decision.action.is_empty() => DecisionEnvelope {
            action: "allow".into(),
            ..Default::default()
        },
        Ok(decision) => decision,
        Err(err) => {
            warn!(
                "decision transport failed phase={} domain={}: {err}",
                event.phase, event.domain
            );
            DecisionEnvelope {
                action: "allow".into(),
                ..Default::default()
            }
        }
    }
}

pub(crate) fn decision_body_bytes(decision: &DecisionEnvelope) -> Result<Option<Vec<u8>>> {
    if let Some(text) = decision.text.as_ref() {
        return Ok(Some(text.as_bytes().to_vec()));
    }
    if let Some(base64) = decision.base64.as_ref() {
        let decoded = base64::engine::general_purpose::STANDARD.decode(base64)?;
        return Ok(Some(decoded));
    }
    Ok(None)
}

pub(crate) fn apply_decision_headers(headers: &mut HeaderMap, decision: &DecisionEnvelope) {
    for header in &decision.headers {
        let Ok(name) = http::header::HeaderName::try_from(header.name.as_str()) else {
            continue;
        };
        let Ok(value) = http::header::HeaderValue::try_from(header.value.as_str()) else {
            continue;
        };
        headers.insert(name, value);
    }
}

pub(crate) fn synth_decision_response(
    decision: &DecisionEnvelope,
    fallback_status: StatusCode,
    fallback_body: &str,
) -> Result<Response<FullBody>> {
    let status = decision
        .status
        .and_then(|code| StatusCode::from_u16(code).ok())
        .unwrap_or(fallback_status);
    let body = decision_body_bytes(decision)?
        .map(Bytes::from)
        .unwrap_or_else(|| Bytes::from(fallback_body.to_string()));

    let mut response = Response::builder().status(status);
    for header in &decision.headers {
        response = response.header(&header.name, &header.value);
    }
    Ok(response.body(body.into())?)
}
