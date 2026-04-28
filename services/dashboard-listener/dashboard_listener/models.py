from __future__ import annotations

from typing import Any

from pydantic import BaseModel


class ListenerEvent(BaseModel):
    schema_version: str
    event_id: int
    session_id: int | None = None
    event_seq: int
    session_seq: int
    phase: str
    transport: str
    direction: str | None = None
    method: str | None = None
    url: str | None = None
    domain: str
    status: int | None = None
    action: str
    content_type: str | None = None
    traffic_class: str
    req_headers: list[dict[str, Any]] = []
    resp_headers: list[dict[str, Any]] = []
    req_body: dict[str, Any] | None = None
    resp_body: dict[str, Any] | None = None
    primary_text: str | None = None
    preview: str | None = None
