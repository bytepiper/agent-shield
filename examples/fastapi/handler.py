import json
import logging

from fastapi import FastAPI, Request

logging.basicConfig(level=logging.INFO, format="%(asctime)s %(levelname)s %(message)s")

app = FastAPI(title="agent-shield-handler-demo")


@app.post("/handler")
async def handler(request: Request):
    payload = await request.json()
    logging.info("handler processing: %s", json.dumps(payload, ensure_ascii=False))

    text = payload.get("primary_text")
    phase = payload.get("phase")
    direction = payload.get("direction")

    should_modify = bool(text) and (
        (phase == "http.request" and direction == "out")
        or (phase == "ws.message.out" and direction == "out")
        or (phase == "sse.event.in" and direction == "in")
    )

    if not should_modify:
        return {"action": "allow", "reason": "demo_no_text"}

    return {
        "action": "modify",
        "reason": "demo_append_hello",
        "text": f"{text} hello",
    }
