use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
use base64::Engine;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

const MAX_INLINE_BODY_BYTES: usize = 64 * 1024;

#[derive(Clone, Serialize, Deserialize)]
pub struct Alert {
    pub pattern: String,
    pub action: String,
    pub matched: String,
}

#[derive(Clone, Serialize, Deserialize, Default)]
pub struct HeaderKv {
    pub name: String,
    pub value: String,
}

#[derive(Clone, Serialize, Deserialize, Default)]
pub struct EventBody {
    pub bytes: usize,
    pub truncated: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub base64: Option<String>,
}

#[derive(Serialize)]
pub struct BodyFile {
    pub name: String,
    pub size: u64,
}

#[derive(Clone, Serialize, Deserialize, Default)]
pub struct InterceptorEvent {
    pub id: u64,
    pub ts: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub session_id: Option<u64>,
    pub event_seq: u64,
    pub session_seq: u64,
    pub phase: String,
    pub transport: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub direction: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub method: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub preview: Option<String>,
    pub domain: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<u16>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub req_bytes: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resp_bytes: Option<usize>,
    pub action: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub decision_action: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub decision_reason: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duration_ms: Option<u64>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub alerts: Vec<Alert>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub req_headers: Vec<HeaderKv>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub resp_headers: Vec<HeaderKv>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub req_body_file: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resp_body_file: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub req_body: Option<EventBody>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resp_body: Option<EventBody>,
}

#[derive(Clone, Serialize, Deserialize, Default)]
pub struct DashboardEntry {
    pub id: u64,
    pub ts: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub method: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub preview: Option<String>,
    pub domain: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<u16>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub req_bytes: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resp_bytes: Option<usize>,
    pub action: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub decision_action: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub decision_reason: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duration_ms: Option<u64>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub alerts: Vec<Alert>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub req_headers: Vec<HeaderKv>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub resp_headers: Vec<HeaderKv>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub req_body_file: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resp_body_file: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub req_body: Option<EventBody>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resp_body: Option<EventBody>,
}

impl From<&InterceptorEvent> for DashboardEntry {
    fn from(value: &InterceptorEvent) -> Self {
        Self {
            id: value.id,
            ts: value.ts.clone(),
            method: value.method.clone(),
            url: value.url.clone(),
            preview: value.preview.clone(),
            domain: value.domain.clone(),
            status: value.status,
            req_bytes: value.req_bytes,
            resp_bytes: value.resp_bytes,
            action: value.action.clone(),
            decision_action: value.decision_action.clone(),
            decision_reason: value.decision_reason.clone(),
            duration_ms: value.duration_ms,
            alerts: value.alerts.clone(),
            req_headers: value.req_headers.clone(),
            resp_headers: value.resp_headers.clone(),
            req_body_file: value.req_body_file.clone(),
            resp_body_file: value.resp_body_file.clone(),
            req_body: value.req_body.clone(),
            resp_body: value.resp_body.clone(),
        }
    }
}

pub fn inline_body(body: &[u8]) -> Option<EventBody> {
    if body.is_empty() {
        return None;
    }
    let inline_len = body.len().min(MAX_INLINE_BODY_BYTES);
    let inline = &body[..inline_len];
    let truncated = body.len() > inline_len;
    if let Ok(text) = std::str::from_utf8(inline) {
        return Some(EventBody {
            bytes: body.len(),
            truncated,
            text: Some(text.to_string()),
            base64: None,
        });
    }
    Some(EventBody {
        bytes: body.len(),
        truncated,
        text: None,
        base64: Some(BASE64_STANDARD.encode(inline)),
    })
}

pub fn dashboard_entries(events: &[InterceptorEvent]) -> Vec<DashboardEntry> {
    events.iter().map(DashboardEntry::from).collect()
}

pub fn alert_entries(events: &[InterceptorEvent]) -> Vec<DashboardEntry> {
    events
        .iter()
        .filter(|event| !event.alerts.is_empty())
        .map(DashboardEntry::from)
        .collect()
}

pub fn stats_json(events: &[InterceptorEvent]) -> serde_json::Value {
    let mut domains: HashMap<String, usize> = HashMap::new();
    let (mut requests, mut blocked, mut alerts) = (0usize, 0usize, 0usize);

    for event in events {
        if !event.domain.is_empty() {
            *domains.entry(event.domain.clone()).or_default() += 1;
        }
        if event.action == "request" || event.action == "response" {
            requests += 1;
        }
        if event.action.starts_with("blocked") || event.decision_action.as_deref() == Some("block")
        {
            blocked += 1;
        }
        if !event.alerts.is_empty() {
            alerts += 1;
        }
    }

    serde_json::json!({
        "domains": domains,
        "total_requests": requests,
        "total_blocked": blocked,
        "total_alerts": alerts,
    })
}
