use std::path::PathBuf;
use std::sync::Arc;

use crate::bus;
use crate::events::InterceptorEvent;
use crate::store::EventStore;

#[derive(Clone, Default)]
pub struct DashboardApp {
    store: Arc<EventStore>,
}

impl DashboardApp {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn push(&self, event: InterceptorEvent) {
        self.store.push(event);
    }

    pub fn spawn_local_subscriber(
        &self,
        name: impl Into<String>,
        queue_capacity: usize,
    ) -> bus::AsyncSubscriber {
        let app = self.clone();
        bus::AsyncSubscriber::spawn(name, queue_capacity, move |event| {
            let app = app.clone();
            Box::pin(async move {
                app.push(event);
                Ok(())
            })
        })
    }

    pub fn spawn_server(&self, body_dir: PathBuf, port: u16) {
        let store = self.store.clone();
        tokio::spawn(async move { crate::dashboard::serve_dashboard(store, body_dir, port).await });
    }

    pub fn snapshot(&self) -> Vec<InterceptorEvent> {
        self.store.snapshot()
    }
}
