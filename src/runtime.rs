use anyhow::Result;
use std::sync::Arc;
use tracing::info;

use crate::bus::{self, DecisionTransport, SubscriberTransport};
use crate::config::Config;
use crate::embedded_dashboard::EmbeddedDashboardRuntime;

pub(crate) async fn build_subscriber_bus(
    cfg: &Config,
    embedded_dashboard: Option<&EmbeddedDashboardRuntime>,
) -> Result<Arc<dyn SubscriberTransport>> {
    let mut transports: Vec<Arc<dyn SubscriberTransport>> = Vec::new();
    if let Some(runtime) = embedded_dashboard {
        transports.push(Arc::new(bus::InProcessSubscriberTransport::new(vec![
            runtime.subscriber(),
        ])));
    }
    if let Some(nats) = cfg.nats.clone() {
        let nats_transport = bus::NatsSubscriberTransport::connect(bus::NatsSubscriberConfig {
            url: nats.url,
            stream: nats.stream,
            subject: nats.subject,
            queue_capacity: nats.queue_capacity,
        })
        .await?;
        transports.push(Arc::new(nats_transport));
    }
    Ok(Arc::new(bus::CompositeSubscriberTransport::new(transports)))
}

pub(crate) async fn build_decision_transport(cfg: &Config) -> Result<Arc<dyn DecisionTransport>> {
    Ok(if let Some(decision) = cfg.decision_nats.clone() {
        Arc::new(
            bus::NatsDecisionTransport::connect(bus::NatsDecisionConfig {
                url: decision.url,
                subject: decision.subject,
                timeout_ms: decision.timeout_ms,
            })
            .await?,
        )
    } else {
        Arc::new(bus::NoopDecisionTransport)
    })
}

pub(crate) fn log_startup(cfg: &Config) {
    info!("=== Agent Shield (Rust) ===");
    info!(
        "MITM default=on | Pass: {:?} | Block: {:?}",
        cfg.pass, cfg.block
    );
    if let Some(nats) = cfg.nats.as_ref() {
        info!(
            "NATS async bus configured url={} stream={} subject={}",
            nats.url, nats.stream, nats.subject
        );
    }
    if let Some(decision) = cfg.decision_nats.as_ref() {
        info!(
            "NATS decision path configured url={} subject={} timeout_ms={}",
            decision.url, decision.subject, decision.timeout_ms
        );
    }
}
