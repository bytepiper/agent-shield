use agent_shield::dashboard_runtime::{run_external_dashboard, ExternalDashboardConfig};
use anyhow::Result;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();
    run_external_dashboard(ExternalDashboardConfig::from_env()).await
}
