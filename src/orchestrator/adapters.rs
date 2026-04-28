use std::collections::HashMap;
use std::sync::Arc;

use anyhow::Result;
use async_trait::async_trait;
use regex::Regex;

use crate::bus::DecisionHeader;

use super::classifier::TrafficClass;
use super::service::EvalContext;

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub enum AdapterAction {
    #[default]
    Neutral,
    Block,
    Modify,
    Replace,
    Route,
}

#[derive(Clone, Debug, Default)]
pub struct AdapterResult {
    pub adapter: String,
    pub action: AdapterAction,
    pub reason: Option<String>,
    pub text: Option<String>,
    pub base64: Option<String>,
    pub status: Option<u16>,
    pub headers: Vec<DecisionHeader>,
    pub route_target: Option<String>,
    pub meta: Option<serde_json::Value>,
}

#[async_trait]
pub trait Adapter: Send + Sync {
    fn name(&self) -> &'static str;

    async fn execute(&self, ctx: &EvalContext) -> Result<AdapterResult>;
}

#[derive(Clone, Default)]
pub struct AdapterRegistry {
    adapters: HashMap<String, Arc<dyn Adapter>>,
}

impl AdapterRegistry {
    pub fn with_defaults() -> Self {
        let mut registry = Self::default();
        registry.register(TelemetryBlockerAdapter);
        registry.register(SecretScannerAdapter::default());
        registry
    }

    pub fn register<A>(&mut self, adapter: A)
    where
        A: Adapter + 'static,
    {
        self.adapters
            .insert(adapter.name().to_string(), Arc::new(adapter));
    }

    pub fn get(&self, name: &str) -> Option<Arc<dyn Adapter>> {
        self.adapters.get(name).cloned()
    }
}

#[derive(Clone, Default)]
pub struct TelemetryBlockerAdapter;

#[async_trait]
impl Adapter for TelemetryBlockerAdapter {
    fn name(&self) -> &'static str {
        "telemetry_blocker"
    }

    async fn execute(&self, ctx: &EvalContext) -> Result<AdapterResult> {
        if ctx.traffic_class == TrafficClass::Telemetry {
            return Ok(AdapterResult {
                adapter: self.name().into(),
                action: AdapterAction::Block,
                reason: Some("telemetry_blocked".into()),
                status: Some(403),
                ..Default::default()
            });
        }
        Ok(AdapterResult {
            adapter: self.name().into(),
            ..Default::default()
        })
    }
}

#[derive(Clone)]
pub struct SecretScannerAdapter {
    patterns: Arc<Vec<SecretPattern>>,
}

impl Default for SecretScannerAdapter {
    fn default() -> Self {
        Self {
            patterns: Arc::new(vec![
                SecretPattern::new("anthropic_key", r"sk-ant-[A-Za-z0-9\-_]{20,}"),
                SecretPattern::new("openai_key", r"sk-[A-Za-z0-9]{20,}"),
                SecretPattern::new("aws_access_key", r"AKIA[0-9A-Z]{16}"),
                SecretPattern::new("github_pat", r"ghp_[A-Za-z0-9]{36}"),
                SecretPattern::new(
                    "credential_assignment",
                    r#"(?i)\b(password|passwd|pwd|secret|api[_-]?key|access[_-]?token|refresh[_-]?token)\b\s*[:=]\s*["']?[^\s"',}]{6,}"#,
                ),
            ]),
        }
    }
}

#[async_trait]
impl Adapter for SecretScannerAdapter {
    fn name(&self) -> &'static str {
        "secret_scanner"
    }

    async fn execute(&self, ctx: &EvalContext) -> Result<AdapterResult> {
        if !matches!(
            ctx.event.phase.as_str(),
            "http.request" | "ws.message.out" | "http.response" | "ws.message.in" | "sse.event.in"
        ) {
            return Ok(AdapterResult {
                adapter: self.name().into(),
                ..Default::default()
            });
        }

        if matches!(
            ctx.traffic_class,
            TrafficClass::Telemetry | TrafficClass::ControlPlane
        ) {
            return Ok(AdapterResult {
                adapter: self.name().into(),
                ..Default::default()
            });
        }

        let Some(text) = ctx.primary_text() else {
            return Ok(AdapterResult {
                adapter: self.name().into(),
                ..Default::default()
            });
        };

        for pattern in self.patterns.iter() {
            if let Some(found) = pattern.find(text) {
                return Ok(AdapterResult {
                    adapter: self.name().into(),
                    action: AdapterAction::Block,
                    reason: Some(format!("secret_detected:{}", pattern.name)),
                    status: Some(403),
                    meta: Some(serde_json::json!({
                        "secret_type": pattern.name,
                        "matched_preview": redact_match(found.as_str()),
                    })),
                    ..Default::default()
                });
            }
        }

        Ok(AdapterResult {
            adapter: self.name().into(),
            ..Default::default()
        })
    }
}

#[derive(Clone)]
struct SecretPattern {
    name: &'static str,
    regex: Regex,
}

impl SecretPattern {
    fn new(name: &'static str, pattern: &'static str) -> Self {
        Self {
            name,
            regex: Regex::new(pattern).expect("valid secret regex"),
        }
    }

    fn find<'a>(&self, text: &'a str) -> Option<regex::Match<'a>> {
        self.regex.find(text)
    }
}

fn redact_match(value: &str) -> String {
    if value.len() <= 8 {
        return "***".into();
    }
    format!("{}***{}", &value[..4], &value[value.len() - 4..])
}

#[cfg(test)]
mod tests {
    use super::{Adapter, SecretScannerAdapter, TelemetryBlockerAdapter};
    use crate::events::{EventBody, InterceptorEvent};
    use crate::orchestrator::classifier::TrafficClass;
    use crate::orchestrator::service::EvalContext;

    #[tokio::test]
    async fn telemetry_blocker_blocks_telemetry() {
        let adapter = TelemetryBlockerAdapter;
        let result = adapter
            .execute(&EvalContext {
                event: InterceptorEvent {
                    phase: "http.request".into(),
                    domain: "play.googleapis.com".into(),
                    ..Default::default()
                },
                traffic_class: TrafficClass::Telemetry,
            })
            .await
            .expect("adapter result");
        assert_eq!(result.action, super::AdapterAction::Block);
    }

    #[tokio::test]
    async fn secret_scanner_blocks_openai_key() {
        let adapter = SecretScannerAdapter::default();
        let fixture_key = format!("sk-{}", "1234567890abcdefghijklmnop");
        let result = adapter
            .execute(&EvalContext {
                event: InterceptorEvent {
                    phase: "http.request".into(),
                    req_body: Some(EventBody {
                        bytes: 32,
                        truncated: false,
                        text: Some(fixture_key),
                        base64: None,
                    }),
                    ..Default::default()
                },
                traffic_class: TrafficClass::Unknown,
            })
            .await
            .expect("adapter result");
        assert_eq!(result.action, super::AdapterAction::Block);
        assert_eq!(result.reason.as_deref(), Some("secret_detected:openai_key"));
    }
}
