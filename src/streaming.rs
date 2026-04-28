use anyhow::Result;
use std::io::Write as _;
use std::path::PathBuf;
use std::time::SystemTime;
use tracing::warn;

use crate::decision::{decision_body_bytes, resolve_decision};
use crate::events::{inline_body, HeaderKv, InterceptorEvent};
use crate::state::State;

pub(crate) struct WsFrameCapture {
    state: std::sync::Arc<State>,
    session_id: u64,
    domain: String,
    url: String,
    action: String,
    forward_masked: bool,
    file: std::fs::File,
    pending: Vec<u8>,
    message_no: u64,
    current_opcode: Option<u8>,
    current_compressed: bool,
    current_payload: Vec<u8>,
    current_raw_frames: Vec<Vec<u8>>,
}

impl WsFrameCapture {
    pub(crate) fn create(
        state: std::sync::Arc<State>,
        session_id: u64,
        path: &PathBuf,
        domain: &str,
        url: &str,
        action: &str,
        forward_masked: bool,
    ) -> Result<Self> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let file = std::fs::OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(path)?;
        Ok(Self {
            state,
            session_id,
            domain: domain.into(),
            url: url.into(),
            action: action.into(),
            forward_masked,
            file,
            pending: Vec::new(),
            message_no: 0,
            current_opcode: None,
            current_compressed: false,
            current_payload: Vec::new(),
            current_raw_frames: Vec::new(),
        })
    }

    pub(crate) async fn ingest(&mut self, chunk: &[u8]) -> Result<Vec<Vec<u8>>> {
        let mut outbound = Vec::new();
        self.pending.extend_from_slice(chunk);
        while let Some((used, fin, opcode, rsv1, _masked, payload, raw_frame)) =
            try_parse_ws_frame(&self.pending)?
        {
            match opcode {
                0x1 | 0x2 => {
                    self.current_opcode = Some(opcode);
                    self.current_compressed = rsv1;
                    self.current_payload.clear();
                    self.current_payload.extend_from_slice(&payload);
                    self.current_raw_frames.clear();
                    self.current_raw_frames.push(raw_frame);
                    if fin {
                        outbound.push(self.flush_message().await?);
                    }
                }
                0x0 => {
                    if self.current_opcode.is_none() {
                        self.write_control_frame(opcode, fin, &payload)?;
                        outbound.push(raw_frame);
                    } else {
                        self.current_payload.extend_from_slice(&payload);
                        self.current_raw_frames.push(raw_frame);
                        if fin {
                            outbound.push(self.flush_message().await?);
                        }
                    }
                }
                _ => {
                    self.write_control_frame(opcode, fin, &payload)?;
                    outbound.push(raw_frame);
                }
            }
            self.pending.drain(..used);
        }
        Ok(outbound)
    }

    async fn flush_message(&mut self) -> Result<Vec<u8>> {
        let opcode = self.current_opcode.take().unwrap_or(0x2);
        let compressed = self.current_compressed;
        self.current_compressed = false;
        self.message_no += 1;

        let payload = if compressed {
            ws_inflate(&self.current_payload)?
        } else {
            self.current_payload.clone()
        };

        writeln!(
            self.file,
            "--- message {} kind={} compressed={} bytes={} ---",
            self.message_no,
            ws_opcode_name(opcode),
            compressed,
            payload.len()
        )?;
        write_ws_payload(&mut self.file, &payload)?;
        self.file.flush()?;
        let outbound = self.log_message(opcode, &payload, compressed).await?;
        self.current_payload.clear();
        self.current_raw_frames.clear();
        Ok(outbound)
    }

    fn write_control_frame(&mut self, opcode: u8, fin: bool, payload: &[u8]) -> Result<()> {
        self.message_no += 1;
        writeln!(
            self.file,
            "--- frame {} kind={} fin={} bytes={} ---",
            self.message_no,
            ws_opcode_name(opcode),
            fin,
            payload.len()
        )?;
        write_ws_payload(&mut self.file, payload)?;
        self.file.flush()?;
        Ok(())
    }

    async fn log_message(&self, opcode: u8, payload: &[u8], compressed: bool) -> Result<Vec<u8>> {
        let id = self.state.id();
        let is_out = self.action == "ws-out";
        let dir = if is_out { "req" } else { "resp" };
        let mut effective_payload = payload.to_vec();
        let mut event = InterceptorEvent {
            id,
            ts: format!(
                "{:?}",
                SystemTime::now()
                    .duration_since(SystemTime::UNIX_EPOCH)
                    .unwrap_or_default()
            ),
            session_id: Some(self.session_id),
            phase: if is_out {
                "ws.message.out".into()
            } else {
                "ws.message.in".into()
            },
            transport: "ws".into(),
            direction: Some(if is_out { "out".into() } else { "in".into() }),
            method: Some(format!(
                "WS {}",
                ws_opcode_name(opcode).to_ascii_uppercase()
            )),
            url: Some(self.url.clone()),
            preview: ws_preview(payload),
            domain: self.domain.clone(),
            content_type: std::str::from_utf8(payload)
                .ok()
                .map(|_| "application/json".into()),
            req_bytes: is_out.then_some(payload.len()),
            resp_bytes: (!is_out).then_some(payload.len()),
            action: self.action.clone(),
            ..Default::default()
        };
        let decision = resolve_decision(&self.state, &event).await;
        if decision.action != "allow" {
            event.decision_action = Some(decision.action.clone());
            event.decision_reason = decision.reason.clone();
        }

        let outbound = match decision.action.as_str() {
            "block" => {
                let body_file = self.state.save_raw(id, dir, "txt", &effective_payload)?;
                event.req_body_file = is_out.then_some(body_file.clone());
                event.resp_body_file = (!is_out).then_some(body_file);
                event.req_body = is_out.then_some(inline_body(&effective_payload)).flatten();
                event.resp_body = (!is_out)
                    .then_some(inline_body(&effective_payload))
                    .flatten();
                self.state.log(event);
                return Err(anyhow::anyhow!("websocket message blocked by orchestrator"));
            }
            "modify" | "replace" => {
                if let Some(body) = decision_body_bytes(&decision)? {
                    effective_payload = body;
                }
                encode_ws_frame(opcode, self.forward_masked, &effective_payload)
            }
            _ => self.current_raw_frames.concat(),
        };

        let body_file = self.state.save_raw(id, dir, "txt", &effective_payload)?;
        event.preview = ws_preview(&effective_payload);
        event.req_bytes = is_out.then_some(effective_payload.len());
        event.resp_bytes = (!is_out).then_some(effective_payload.len());
        event.req_body_file = is_out.then_some(body_file.clone());
        event.resp_body_file = (!is_out).then_some(body_file);
        event.req_body = is_out.then_some(inline_body(&effective_payload)).flatten();
        event.resp_body = (!is_out)
            .then_some(inline_body(&effective_payload))
            .flatten();
        if compressed && matches!(decision.action.as_str(), "modify" | "replace") {
            event.content_type = event
                .content_type
                .or(Some("application/octet-stream".into()));
        }
        self.state.log(event);
        Ok(outbound)
    }
}

fn ws_opcode_name(opcode: u8) -> &'static str {
    match opcode {
        0x0 => "continuation",
        0x1 => "text",
        0x2 => "binary",
        0x8 => "close",
        0x9 => "ping",
        0xA => "pong",
        _ => "other",
    }
}

fn try_parse_ws_frame(
    buf: &[u8],
) -> Result<Option<(usize, bool, u8, bool, bool, Vec<u8>, Vec<u8>)>> {
    if buf.len() < 2 {
        return Ok(None);
    }

    let b0 = buf[0];
    let b1 = buf[1];
    let fin = b0 & 0x80 != 0;
    let opcode = b0 & 0x0F;
    let rsv1 = b0 & 0x40 != 0;
    let masked = b1 & 0x80 != 0;

    let mut idx = 2usize;
    let len_code = (b1 & 0x7F) as u64;
    let payload_len = if len_code < 126 {
        len_code
    } else if len_code == 126 {
        if buf.len() < idx + 2 {
            return Ok(None);
        }
        let len = u16::from_be_bytes([buf[idx], buf[idx + 1]]) as u64;
        idx += 2;
        len
    } else {
        if buf.len() < idx + 8 {
            return Ok(None);
        }
        let len = u64::from_be_bytes([
            buf[idx],
            buf[idx + 1],
            buf[idx + 2],
            buf[idx + 3],
            buf[idx + 4],
            buf[idx + 5],
            buf[idx + 6],
            buf[idx + 7],
        ]);
        idx += 8;
        len
    };

    let mask = if masked {
        if buf.len() < idx + 4 {
            return Ok(None);
        }
        let m = [buf[idx], buf[idx + 1], buf[idx + 2], buf[idx + 3]];
        idx += 4;
        Some(m)
    } else {
        None
    };

    let payload_len_usize =
        usize::try_from(payload_len).map_err(|_| anyhow::anyhow!("websocket frame too large"))?;
    if buf.len() < idx + payload_len_usize {
        return Ok(None);
    }

    let mut payload = buf[idx..idx + payload_len_usize].to_vec();
    if let Some(mask) = mask {
        for (i, byte) in payload.iter_mut().enumerate() {
            *byte ^= mask[i % 4];
        }
    }

    let used = idx + payload_len_usize;
    Ok(Some((
        used,
        fin,
        opcode,
        rsv1,
        masked,
        payload,
        buf[..used].to_vec(),
    )))
}

fn write_ws_payload(file: &mut std::fs::File, payload: &[u8]) -> Result<()> {
    if let Ok(text) = std::str::from_utf8(payload) {
        file.write_all(text.as_bytes())?;
        if !text.ends_with('\n') {
            writeln!(file)?;
        }
    } else {
        for b in payload {
            write!(file, "{b:02x}")?;
        }
        writeln!(file)?;
    }
    Ok(())
}

fn encode_ws_frame(opcode: u8, masked: bool, payload: &[u8]) -> Vec<u8> {
    let mut frame = Vec::with_capacity(payload.len() + 14);
    frame.push(0x80 | (opcode & 0x0F));

    let len = payload.len();
    let mask_bit = if masked { 0x80 } else { 0x00 };
    if len < 126 {
        frame.push(mask_bit | (len as u8));
    } else if len <= u16::MAX as usize {
        frame.push(mask_bit | 126);
        frame.extend_from_slice(&(len as u16).to_be_bytes());
    } else {
        frame.push(mask_bit | 127);
        frame.extend_from_slice(&(len as u64).to_be_bytes());
    }

    if masked {
        let seed = len as u32 ^ 0x5A17_1EAF;
        let mask = seed.to_be_bytes();
        frame.extend_from_slice(&mask);
        for (i, byte) in payload.iter().enumerate() {
            frame.push(byte ^ mask[i % 4]);
        }
    } else {
        frame.extend_from_slice(payload);
    }

    frame
}

fn ws_preview(payload: &[u8]) -> Option<String> {
    if let Ok(text) = std::str::from_utf8(payload) {
        for line in text.lines().filter(|line| !line.trim().is_empty()) {
            if let Ok(value) = serde_json::from_str::<serde_json::Value>(line) {
                if let Some(preview) = ws_preview_json(&value) {
                    return Some(preview);
                }
            }
        }
        let preview = compact_preview(text);
        return (!preview.is_empty()).then_some(preview);
    }

    let hex = payload
        .iter()
        .take(32)
        .map(|b| format!("{b:02x}"))
        .collect::<Vec<_>>()
        .join(" ");
    (!hex.is_empty()).then_some(format!("binary {hex}"))
}

fn ws_preview_json(value: &serde_json::Value) -> Option<String> {
    const KEYS: &[&str] = &[
        "output_text",
        "text",
        "delta",
        "content",
        "message",
        "summary",
    ];
    for key in KEYS {
        if let Some(found) = find_json_string(value, key) {
            let preview = compact_preview(found);
            if !preview.is_empty() {
                return Some(preview);
            }
        }
    }
    value
        .get("type")
        .and_then(|kind| kind.as_str())
        .map(compact_preview)
        .filter(|preview| !preview.is_empty())
}

fn find_json_string<'a>(value: &'a serde_json::Value, key: &str) -> Option<&'a str> {
    match value {
        serde_json::Value::Object(map) => {
            if let Some(found) = map.get(key).and_then(first_json_string) {
                return Some(found);
            }
            map.values().find_map(|item| find_json_string(item, key))
        }
        serde_json::Value::Array(items) => {
            items.iter().find_map(|item| find_json_string(item, key))
        }
        _ => None,
    }
}

fn first_json_string(value: &serde_json::Value) -> Option<&str> {
    match value {
        serde_json::Value::String(text) => Some(text),
        serde_json::Value::Array(items) => items.iter().find_map(first_json_string),
        serde_json::Value::Object(map) => map.values().find_map(first_json_string),
        _ => None,
    }
}

fn compact_preview(text: &str) -> String {
    let compact = text.split_whitespace().collect::<Vec<_>>().join(" ");
    let mut preview = compact.chars().take(160).collect::<String>();
    if compact.chars().count() > 160 {
        preview.push_str("...");
    }
    preview
}

fn ws_inflate(payload: &[u8]) -> Result<Vec<u8>> {
    let mut framed = Vec::with_capacity(payload.len() + 4);
    framed.extend_from_slice(payload);
    framed.extend_from_slice(&[0x00, 0x00, 0xFF, 0xFF]);

    let mut out = Vec::new();
    let mut decoder = flate2::read::DeflateDecoder::new(&framed[..]);
    std::io::Read::read_to_end(&mut decoder, &mut out)
        .map_err(|e| anyhow::anyhow!("permessage-deflate decode failed: {e}"))?;
    Ok(out)
}

async fn process_sse_event(
    state: &std::sync::Arc<State>,
    session_id: u64,
    domain: &str,
    method: &str,
    url: &str,
    status: u16,
    event_name: Option<String>,
    data_lines: &[String],
) -> Result<Option<String>> {
    if data_lines.is_empty() {
        return Ok(None);
    }

    let mut payload = data_lines.join("\n");
    let id = state.id();
    let mut event = InterceptorEvent {
        id,
        ts: format!(
            "{:?}",
            SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap_or_default()
        ),
        session_id: Some(session_id),
        phase: "sse.event.in".into(),
        transport: "sse".into(),
        direction: Some("in".into()),
        method: Some(method.to_string()),
        url: Some(url.to_string()),
        preview: Some(compact_preview(&payload)),
        domain: domain.to_string(),
        status: Some(status),
        content_type: Some("text/event-stream".into()),
        resp_bytes: Some(payload.len()),
        action: "sse-in".into(),
        resp_body: inline_body(payload.as_bytes()),
        resp_headers: event_name
            .as_ref()
            .map(|name| {
                vec![HeaderKv {
                    name: "event".into(),
                    value: name.clone(),
                }]
            })
            .unwrap_or_default(),
        ..Default::default()
    };

    let decision = resolve_decision(state, &event).await;
    if decision.action != "allow" {
        event.decision_action = Some(decision.action.clone());
        event.decision_reason = decision.reason.clone();
    }

    match decision.action.as_str() {
        "block" => {
            state.log(event);
            return Ok(None);
        }
        "modify" | "replace" => {
            if let Some(body) = decision_body_bytes(&decision)? {
                payload = String::from_utf8_lossy(&body).into_owned();
            }
        }
        _ => {}
    }

    let body_file = state.save_raw(id, "resp", "txt", payload.as_bytes())?;
    event.preview = Some(compact_preview(&payload));
    event.resp_bytes = Some(payload.len());
    event.resp_body_file = Some(body_file);
    event.resp_body = inline_body(payload.as_bytes());
    state.log(event);

    let mut rebuilt = String::new();
    if let Some(name) = event_name {
        rebuilt.push_str("event: ");
        rebuilt.push_str(&name);
        rebuilt.push('\n');
    }
    for line in payload.lines() {
        rebuilt.push_str("data: ");
        rebuilt.push_str(line);
        rebuilt.push('\n');
    }
    rebuilt.push('\n');
    Ok(Some(rebuilt))
}

pub(crate) async fn filter_sse_events(
    state: &std::sync::Arc<State>,
    session_id: u64,
    domain: &str,
    method: &str,
    url: &str,
    status: u16,
    body: &[u8],
) -> Result<Vec<u8>> {
    let text = match std::str::from_utf8(body) {
        Ok(text) => text,
        Err(_) => return Ok(body.to_vec()),
    };

    let mut rebuilt = String::new();
    let mut event_name: Option<String> = None;
    let mut data_lines: Vec<String> = Vec::new();

    for line in text.lines() {
        if line.is_empty() {
            if let Some(chunk) = process_sse_event(
                state,
                session_id,
                domain,
                method,
                url,
                status,
                event_name.take(),
                &data_lines,
            )
            .await?
            {
                rebuilt.push_str(&chunk);
            }
            data_lines.clear();
            continue;
        }
        if let Some(rest) = line.strip_prefix("event:") {
            event_name = Some(rest.trim().to_string());
            continue;
        }
        if let Some(rest) = line.strip_prefix("data:") {
            data_lines.push(rest.trim_start().to_string());
        }
    }

    if let Some(chunk) = process_sse_event(
        state,
        session_id,
        domain,
        method,
        url,
        status,
        event_name.take(),
        &data_lines,
    )
    .await?
    {
        rebuilt.push_str(&chunk);
    }

    Ok(rebuilt.into_bytes())
}

pub(crate) async fn relay_with_capture<R, W>(
    mut reader: R,
    mut writer: W,
    mut capture: Option<WsFrameCapture>,
) -> Result<()>
where
    R: tokio::io::AsyncRead + Unpin,
    W: tokio::io::AsyncWrite + Unpin,
{
    let mut buf = [0u8; 16 * 1024];
    loop {
        let n = tokio::io::AsyncReadExt::read(&mut reader, &mut buf).await?;
        if n == 0 {
            tokio::io::AsyncWriteExt::shutdown(&mut writer).await?;
            return Ok(());
        }
        let outbound_chunks = if let Some(capture) = capture.as_mut() {
            match capture.ingest(&buf[..n]).await {
                Ok(chunks) => chunks,
                Err(e) => {
                    warn!("websocket capture error: {e}");
                    return Err(e);
                }
            }
        } else {
            vec![buf[..n].to_vec()]
        };
        for chunk in outbound_chunks {
            tokio::io::AsyncWriteExt::write_all(&mut writer, &chunk).await?;
        }
    }
}
