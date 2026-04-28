use std::path::PathBuf;

use crate::bus;
use crate::dashboard_app::DashboardApp;

pub struct EmbeddedDashboardRuntime {
    app: DashboardApp,
    subscriber: bus::AsyncSubscriber,
}

impl EmbeddedDashboardRuntime {
    pub fn new(queue_capacity: usize) -> Self {
        let app = DashboardApp::new();
        let subscriber = app.spawn_local_subscriber("local-event-store", queue_capacity);
        Self { app, subscriber }
    }

    pub fn subscriber(&self) -> bus::AsyncSubscriber {
        self.subscriber.clone()
    }

    pub fn spawn(&self, body_dir: PathBuf, port: u16) {
        self.app.spawn_server(body_dir, port);
    }

    #[allow(dead_code)]
    pub fn snapshot(&self) -> Vec<crate::events::InterceptorEvent> {
        self.app.snapshot()
    }
}

pub fn maybe_runtime(port: Option<u16>, queue_capacity: usize) -> Option<EmbeddedDashboardRuntime> {
    port.map(|_| EmbeddedDashboardRuntime::new(queue_capacity))
}
