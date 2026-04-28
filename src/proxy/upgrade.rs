use anyhow::Result;
use bytes::Bytes;
use hyper::Response;
use hyper_util::rt::TokioIo;
use std::sync::Arc;
use tracing::warn;

use crate::events::{inline_body, HeaderKv, InterceptorEvent};
use crate::state::{content_type, headers_to_pairs, State};
use crate::streaming::{relay_with_capture, WsFrameCapture};
use crate::FullBody;

use super::now_ts;

pub(crate) struct UpgradeContext {
    pub id: u64,
    pub session_id: u64,
    pub domain: String,
    pub method: String,
    pub uri: String,
    pub status: u16,
    pub req_size: usize,
    pub req_headers: Vec<HeaderKv>,
    pub req_body_file: Option<String>,
    pub body_bytes: Vec<u8>,
    pub save_req: bool,
}

pub(crate) async fn handle_switching_protocols(
    state: Arc<State>,
    mut resp: Response<hyper::body::Incoming>,
    client_upgrade: hyper::upgrade::OnUpgrade,
    ctx: UpgradeContext,
    started_at: std::time::Instant,
) -> Result<Response<FullBody>> {
    let server_upgrade = hyper::upgrade::on(&mut resp);
    let (resp_parts, _) = resp.into_parts();
    let resp_headers = headers_to_pairs(&resp_parts.headers);
    let duration_ms = started_at.elapsed().as_millis() as u64;
    let mut req_body_file = ctx.req_body_file.clone();
    let mut resp_body_file = None;

    let captures = if ctx.save_req {
        let (req_name, req_path) = state.body_file_path(ctx.id, "req", "txt");
        let (resp_name, resp_path) = state.body_file_path(ctx.id, "resp", "txt");
        let mut req_capture = WsFrameCapture::create(
            state.clone(),
            ctx.session_id,
            &req_path,
            &ctx.domain,
            &ctx.uri,
            "ws-out",
            true,
        )?;
        if !ctx.body_bytes.is_empty() {
            let _ = req_capture.ingest(&ctx.body_bytes).await?;
        }
        req_body_file = Some(req_name.clone());
        resp_body_file = Some(resp_name.clone());
        Some((
            req_capture,
            WsFrameCapture::create(
                state.clone(),
                ctx.session_id,
                &resp_path,
                &ctx.domain,
                &ctx.uri,
                "ws-in",
                false,
            )?,
        ))
    } else {
        None
    };

    state.log(InterceptorEvent {
        id: ctx.id,
        ts: now_ts(),
        session_id: Some(ctx.session_id),
        phase: "http.response".into(),
        transport: "http".into(),
        direction: Some("in".into()),
        method: Some(ctx.method),
        url: Some(ctx.uri.clone()),
        domain: ctx.domain.clone(),
        status: Some(ctx.status),
        content_type: content_type(&resp_parts.headers),
        req_bytes: Some(ctx.req_size),
        resp_bytes: Some(0),
        action: "response".into(),
        duration_ms: Some(duration_ms),
        req_headers: ctx.req_headers,
        resp_headers,
        req_body_file,
        resp_body_file,
        req_body: inline_body(&ctx.body_bytes),
        ..Default::default()
    });

    let tunnel_domain = ctx.domain.clone();
    tokio::spawn(async move {
        match tokio::try_join!(client_upgrade, server_upgrade) {
            Ok((client_upgraded, server_upgraded)) => {
                let client = TokioIo::new(client_upgraded);
                let server = TokioIo::new(server_upgraded);
                let (client_r, client_w) = tokio::io::split(client);
                let (server_r, server_w) = tokio::io::split(server);
                let (req_capture, resp_capture) = match captures {
                    Some((req_capture, resp_capture)) => (Some(req_capture), Some(resp_capture)),
                    None => (None, None),
                };
                if let Err(err) = tokio::try_join!(
                    relay_with_capture(client_r, server_w, req_capture),
                    relay_with_capture(server_r, client_w, resp_capture)
                ) {
                    warn!("upgrade tunnel {tunnel_domain}: {err}");
                }
            }
            Err(err) => warn!("upgrade setup {tunnel_domain}: {err}"),
        }
    });

    let mut response = Response::builder().status(resp_parts.status);
    for (key, value) in &resp_parts.headers {
        response = response.header(key, value);
    }
    Ok(response.body(Bytes::new().into())?)
}
