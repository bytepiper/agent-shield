import json
import logging

from fastapi import FastAPI, Request

logging.basicConfig(level=logging.INFO, format="%(asctime)s %(levelname)s %(message)s")

app = FastAPI(title="agent-shield-listener-demo")


@app.post("/listener")
async def listener(request: Request):
    payload = await request.json()
    logging.info("listener intercepted: %s", json.dumps(payload, ensure_ascii=False))
    return {"ok": True}
