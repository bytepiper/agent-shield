use std::path::PathBuf;

pub(crate) struct Config {
    pub(crate) proxy_port: u16,
    pub(crate) pass: Vec<String>,
    pub(crate) block: Vec<String>,
    pub(crate) block_telemetry: bool,
    pub(crate) scan_secrets: bool,
    pub(crate) body_dir: PathBuf,
    pub(crate) dashboard_port: Option<u16>,
    pub(crate) nats: Option<NatsConfig>,
    pub(crate) decision_nats: Option<DecisionNatsConfig>,
}

#[derive(Clone)]
pub(crate) struct NatsConfig {
    pub(crate) url: String,
    pub(crate) stream: String,
    pub(crate) subject: String,
    pub(crate) queue_capacity: usize,
}

#[derive(Clone)]
pub(crate) struct DecisionNatsConfig {
    pub(crate) url: String,
    pub(crate) subject: String,
    pub(crate) timeout_ms: u64,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            proxy_port: std::env::var("AGENT_SHIELD_PROXY_PORT")
                .ok()
                .and_then(|value| value.parse().ok())
                .unwrap_or(8888),
            pass: vec!["registry.npmjs.org", "pypi.org", "github.com"]
                .into_iter()
                .map(Into::into)
                .collect(),
            block: vec!["http-intake.logs.us5.datadoghq.com"]
                .into_iter()
                .map(Into::into)
                .collect(),
            block_telemetry: true,
            scan_secrets: true,
            body_dir: "/tmp/agent-shield-bodies".into(),
            dashboard_port: if std::env::var("AGENT_SHIELD_DISABLE_EMBEDDED_DASHBOARD")
                .ok()
                .as_deref()
                == Some("1")
            {
                None
            } else {
                Some(
                    std::env::var("AGENT_SHIELD_DASHBOARD_PORT")
                        .ok()
                        .and_then(|value| value.parse().ok())
                        .unwrap_or(9999),
                )
            },
            nats: std::env::var("AGENT_SHIELD_NATS_URL")
                .ok()
                .map(|url| NatsConfig {
                    url,
                    stream: std::env::var("AGENT_SHIELD_NATS_STREAM")
                        .unwrap_or_else(|_| "ash_events".into()),
                    subject: std::env::var("AGENT_SHIELD_NATS_SUBJECT")
                        .unwrap_or_else(|_| "ash.events.raw".into()),
                    queue_capacity: std::env::var("AGENT_SHIELD_NATS_QUEUE_CAPACITY")
                        .ok()
                        .and_then(|value| value.parse().ok())
                        .unwrap_or(4096),
                }),
            decision_nats: std::env::var("AGENT_SHIELD_DECISION_NATS_URL")
                .ok()
                .or_else(|| std::env::var("AGENT_SHIELD_NATS_URL").ok())
                .map(|url| DecisionNatsConfig {
                    url,
                    subject: std::env::var("AGENT_SHIELD_DECISION_NATS_SUBJECT")
                        .unwrap_or_else(|_| "ash.hooks.decision".into()),
                    timeout_ms: std::env::var("AGENT_SHIELD_DECISION_TIMEOUT_MS")
                        .ok()
                        .and_then(|value| value.parse().ok())
                        .unwrap_or(1500),
                }),
        }
    }
}
