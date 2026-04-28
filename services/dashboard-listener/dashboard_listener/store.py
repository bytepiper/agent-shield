from __future__ import annotations

from collections import deque
from datetime import datetime, timezone
from itertools import count
from threading import Lock
from typing import Any

MAX_EVENTS = 500


def now_iso() -> str:
    return datetime.now(timezone.utc).isoformat()


class EventStore:
    def __init__(self, max_events: int = MAX_EVENTS) -> None:
        self._events: deque[dict[str, Any]] = deque(maxlen=max_events)
        self._lock = Lock()
        self._ids = count(1)

    def add(self, item: dict[str, Any]) -> dict[str, Any]:
        stored = dict(item)
        stored["listener_id"] = next(self._ids)
        stored["received_at"] = now_iso()
        with self._lock:
            self._events.append(stored)
        return stored

    def snapshot(self) -> list[dict[str, Any]]:
        with self._lock:
            return list(self._events)

    def find(self, listener_id: int) -> dict[str, Any] | None:
        with self._lock:
            for item in self._events:
                if int(item["listener_id"]) == listener_id:
                    return dict(item)
        return None


store = EventStore()
