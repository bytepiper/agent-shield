from __future__ import annotations

import json
from pathlib import Path
from typing import Any

from jinja2 import Environment, FileSystemLoader, select_autoescape
from markupsafe import Markup

VALID_SORT_FIELDS = {"time", "action", "method", "domain", "url", "bodies", "size", "flow"}
VALID_SORT_DIRS = {"asc", "desc"}

BASE_DIR = Path(__file__).resolve().parent
TEMPLATES_DIR = BASE_DIR / "templates"

env = Environment(
    loader=FileSystemLoader(TEMPLATES_DIR),
    autoescape=select_autoescape(["html", "xml"]),
    trim_blocks=True,
    lstrip_blocks=True,
)


def render(template_name: str, **context: Any) -> str:
    return env.get_template(template_name).render(**context)


def is_llm_event(event: dict[str, Any]) -> bool:
    if event.get("phase") == "connect.pre":
        return False

    if event.get("traffic_class") in {"model_http", "model_ws"}:
        return True

    domain = str(event.get("domain") or "").lower()
    url = str(event.get("url") or "").lower()
    llm_domains = {
        "api.anthropic.com",
        "api.openai.com",
        "chatgpt.com",
        "ab.chatgpt.com",
        "cloudcode-pa.googleapis.com",
    }
    llm_paths = (
        "/v1/messages",
        "/backend-api/",
        "/responses",
        "streamgeneratecontent",
    )
    return domain in llm_domains or any(path in url for path in llm_paths)


def event_time(value: str | None) -> str:
    if not value:
        return ""
    return value.split("T")[1].split(".")[0]


def body_bytes(body: dict[str, Any] | None) -> int:
    if not body:
        return 0
    value = body.get("bytes")
    if isinstance(value, int):
        return value
    text = body.get("text")
    if isinstance(text, str):
        return len(text.encode())
    base64_value = body.get("base64")
    if isinstance(base64_value, str):
        return len(base64_value)
    return 0


def body_text(body: dict[str, Any] | None) -> str:
    if not body:
        return ""
    text = body.get("text")
    if isinstance(text, str):
        return text
    base64_value = body.get("base64")
    if isinstance(base64_value, str):
        return json.dumps(
            {
                "encoding": "base64",
                "bytes": body.get("bytes"),
                "truncated": body.get("truncated"),
                "data": base64_value,
            },
            ensure_ascii=False,
            indent=2,
        )
    return ""


def pretty_text(value: str) -> str:
    if not value:
        return ""
    try:
        return json.dumps(json.loads(value), ensure_ascii=False, indent=2)
    except Exception:
        return value


def render_pre(value: str, empty: str) -> Markup:
    text = pretty_text(value)
    if not text:
        return Markup(f'<div class="text-zinc-500" data-testid="listener-detail-empty">{empty}</div>')
    data = text.replace("&", "&amp;").replace("<", "&lt;").replace(">", "&gt;").replace('"', "&quot;")
    escaped = text.replace("&", "&amp;").replace("<", "&lt;").replace(">", "&gt;")
    return Markup(
        '<div class="relative min-w-0 overflow-x-hidden" data-testid="listener-body-viewer">'
        '<button class="absolute right-0 top-0 rounded border border-zinc-700 px-2 py-1 text-[10px] text-zinc-400 hover:border-zinc-500 hover:text-zinc-200" '
        f'data-testid="listener-copy-body" data-copy="{data}" onclick="navigator.clipboard.writeText(this.dataset.copy);">COPY</button>'
        f'<pre class="max-w-full whitespace-pre-wrap break-words pr-14 text-[11px] leading-5 text-zinc-200" data-testid="listener-body-pre">{escaped}</pre>'
        "</div>"
    )


def render_headers(headers: list[dict[str, Any]]) -> Markup:
    if not headers:
        return Markup('<div class="text-zinc-500" data-testid="listener-headers-empty">No headers</div>')
    rows = []
    for header in headers:
        name = str(header.get("name", "")).replace("&", "&amp;").replace("<", "&lt;").replace(">", "&gt;")
        value = str(header.get("value", "")).replace("&", "&amp;").replace("<", "&lt;").replace(">", "&gt;")
        rows.append(
            '<div class="contents" data-testid="listener-header-row">'
            f'<div class="border-b border-zinc-900 px-3 py-2 text-cyan-300 break-words">{name}</div>'
            f'<div class="min-w-0 border-b border-zinc-900 px-3 py-2 text-zinc-200 whitespace-pre-wrap break-words">{value}</div>'
            "</div>"
        )
    return Markup(
        '<div class="grid grid-cols-[minmax(11rem,15rem)_minmax(0,1fr)] rounded-lg border border-zinc-800 bg-zinc-950 text-xs" data-testid="listener-headers-grid">'
        + "".join(rows)
        + "</div>"
    )


def normalize_sort(sort_by: str | None, sort_dir: str | None) -> tuple[str, str]:
    field = sort_by if sort_by in VALID_SORT_FIELDS else "time"
    direction = sort_dir if sort_dir in VALID_SORT_DIRS else "desc"
    return field, direction


def normalize_filters(
    q: str | None,
    domain: str | None,
    method: str | None,
    phase: str | None,
) -> tuple[str, str, str, str]:
    return (
        (q or "").strip(),
        (domain or "").strip(),
        (method or "").strip(),
        (phase or "").strip(),
    )


def event_matches(
    item: dict[str, Any],
    q: str,
    domain: str,
    method: str,
    phase: str,
) -> bool:
    if domain and domain.lower() not in str(item.get("domain") or "").lower():
        return False
    if method and method.lower() != str(item.get("method") or "").lower():
        return False
    if phase and phase.lower() != str(item.get("phase") or "").lower():
        return False
    if q:
        haystack = " ".join(
            [
                str(item.get("domain") or ""),
                str(item.get("url") or ""),
                str(item.get("preview") or ""),
                str(item.get("primary_text") or ""),
                body_text(item.get("req_body")),
                body_text(item.get("resp_body")),
                str(item.get("phase") or ""),
                str(item.get("method") or ""),
                str(item.get("transport") or ""),
            ]
        ).lower()
        if q.lower() not in haystack:
            return False
    return True


def event_sort_value(item: dict[str, Any], sort_by: str) -> Any:
    if sort_by == "time":
        return item.get("received_at") or ""
    if sort_by == "action":
        return (item.get("action") or item.get("phase") or "").lower()
    if sort_by == "method":
        return (item.get("method") or "").lower()
    if sort_by == "domain":
        return (item.get("domain") or "").lower()
    if sort_by == "url":
        return ((item.get("url") or "") + " " + (item.get("preview") or item.get("primary_text") or "")).lower()
    if sort_by == "bodies":
        return (
            int(bool(item.get("req_body"))) + int(bool(item.get("resp_body"))),
            int(bool(item.get("req_body"))),
            int(bool(item.get("resp_body"))),
        )
    if sort_by == "size":
        return body_bytes(item.get("req_body")) + body_bytes(item.get("resp_body"))
    if sort_by == "flow":
        return (item.get("transport") or "").lower()
    return item.get("received_at") or ""


def next_sort_dir(current_field: str, current_dir: str, header_field: str) -> str:
    if current_field == header_field:
        return "asc" if current_dir == "desc" else "desc"
    return "desc" if header_field == "time" else "asc"


def header_label(label: str, current_field: str, current_dir: str, header_field: str) -> str:
    if current_field != header_field:
        return label
    arrow = "↓" if current_dir == "desc" else "↑"
    return f"{label} {arrow}"


def header_defs(sort_by: str, sort_dir: str) -> list[dict[str, str]]:
    defs: list[tuple[str, str]] = [
        ("time", "Time"),
        ("action", "Action"),
        ("method", "Method"),
        ("domain", "Domain"),
        ("url", "URL"),
        ("bodies", "Bodies"),
        ("size", "Size"),
        ("flow", "Flow"),
    ]
    rows = []
    for field, label in defs:
        rows.append(
            {
                "field": field,
                "label": header_label(label, sort_by, sort_dir, field),
                "next_dir": next_sort_dir(sort_by, sort_dir, field),
            }
        )
    return rows


def stats_context(snapshot: list[dict[str, Any]]) -> dict[str, Any]:
    domains = len({item["domain"] for item in snapshot})
    sessions = len({item.get("session_id") for item in snapshot if item.get("session_id") is not None})
    updated = event_time(snapshot[-1].get("received_at")) if snapshot else "idle"
    return {
        "events_count": len(snapshot),
        "domains_count": domains,
        "sessions_count": sessions,
        "updated": updated,
    }


def filters_context(q: str, domain: str, method: str, phase: str, sort_by: str, sort_dir: str) -> dict[str, Any]:
    return {
        "q": q,
        "domain": domain,
        "method": method,
        "phase": phase,
        "sort_by": sort_by,
        "sort_dir": sort_dir,
    }


def traffic_context(
    snapshot: list[dict[str, Any]],
    sort_by: str,
    sort_dir: str,
    q: str,
    domain: str,
    method: str,
    phase: str,
) -> dict[str, Any]:
    events = [item for item in snapshot if event_matches(item, q, domain, method, phase)]
    events.sort(key=lambda item: event_sort_value(item, sort_by), reverse=sort_dir == "desc")

    rows = []
    for item in events:
        rows.append(
            {
                "listener_id": item["listener_id"],
                "time": event_time(item.get("received_at")),
                "action": item.get("action") or item.get("phase") or "",
                "action_class": "text-rose-400" if (item.get("status") or 0) >= 400 else "text-cyan-300",
                "method": item.get("method") or "",
                "domain": item.get("domain") or "",
                "url": (item.get("url") or "")[:180],
                "preview": (item.get("preview") or item.get("primary_text") or "")[:140],
                "has_req_body": bool(item.get("req_body")),
                "has_resp_body": bool(item.get("resp_body")),
                "size": (
                    f"{body_bytes(item.get('req_body')) if body_bytes(item.get('req_body')) else ''}"
                    f"{'→' if body_bytes(item.get('req_body')) and body_bytes(item.get('resp_body')) else ''}"
                    f"{body_bytes(item.get('resp_body')) if body_bytes(item.get('resp_body')) else ''}"
                )
                or "-",
                "flow": item.get("transport") or "",
            }
        )

    return {
        "headers": header_defs(sort_by, sort_dir),
        "rows": rows,
    }


def available_tabs(event: dict[str, Any]) -> list[dict[str, str]]:
    tabs: list[dict[str, str]] = [{"name": "overview", "label": "Overview"}]
    if event.get("req_headers"):
        tabs.append({"name": "reqh", "label": "Request Headers"})
    if event.get("resp_headers"):
        tabs.append({"name": "resph", "label": "Response Headers"})
    if event.get("req_body"):
        tabs.append({"name": "req", "label": "Request Body"})
    if event.get("resp_body"):
        tabs.append({"name": "resp", "label": "Response Body"})
    return tabs


def detail_context(event: dict[str, Any], tab: str) -> dict[str, Any]:
    tabs = available_tabs(event)
    allowed_tabs = {item["name"] for item in tabs}
    current_tab = tab if tab in allowed_tabs else "overview"

    summary = {
        "listener_id": event["listener_id"],
        "event_id": event["event_id"],
        "session_id": event.get("session_id"),
        "event_seq": event.get("event_seq"),
        "session_seq": event.get("session_seq"),
        "phase": event.get("phase"),
        "transport": event.get("transport"),
        "direction": event.get("direction"),
        "method": event.get("method"),
        "domain": event.get("domain"),
        "url": event.get("url"),
        "traffic_class": event.get("traffic_class"),
        "status": event.get("status"),
        "received_at": event.get("received_at"),
    }

    if current_tab == "overview":
        content_html = render_pre(json.dumps(summary, ensure_ascii=False, indent=2), "No details")
    elif current_tab == "reqh":
        content_html = render_headers(event.get("req_headers") or [])
    elif current_tab == "resph":
        content_html = render_headers(event.get("resp_headers") or [])
    elif current_tab == "req":
        content_html = render_pre(body_text(event.get("req_body")), "No body")
    elif current_tab == "resp":
        content_html = render_pre(body_text(event.get("resp_body")), "No body")
    else:
        content_html = Markup('<div class="text-zinc-500">Unsupported tab</div>')

    for item in tabs:
        item["active"] = item["name"] == current_tab

    return {
        "event": event,
        "tabs": tabs,
        "content_html": content_html,
    }
