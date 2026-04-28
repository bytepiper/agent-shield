use anyhow::Result;
use flate2::read::GzDecoder;
use http::HeaderMap;
use std::collections::HashMap;
use std::io::Read;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use crate::bus::{DecisionTransport, SubscriberTransport};
use crate::ca::Ca;
use crate::config::Config;
use crate::events::{HeaderKv, InterceptorEvent};
use crate::scanner::Scanner;

const MAX_CAPTURE_BYTES: usize = 2 * 1024 * 1024;

pub(crate) fn domain_of(host: &str) -> &str {
    host.split(':').next().unwrap_or(host)
}

pub(crate) fn matches(host: &str, list: &[String]) -> bool {
    let domain = domain_of(host);
    list.iter()
        .any(|pattern| domain == pattern || domain.ends_with(&format!(".{pattern}")))
}

fn body_is_textual(headers: &HeaderMap) -> bool {
    headers
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .map(|ct| {
            let ct = ct.to_ascii_lowercase();
            ct.starts_with("text/")
                || ct.contains("json")
                || ct.contains("xml")
                || ct.contains("javascript")
                || ct.contains("x-www-form-urlencoded")
                || ct.contains("graphql")
                || ct.contains("event-stream")
        })
        .unwrap_or(false)
}

pub(crate) fn content_type(headers: &HeaderMap) -> Option<String> {
    headers
        .get("content-type")
        .and_then(|value| value.to_str().ok())
        .map(|value| value.to_string())
}

pub(crate) fn should_save_request(
    method: &str,
    path: &str,
    headers: &HeaderMap,
    body_len: usize,
) -> bool {
    if body_len > MAX_CAPTURE_BYTES {
        return false;
    }
    if [
        "/messages",
        "/responses",
        "/event_logging",
        "/codex/",
        "/models",
        "/usage",
        "/otlp/",
    ]
    .iter()
    .any(|s| path.contains(s))
    {
        return true;
    }
    body_len > 0 && matches!(method, "POST" | "PUT" | "PATCH") && body_is_textual(headers)
}

pub(crate) fn should_save_response(
    path: &str,
    headers: &HeaderMap,
    body_len: usize,
    req_saved: bool,
) -> bool {
    if body_len > MAX_CAPTURE_BYTES {
        return false;
    }
    req_saved
        || [
            "/messages",
            "/responses",
            "/event_logging",
            "/codex/",
            "/models",
            "/usage",
            "/otlp/",
        ]
        .iter()
        .any(|s| path.contains(s))
        || (body_len > 0 && body_is_textual(headers))
}

pub(crate) fn headers_to_pairs(headers: &HeaderMap) -> Vec<HeaderKv> {
    headers
        .iter()
        .map(|(name, value)| HeaderKv {
            name: name.as_str().to_string(),
            value: String::from_utf8_lossy(value.as_bytes()).into_owned(),
        })
        .collect()
}

pub(crate) struct State {
    pub(crate) cfg: Config,
    pub(crate) ca: Ca,
    pub(crate) scanner: Scanner,
    ctr: AtomicU64,
    event_seq: AtomicU64,
    session_seq: Mutex<HashMap<u64, u64>>,
    pub(crate) subscriber_bus: Arc<dyn SubscriberTransport>,
    pub(crate) decision_transport: Arc<dyn DecisionTransport>,
}

impl State {
    pub(crate) fn new(
        cfg: Config,
        ca: Ca,
        scanner: Scanner,
        subscriber_bus: Arc<dyn SubscriberTransport>,
        decision_transport: Arc<dyn DecisionTransport>,
    ) -> Self {
        Self {
            cfg,
            ca,
            scanner,
            ctr: AtomicU64::new(1),
            event_seq: AtomicU64::new(1),
            session_seq: Mutex::new(HashMap::new()),
            subscriber_bus,
            decision_transport,
        }
    }

    pub(crate) fn id(&self) -> u64 {
        self.ctr.fetch_add(1, Ordering::Relaxed)
    }

    pub(crate) fn log(&self, mut event: InterceptorEvent) {
        event.event_seq = self.event_seq.fetch_add(1, Ordering::Relaxed);
        if let Some(session_id) = event.session_id {
            let mut session_seq = self.session_seq.lock().unwrap();
            let next = session_seq.get(&session_id).copied().unwrap_or(0) + 1;
            event.session_seq = next;
            if event.phase == "session.close" {
                session_seq.remove(&session_id);
            } else {
                session_seq.insert(session_id, next);
            }
        }
        if let Ok(json) = serde_json::to_string(&event) {
            println!("{json}");
        }
        self.subscriber_bus.publish(event);
    }

    pub(crate) fn decomp(body: &[u8]) -> Vec<u8> {
        if body.len() > 4 && body[..4] == [0x28, 0xB5, 0x2F, 0xFD] {
            if let Ok(decoded) = zstd::decode_all(body) {
                return decoded;
            }
        }
        if body.len() > 2 && body[..2] == [0x1F, 0x8B] {
            let mut decoder = GzDecoder::new(body);
            let mut out = Vec::new();
            if decoder.read_to_end(&mut out).is_ok() {
                return out;
            }
        }
        body.to_vec()
    }

    pub(crate) fn save(&self, id: u64, dir: &str, body: &[u8]) -> String {
        let _ = std::fs::create_dir_all(&self.cfg.body_dir);
        let decoded = Self::decomp(body);
        let name = format!("{id}_{dir}.json");
        let _ = std::fs::write(self.cfg.body_dir.join(&name), &decoded);
        name
    }

    pub(crate) fn body_file_path(&self, id: u64, dir: &str, ext: &str) -> (String, PathBuf) {
        let name = format!("{id}_{dir}.{ext}");
        (name.clone(), self.cfg.body_dir.join(&name))
    }

    pub(crate) fn save_raw(&self, id: u64, dir: &str, ext: &str, body: &[u8]) -> Result<String> {
        let (name, path) = self.body_file_path(id, dir, ext);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&path, body)?;
        Ok(name)
    }
}
