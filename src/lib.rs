use bytes::Bytes;

pub mod app;
pub mod bus;
pub mod ca;
pub mod config;
pub mod dashboard;
pub mod dashboard_app;
pub mod dashboard_runtime;
pub mod decision;
pub mod embedded_dashboard;
pub mod events;
pub mod orchestrator;
pub mod proxy;
pub mod runtime;
pub mod scanner;
pub mod server;
pub mod state;
pub mod store;
pub mod streaming;

pub type FullBody = http_body_util::Full<Bytes>;
