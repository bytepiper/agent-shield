use crate::events::{alert_entries, dashboard_entries, stats_json, BodyFile};
use crate::store::EventStore;
use axum::{extract::Path as AxumPath, response::Html, routing::get, Json, Router};
use hyper::StatusCode;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::net::TcpListener;
use tracing::info;

pub async fn serve_dashboard(store: Arc<EventStore>, body_dir: PathBuf, port: u16) {
    let (s1, s2, s3, s4, s5, s6) = (
        store.clone(),
        store.clone(),
        store.clone(),
        store.clone(),
        body_dir.clone(),
        body_dir.clone(),
    );
    let app = Router::new()
        .route(
            "/",
            get(|| async {
                (
                    [
                        ("cache-control", "no-store, no-cache, must-revalidate"),
                        ("pragma", "no-cache"),
                        ("expires", "0"),
                    ],
                    Html(include_str!("dashboard.html")),
                )
            }),
        )
        .route(
            "/api/events",
            get(move || async move { Json(s1.snapshot()) }),
        )
        .route(
            "/api/traffic",
            get(move || async move { Json(dashboard_entries(&s2.snapshot())) }),
        )
        .route(
            "/api/alerts",
            get(move || async move { Json(alert_entries(&s3.snapshot())) }),
        )
        .route(
            "/api/stats",
            get(move || async move { Json(stats_json(&s4.snapshot())) }),
        )
        .route(
            "/api/bodies",
            get(move || {
                let body_dir = s6.clone();
                async move {
                    let mut items = Vec::new();
                    if let Ok(rd) = std::fs::read_dir(&body_dir) {
                        for ent in rd.flatten() {
                            if let Ok(meta) = ent.metadata() {
                                if meta.is_file() {
                                    items.push(BodyFile {
                                        name: ent.file_name().to_string_lossy().into_owned(),
                                        size: meta.len(),
                                    });
                                }
                            }
                        }
                    }
                    items.sort_by(|a, b| a.name.cmp(&b.name));
                    Json(items)
                }
            }),
        )
        .route(
            "/api/body/{name}",
            get(move |AxumPath(n): AxumPath<String>| {
                let body_dir = s5.clone();
                async move {
                    let p = body_dir.join(n.replace("..", "").replace('/', ""));
                    match std::fs::read(&p) {
                        Ok(c) => {
                            if let Ok(v) = serde_json::from_slice::<serde_json::Value>(&c) {
                                (
                                    StatusCode::OK,
                                    [("content-type", "application/json")],
                                    serde_json::to_string_pretty(&v).unwrap_or_default(),
                                )
                            } else {
                                (
                                    StatusCode::OK,
                                    [("content-type", "text/plain")],
                                    String::from_utf8_lossy(&c).into_owned(),
                                )
                            }
                        }
                        Err(_) => (
                            StatusCode::NOT_FOUND,
                            [("content-type", "text/plain")],
                            "not found".into(),
                        ),
                    }
                }
            }),
        );

    let listener = TcpListener::bind(format!("0.0.0.0:{port}")).await.unwrap();
    info!("Dashboard :{port}");
    axum::serve(listener, app).await.unwrap();
}
