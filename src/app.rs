use anyhow::Result;
use std::sync::Arc;

use crate::ca::Ca;
use crate::config::Config;
use crate::embedded_dashboard;
use crate::runtime;
use crate::scanner::Scanner;
use crate::server;
use crate::state::State;

pub async fn run() -> Result<()> {
    let cfg = Config::default();
    let ca = Ca::load_or_gen()?;
    let _ = std::fs::create_dir_all(&cfg.body_dir);
    runtime::log_startup(&cfg);

    let embedded_dashboard = embedded_dashboard::maybe_runtime(cfg.dashboard_port, 2048);
    let subscriber_bus = runtime::build_subscriber_bus(&cfg, embedded_dashboard.as_ref()).await?;
    let decision_transport = runtime::build_decision_transport(&cfg).await?;

    let body_dir = cfg.body_dir.clone();
    let dashboard_port = cfg.dashboard_port;
    let proxy_port = cfg.proxy_port;
    let state = Arc::new(State::new(
        cfg,
        ca,
        Scanner::new(),
        subscriber_bus,
        decision_transport,
    ));

    if let (Some(runtime), Some(port)) = (embedded_dashboard.as_ref(), dashboard_port) {
        runtime.spawn(body_dir, port);
    }

    server::serve_proxy(state, proxy_port).await
}
