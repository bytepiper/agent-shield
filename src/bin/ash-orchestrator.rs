use agent_shield::bus::DecisionEnvelope;
use agent_shield::events::InterceptorEvent;
use agent_shield::orchestrator::OrchestratorService;
use anyhow::Result;
use futures_util::StreamExt;
use tracing::{info, warn};

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();

    let nats_url = std::env::var("AGENT_SHIELD_ORCHESTRATOR_NATS_URL")
        .or_else(|_| std::env::var("AGENT_SHIELD_DECISION_NATS_URL"))
        .or_else(|_| std::env::var("AGENT_SHIELD_NATS_URL"))
        .unwrap_or_else(|_| "nats://127.0.0.1:4222".into());
    let subject = std::env::var("AGENT_SHIELD_ORCHESTRATOR_NATS_SUBJECT")
        .or_else(|_| std::env::var("AGENT_SHIELD_DECISION_NATS_SUBJECT"))
        .unwrap_or_else(|_| "ash.hooks.decision".into());

    let client = async_nats::connect(nats_url.clone()).await?;
    let mut subscriber = client.subscribe(subject.clone()).await?;
    let service = OrchestratorService::default();

    info!(
        "Orchestrator listening url={} subject={}",
        nats_url, subject
    );

    while let Some(message) = subscriber.next().await {
        let reply = message.reply.clone();
        let decision = match serde_json::from_slice::<InterceptorEvent>(&message.payload) {
            Ok(event) => match service.decide(event).await {
                Ok(decision) => decision,
                Err(err) => {
                    warn!("orchestrator decision failed: {err}");
                    DecisionEnvelope {
                        action: "block".into(),
                        reason: Some("orchestrator_error".into()),
                        status: Some(500),
                        ..Default::default()
                    }
                }
            },
            Err(err) => {
                warn!("orchestrator decode failed: {err}");
                DecisionEnvelope {
                    action: "block".into(),
                    reason: Some("invalid_event".into()),
                    status: Some(400),
                    ..Default::default()
                }
            }
        };

        if let Some(reply) = reply {
            match serde_json::to_vec(&decision) {
                Ok(payload) => {
                    if let Err(err) = client.publish(reply, payload.into()).await {
                        warn!("orchestrator reply publish failed: {err}");
                    }
                }
                Err(err) => warn!("orchestrator decision serialize failed: {err}"),
            }
        }
    }

    Ok(())
}
