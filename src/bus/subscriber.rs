use crate::events::InterceptorEvent;
use anyhow::Result;
use async_nats::jetstream;
use async_nats::jetstream::stream::Config as StreamConfig;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use tokio::sync::mpsc;
use tracing::{info, warn};

type EventFuture = Pin<Box<dyn Future<Output = Result<()>> + Send + 'static>>;
type EventHandler = Arc<dyn Fn(InterceptorEvent) -> EventFuture + Send + Sync>;

pub trait SubscriberTransport: Send + Sync {
    fn publish(&self, event: InterceptorEvent);
}

#[derive(Clone)]
pub struct NatsSubscriberConfig {
    pub url: String,
    pub stream: String,
    pub subject: String,
    pub queue_capacity: usize,
}

#[derive(Clone)]
pub struct AsyncSubscriber {
    name: Arc<str>,
    tx: mpsc::Sender<InterceptorEvent>,
}

impl AsyncSubscriber {
    pub fn spawn(
        name: impl Into<String>,
        queue_capacity: usize,
        handler: impl Fn(InterceptorEvent) -> EventFuture + Send + Sync + 'static,
    ) -> Self {
        let name = Arc::<str>::from(name.into());
        let handler: EventHandler = Arc::new(handler);
        let (tx, mut rx) = mpsc::channel(queue_capacity);
        let worker_name = name.clone();

        tokio::spawn(async move {
            while let Some(event) = rx.recv().await {
                if let Err(err) = handler(event).await {
                    warn!("subscriber {} failed: {err}", worker_name);
                }
            }
        });

        Self { name, tx }
    }
}

#[derive(Clone, Default)]
pub struct InProcessSubscriberTransport {
    subscribers: Arc<Vec<AsyncSubscriber>>,
}

impl InProcessSubscriberTransport {
    pub fn new(subscribers: Vec<AsyncSubscriber>) -> Self {
        Self {
            subscribers: Arc::new(subscribers),
        }
    }
}

impl SubscriberTransport for InProcessSubscriberTransport {
    fn publish(&self, event: InterceptorEvent) {
        for subscriber in self.subscribers.iter() {
            if let Err(err) = subscriber.tx.try_send(event.clone()) {
                let reason = match err {
                    mpsc::error::TrySendError::Full(_) => "queue full",
                    mpsc::error::TrySendError::Closed(_) => "subscriber closed",
                };
                warn!(
                    "subscriber {} dropped event {}: {}",
                    subscriber.name, event.id, reason
                );
            }
        }
    }
}

#[derive(Clone, Default)]
pub struct CompositeSubscriberTransport {
    transports: Arc<Vec<Arc<dyn SubscriberTransport>>>,
}

impl CompositeSubscriberTransport {
    pub fn new(transports: Vec<Arc<dyn SubscriberTransport>>) -> Self {
        Self {
            transports: Arc::new(transports),
        }
    }
}

impl SubscriberTransport for CompositeSubscriberTransport {
    fn publish(&self, event: InterceptorEvent) {
        for transport in self.transports.iter() {
            transport.publish(event.clone());
        }
    }
}

#[derive(Clone)]
pub struct NatsSubscriberTransport {
    subject: Arc<str>,
    tx: mpsc::Sender<InterceptorEvent>,
}

impl NatsSubscriberTransport {
    pub async fn connect(config: NatsSubscriberConfig) -> Result<Self> {
        let client = async_nats::connect(config.url.clone()).await?;
        let jetstream = jetstream::new(client);
        jetstream
            .get_or_create_stream(StreamConfig {
                name: config.stream.clone(),
                subjects: vec![config.subject.clone()],
                ..Default::default()
            })
            .await?;

        let subject = Arc::<str>::from(config.subject.clone());
        let stream = config.stream.clone();
        let (tx, mut rx) = mpsc::channel::<InterceptorEvent>(config.queue_capacity);
        let publish_subject = config.subject.clone();

        tokio::spawn(async move {
            while let Some(event) = rx.recv().await {
                let payload = match serde_json::to_vec(&event) {
                    Ok(payload) => payload,
                    Err(err) => {
                        warn!("nats encode failed for event {}: {err}", event.id);
                        continue;
                    }
                };

                match jetstream
                    .publish(publish_subject.clone(), payload.into())
                    .await
                {
                    Ok(ack) => {
                        if let Err(err) = ack.await {
                            warn!("nats ack failed for event {}: {err}", event.id);
                        }
                    }
                    Err(err) => warn!("nats publish failed for event {}: {err}", event.id),
                }
            }
        });

        info!(
            "NATS subscriber transport enabled url={} stream={} subject={}",
            config.url, stream, config.subject
        );

        Ok(Self { subject, tx })
    }
}

impl SubscriberTransport for NatsSubscriberTransport {
    fn publish(&self, event: InterceptorEvent) {
        if let Err(err) = self.tx.try_send(event.clone()) {
            let reason = match err {
                mpsc::error::TrySendError::Full(_) => "queue full",
                mpsc::error::TrySendError::Closed(_) => "publisher closed",
            };
            warn!(
                "nats transport dropped event {} on {}: {}",
                event.id, self.subject, reason
            );
        }
    }
}
