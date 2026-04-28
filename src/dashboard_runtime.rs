use crate::dashboard_app::DashboardApp;
use crate::events::InterceptorEvent;
use anyhow::Result;
use async_nats::jetstream;
use async_nats::jetstream::consumer::{pull, AckPolicy};
use async_nats::jetstream::stream::Config as StreamConfig;
use futures_util::StreamExt;
use std::path::PathBuf;
use tokio::time::{sleep, Duration};
use tracing::{info, warn};

pub struct ExternalDashboardConfig {
    pub nats_url: String,
    pub stream_name: String,
    pub subject: String,
    pub consumer_name: String,
    pub port: u16,
    pub body_dir: PathBuf,
}

impl ExternalDashboardConfig {
    pub fn from_env() -> Self {
        Self {
            nats_url: std::env::var("AGENT_SHIELD_DASHBOARD_NATS_URL")
                .or_else(|_| std::env::var("AGENT_SHIELD_NATS_URL"))
                .unwrap_or_else(|_| "nats://127.0.0.1:4222".into()),
            stream_name: std::env::var("AGENT_SHIELD_DASHBOARD_NATS_STREAM")
                .or_else(|_| std::env::var("AGENT_SHIELD_NATS_STREAM"))
                .unwrap_or_else(|_| "ash_events".into()),
            subject: std::env::var("AGENT_SHIELD_DASHBOARD_NATS_SUBJECT")
                .or_else(|_| std::env::var("AGENT_SHIELD_NATS_SUBJECT"))
                .unwrap_or_else(|_| "ash.events.raw".into()),
            consumer_name: std::env::var("AGENT_SHIELD_DASHBOARD_CONSUMER")
                .unwrap_or_else(|_| "ash_dashboard".into()),
            port: std::env::var("AGENT_SHIELD_DASHBOARD_PORT")
                .ok()
                .and_then(|value| value.parse().ok())
                .unwrap_or(9999),
            body_dir: std::env::var("AGENT_SHIELD_BODY_DIR")
                .map(Into::into)
                .unwrap_or_else(|_| "/tmp/agent-shield-bodies".into()),
        }
    }
}

pub async fn run_external_dashboard(config: ExternalDashboardConfig) -> Result<()> {
    let app = DashboardApp::new();
    app.spawn_server(config.body_dir.clone(), config.port);

    loop {
        let client = match async_nats::connect(config.nats_url.clone()).await {
            Ok(client) => client,
            Err(err) => {
                warn!("dashboard nats connect failed: {err}");
                sleep(Duration::from_secs(1)).await;
                continue;
            }
        };
        let jetstream = jetstream::new(client);
        let stream = match jetstream
            .get_or_create_stream(StreamConfig {
                name: config.stream_name.clone(),
                subjects: vec![config.subject.clone()],
                ..Default::default()
            })
            .await
        {
            Ok(stream) => stream,
            Err(err) => {
                warn!("dashboard stream setup failed: {err}");
                sleep(Duration::from_secs(1)).await;
                continue;
            }
        };
        let consumer = match stream
            .get_or_create_consumer(
                &config.consumer_name,
                pull::Config {
                    durable_name: Some(config.consumer_name.clone()),
                    filter_subject: config.subject.clone(),
                    ack_policy: AckPolicy::Explicit,
                    ..Default::default()
                },
            )
            .await
        {
            Ok(consumer) => consumer,
            Err(err) => {
                warn!("dashboard consumer setup failed: {err}");
                sleep(Duration::from_secs(1)).await;
                continue;
            }
        };

        info!(
            "External dashboard subscriber enabled url={} stream={} subject={} consumer={} port={}",
            config.nats_url, config.stream_name, config.subject, config.consumer_name, config.port
        );

        let mut messages = match consumer.messages().await {
            Ok(messages) => messages,
            Err(err) => {
                warn!("dashboard consumer stream failed: {err}");
                sleep(Duration::from_secs(1)).await;
                continue;
            }
        };
        while let Some(message) = messages.next().await {
            match message {
                Ok(message) => {
                    match serde_json::from_slice::<InterceptorEvent>(&message.payload) {
                        Ok(event) => app.push(event),
                        Err(err) => warn!("dashboard decode failed: {err}"),
                    }
                    if let Err(err) = message.ack().await {
                        warn!("dashboard ack failed: {err}");
                    }
                }
                Err(err) => warn!("dashboard subscriber error: {err}"),
            }
        }
        warn!("dashboard subscriber stream ended, reconnecting");
        sleep(Duration::from_secs(1)).await;
    }
}
