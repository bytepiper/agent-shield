use std::collections::BTreeSet;
use std::sync::Arc;
use std::time::Duration;

use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use tracing::{info, warn};

use crate::bus::{DecisionEnvelope, DecisionHeader};
use crate::events::{EventBody, HeaderKv};

use super::classifier::TrafficClass;

const HANDLER_CONTEXT_SCHEMA_VERSION: &str = "v1";

#[derive(Clone, Serialize)]
pub struct HandlerContext {
    pub schema_version: &'static str,
    pub event_id: u64,
    pub session_id: Option<u64>,
    pub event_seq: u64,
    pub session_seq: u64,
    pub phase: String,
    pub transport: String,
    pub direction: Option<String>,
    pub method: Option<String>,
    pub url: Option<String>,
    pub domain: String,
    pub status: Option<u16>,
    pub action: String,
    pub content_type: Option<String>,
    pub traffic_class: TrafficClass,
    pub req_headers: Vec<HeaderKv>,
    pub resp_headers: Vec<HeaderKv>,
    pub req_body: Option<EventBody>,
    pub resp_body: Option<EventBody>,
    pub primary_text: Option<String>,
    pub preview: Option<String>,
}

impl HandlerContext {
    pub fn schema_version() -> &'static str {
        HANDLER_CONTEXT_SCHEMA_VERSION
    }
}

#[derive(Clone, Debug, Default, Deserialize)]
pub struct HandlerResultEnvelope {
    pub action: Option<String>,
    pub reason: Option<String>,
    #[serde(default)]
    pub headers: Vec<DecisionHeader>,
    pub text: Option<String>,
    pub base64: Option<String>,
    pub status: Option<u16>,
    pub route_target: Option<String>,
    pub meta: Option<serde_json::Value>,
}

impl From<HandlerResultEnvelope> for DecisionEnvelope {
    fn from(value: HandlerResultEnvelope) -> Self {
        Self {
            action: value.action.unwrap_or_else(|| "allow".into()),
            reason: value.reason,
            status: value.status,
            headers: value.headers,
            text: value.text,
            base64: value.base64,
            route_target: value.route_target,
            meta: value.meta,
        }
    }
}

#[derive(Clone, Debug, Default)]
struct PhaseFilter {
    phases: BTreeSet<String>,
}

impl PhaseFilter {
    fn from_env(var: &str) -> Self {
        let phases = std::env::var(var)
            .unwrap_or_default()
            .split(',')
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(|value| value.to_string())
            .collect();
        Self { phases }
    }

    fn matches(&self, phase: &str) -> bool {
        self.phases.is_empty() || self.phases.contains(phase)
    }
}

#[derive(Clone)]
struct ListenerTarget {
    url: Arc<str>,
    phase_filter: PhaseFilter,
}

#[derive(Clone)]
struct HandlerTarget {
    url: Arc<str>,
    phase_filter: PhaseFilter,
}

#[derive(Clone)]
pub struct RestCallbacks {
    listeners: Vec<ListenerTarget>,
    handler: Option<HandlerTarget>,
    listener_client: reqwest::Client,
    handler_client: reqwest::Client,
}

impl RestCallbacks {
    pub fn from_env() -> Result<Option<Self>> {
        let listener_urls = std::env::var("AGENT_SHIELD_ORCHESTRATOR_LISTENER_URLS")
            .ok()
            .filter(|value| !value.is_empty())
            .map(|value| parse_csv_values(&value))
            .unwrap_or_else(|| {
                std::env::var("AGENT_SHIELD_ORCHESTRATOR_LISTENER_URL")
                    .ok()
                    .filter(|value| !value.is_empty())
                    .map(|value| vec![value])
                    .unwrap_or_default()
            });
        let handler_url = std::env::var("AGENT_SHIELD_ORCHESTRATOR_HANDLER_URL")
            .ok()
            .filter(|value| !value.is_empty());
        if listener_urls.is_empty() && handler_url.is_none() {
            return Ok(None);
        }

        let listener_timeout_ms = std::env::var("AGENT_SHIELD_ORCHESTRATOR_LISTENER_TIMEOUT_MS")
            .ok()
            .and_then(|value| value.parse().ok())
            .unwrap_or(500);
        let handler_timeout_ms = std::env::var("AGENT_SHIELD_ORCHESTRATOR_HANDLER_TIMEOUT_MS")
            .ok()
            .and_then(|value| value.parse().ok())
            .unwrap_or(1500);
        let listener_phase_filter =
            PhaseFilter::from_env("AGENT_SHIELD_ORCHESTRATOR_LISTENER_PHASES");
        let handler_phase_filter =
            PhaseFilter::from_env("AGENT_SHIELD_ORCHESTRATOR_HANDLER_PHASES");

        let listener_client = reqwest::Client::builder()
            .timeout(Duration::from_millis(listener_timeout_ms))
            .build()?;
        let handler_client = reqwest::Client::builder()
            .timeout(Duration::from_millis(handler_timeout_ms))
            .build()?;
        let listeners = listener_urls
            .into_iter()
            .map(|url| ListenerTarget {
                url: Arc::<str>::from(url),
                phase_filter: listener_phase_filter.clone(),
            })
            .collect::<Vec<_>>();
        let handler = handler_url.map(|url| HandlerTarget {
            url: Arc::<str>::from(url),
            phase_filter: handler_phase_filter,
        });

        info!(
            "REST callbacks configured listeners={} handler={} listener_timeout_ms={} handler_timeout_ms={}",
            listeners.len(),
            handler
                .as_ref()
                .map(|target| target.url.as_ref())
                .unwrap_or(""),
            listener_timeout_ms,
            handler_timeout_ms
        );

        Ok(Some(Self {
            listeners,
            handler,
            listener_client,
            handler_client,
        }))
    }

    pub fn notify_listeners(&self, context: HandlerContext) {
        for listener in &self.listeners {
            if !listener.phase_filter.matches(&context.phase) {
                continue;
            }

            let url = listener.url.clone();
            let client = self.listener_client.clone();
            let payload = context.clone();
            tokio::spawn(async move {
                match client.post(url.to_string()).json(&payload).send().await {
                    Ok(response) if response.status().is_success() => {}
                    Ok(response) => warn!(
                        "listener callback returned non-success url={} status={}",
                        url,
                        response.status()
                    ),
                    Err(err) => warn!("listener callback failed url={url}: {err}"),
                }
            });
        }
    }

    pub async fn call_handler(&self, context: &HandlerContext) -> Result<Option<DecisionEnvelope>> {
        let Some(handler) = &self.handler else {
            return Ok(None);
        };
        if !handler.phase_filter.matches(&context.phase) {
            return Ok(None);
        }

        let response = self
            .handler_client
            .post(handler.url.to_string())
            .json(context)
            .send()
            .await?;
        if !response.status().is_success() {
            return Err(anyhow!(
                "handler callback status={} url={}",
                response.status(),
                handler.url
            ));
        }
        if response.status() == reqwest::StatusCode::NO_CONTENT {
            return Ok(None);
        }
        let result = response.json::<HandlerResultEnvelope>().await?;
        Ok(Some(result.into()))
    }
}

fn parse_csv_values(value: &str) -> Vec<String> {
    value
        .split(',')
        .map(str::trim)
        .filter(|item| !item.is_empty())
        .map(|item| item.to_string())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::{HandlerContext, PhaseFilter};
    use crate::events::{EventBody, HeaderKv};
    use crate::orchestrator::classifier::TrafficClass;

    #[test]
    fn phase_filter_matches_all_when_empty() {
        let filter = PhaseFilter::default();
        assert!(filter.matches("http.request"));
        assert!(filter.matches("ws.message.in"));
    }

    #[test]
    fn handler_context_contains_schema_version() {
        let ctx = HandlerContext {
            schema_version: HandlerContext::schema_version(),
            event_id: 1,
            session_id: Some(2),
            event_seq: 3,
            session_seq: 4,
            phase: "http.request".into(),
            transport: "http".into(),
            direction: Some("out".into()),
            method: Some("POST".into()),
            url: Some("/v1/messages".into()),
            domain: "api.anthropic.com".into(),
            status: None,
            action: "request".into(),
            content_type: Some("application/json".into()),
            traffic_class: TrafficClass::ModelHttp,
            req_headers: vec![HeaderKv {
                name: "content-type".into(),
                value: "application/json".into(),
            }],
            resp_headers: Vec::new(),
            req_body: Some(EventBody {
                bytes: 5,
                truncated: false,
                text: Some("hello".into()),
                base64: None,
            }),
            resp_body: None,
            primary_text: Some("hello".into()),
            preview: Some("hello".into()),
        };

        let json = serde_json::to_value(ctx).expect("serialize");
        assert_eq!(json["schema_version"], HandlerContext::schema_version());
        assert_eq!(json["phase"], "http.request");
        assert_eq!(json["primary_text"], "hello");
    }
}
