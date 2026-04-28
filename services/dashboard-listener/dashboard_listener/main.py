from __future__ import annotations

import os
from pathlib import Path
from typing import Any

from fastapi import FastAPI, HTTPException, Query, Request
from fastapi.responses import HTMLResponse
from fastapi.staticfiles import StaticFiles

from .demo import demo_events
from .models import ListenerEvent
from .store import store
from .views import (
    detail_context,
    filters_context,
    is_llm_event,
    normalize_filters,
    normalize_sort,
    render,
    stats_context,
    traffic_context,
)

BASE_DIR = Path(__file__).resolve().parent
STATIC_DIR = BASE_DIR / "static"

app = FastAPI(title="agent-shield-dashboard-listener")
app.mount("/static", StaticFiles(directory=STATIC_DIR), name="static")


def env_enabled(name: str) -> bool:
    return os.getenv(name, "").strip().lower() in {"1", "true", "yes", "on"}


@app.on_event("startup")
async def seed_demo_data() -> None:
    if not env_enabled("AGENT_SHIELD_DASHBOARD_SEED_DEMO"):
        return
    if store.snapshot():
        return
    for item in demo_events():
        store.add(item)


def base_context(
    q: str = "",
    domain: str = "",
    method: str = "",
    phase: str = "",
    sort: str = "time",
    dir: str = "desc",
) -> dict[str, Any]:
    sort_by, sort_dir = normalize_sort(sort, dir)
    query, domain_filter, method_filter, phase_filter = normalize_filters(q, domain, method, phase)
    snapshot = store.snapshot()
    return {
        "stats": stats_context(snapshot),
        "filters": filters_context(query, domain_filter, method_filter, phase_filter, sort_by, sort_dir),
        "traffic": traffic_context(snapshot, sort_by, sort_dir, query, domain_filter, method_filter, phase_filter),
    }


@app.get("/", response_class=HTMLResponse)
async def index() -> str:
    return render("index.html", **base_context())


@app.get("/fragments/stats", response_class=HTMLResponse)
async def stats_fragment() -> str:
    return render("_stats.html", stats=stats_context(store.snapshot()))


@app.get("/fragments/traffic", response_class=HTMLResponse)
async def traffic_fragment(
    sort: str = Query(default="time"),
    dir: str = Query(default="desc"),
    q: str = Query(default=""),
    domain: str = Query(default=""),
    method: str = Query(default=""),
    phase: str = Query(default=""),
) -> str:
    sort_by, sort_dir = normalize_sort(sort, dir)
    query, domain_filter, method_filter, phase_filter = normalize_filters(q, domain, method, phase)
    return render(
        "_traffic.html",
        traffic=traffic_context(
            store.snapshot(),
            sort_by,
            sort_dir,
            query,
            domain_filter,
            method_filter,
            phase_filter,
        ),
    )


@app.get("/fragments/detail/{listener_id}", response_class=HTMLResponse)
async def detail_fragment(listener_id: int, tab: str = Query(default="overview")) -> str:
    event = store.find(listener_id)
    if not event:
        raise HTTPException(status_code=404, detail="event not found")
    return render("_detail.html", detail=detail_context(event, tab))


@app.post("/listener")
async def listener(payload: ListenerEvent, request: Request) -> dict[str, Any]:
    item = payload.model_dump()
    if not is_llm_event(item):
        return {"ok": True, "stored": False}

    item["remote"] = request.client.host if request.client else None
    stored = store.add(item)
    return {"ok": True, "stored": True, "count": len(store.snapshot()), "listener_id": stored["listener_id"]}
