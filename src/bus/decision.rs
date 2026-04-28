use crate::events::InterceptorEvent;
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::pin::Pin;
use tokio::time::{timeout, Duration};
use tracing::info;

#[derive(Clone)]
pub struct NatsDecisionConfig {
    pub url: String,
    pub subject: String,
    pub timeout_ms: u64,
}

pub trait DecisionTransport: Send + Sync {
    fn decide(
        &self,
        event: &InterceptorEvent,
    ) -> Pin<Box<dyn Future<Output = Result<DecisionEnvelope>> + Send + '_>>;
}

use std::future::Future;

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct DecisionEnvelope {
    pub action: String,
    pub reason: Option<String>,
    pub status: Option<u16>,
    #[serde(default)]
    pub headers: Vec<DecisionHeader>,
    pub text: Option<String>,
    pub base64: Option<String>,
    pub route_target: Option<String>,
    pub meta: Option<serde_json::Value>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct DecisionHeader {
    pub name: String,
    pub value: String,
}

#[derive(Clone, Default)]
pub struct NoopDecisionTransport;

impl DecisionTransport for NoopDecisionTransport {
    fn decide(
        &self,
        _event: &InterceptorEvent,
    ) -> Pin<Box<dyn Future<Output = Result<DecisionEnvelope>> + Send + '_>> {
        Box::pin(async {
            Ok(DecisionEnvelope {
                action: "allow".into(),
                reason: None,
                ..Default::default()
            })
        })
    }
}

#[derive(Clone)]
pub struct NatsDecisionTransport {
    client: async_nats::Client,
    subject: Arc<str>,
    timeout: Duration,
}

use std::sync::Arc;

impl NatsDecisionTransport {
    pub async fn connect(config: NatsDecisionConfig) -> Result<Self> {
        let client = async_nats::connect(config.url.clone()).await?;
        info!(
            "NATS decision transport enabled url={} subject={} timeout_ms={}",
            config.url, config.subject, config.timeout_ms
        );
        Ok(Self {
            client,
            subject: Arc::<str>::from(config.subject),
            timeout: Duration::from_millis(config.timeout_ms),
        })
    }
}

impl DecisionTransport for NatsDecisionTransport {
    fn decide(
        &self,
        event: &InterceptorEvent,
    ) -> Pin<Box<dyn Future<Output = Result<DecisionEnvelope>> + Send + '_>> {
        let payload = serde_json::to_vec(event);
        let client = self.client.clone();
        let subject = self.subject.to_string();
        let timeout_duration = self.timeout;
        Box::pin(async move {
            let payload = payload?;
            let message =
                timeout(timeout_duration, client.request(subject, payload.into())).await??;
            if message.payload.is_empty() {
                return Ok(DecisionEnvelope {
                    action: "allow".into(),
                    ..Default::default()
                });
            }
            Ok(serde_json::from_slice::<DecisionEnvelope>(
                &message.payload,
            )?)
        })
    }
}
