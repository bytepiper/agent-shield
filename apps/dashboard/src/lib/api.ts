import type { BodyFile, Stats, TrafficEntry } from "./types";

async function readJson<T>(path: string): Promise<T> {
  const response = await fetch(path, {
    headers: {
      accept: "application/json",
    },
  });
  if (!response.ok) {
    throw new Error(`${path} returned ${response.status}`);
  }
  return (await response.json()) as T;
}

export function fetchStats() {
  return readJson<Stats>("/api/stats");
}

export function fetchTraffic() {
  return readJson<TrafficEntry[]>("/api/traffic");
}

export function fetchEvents() {
  return readJson<TrafficEntry[]>("/api/events");
}

export function fetchBodies() {
  return readJson<BodyFile[]>("/api/bodies");
}

export async function fetchBody(name: string) {
  const response = await fetch(`/api/body/${encodeURIComponent(name)}`);
  if (!response.ok) {
    throw new Error(`/api/body/${name} returned ${response.status}`);
  }
  return response.text();
}
