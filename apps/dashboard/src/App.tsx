import { useQuery } from "@tanstack/react-query";
import { startTransition, useEffect, useMemo, useState } from "react";

import { fetchBodies, fetchBody, fetchStats, fetchTraffic } from "./lib/api";
import type { BodyFile, EventBody, Stats, TrafficEntry } from "./lib/types";

type TabKey = "traffic" | "packets";
type DetailTabKey = "overview" | "reqh" | "resph" | "req" | "resp";

type PacketDetail =
  | { kind: "event"; entry: TrafficEntry }
  | { kind: "body"; title: string; bodyText: string };

function formatBytes(bytes?: number | null) {
  if (!bytes) return "";
  return bytes > 1024 ? `${(bytes / 1024).toFixed(1)}KB` : `${bytes}B`;
}

function eventSize(entry: TrafficEntry) {
  return `${entry.req_bytes ?? ""}${entry.req_bytes && entry.resp_bytes ? "→" : ""}${entry.resp_bytes ?? ""}`;
}

function eventTime(ts?: string | null) {
  if (!ts) return "";
  return ts.split("T")[1]?.split(".")[0] ?? ts;
}

function prettyText(text: string) {
  try {
    return JSON.stringify(JSON.parse(text), null, 2);
  } catch {
    return text;
  }
}

function bodyText(body?: EventBody | null) {
  if (!body) return "";
  if (body.text) return prettyText(body.text);
  if (body.base64) {
    return JSON.stringify(
      {
        encoding: "base64",
        data: body.base64,
        bytes: body.bytes,
        truncated: body.truncated,
      },
      null,
      2,
    );
  }
  return "";
}

function HeaderGrid({
  headers,
}: {
  headers?: TrafficEntry["req_headers"];
}) {
  if (!headers?.length) {
    return <div className="null" data-testid="headers-empty">No headers</div>;
  }

  return (
    <div className="headers-grid" data-testid="headers-grid">
      {headers.map((header) => (
        <div data-testid="header-row" key={`${header.name}:${header.value}`}>
          <div className="headers-name">{header.name}</div>
          <div className="headers-value">{header.value}</div>
        </div>
      ))}
    </div>
  );
}

function BodyPanel({
  inlineBody,
  fileName,
}: {
  inlineBody?: EventBody | null;
  fileName?: string | null;
}) {
  const [copied, setCopied] = useState(false);
  const bodyQuery = useQuery({
    queryKey: ["body", fileName],
    queryFn: () => fetchBody(fileName!),
    enabled: !inlineBody?.text && !inlineBody?.base64 && Boolean(fileName),
    staleTime: Infinity,
  });

  const content = useMemo(() => {
    const inline = bodyText(inlineBody);
    if (inline) return inline;
    if (bodyQuery.data) return prettyText(bodyQuery.data);
    return "";
  }, [bodyQuery.data, inlineBody]);

  useEffect(() => {
    if (!copied) return;
    const timeoutId = window.setTimeout(() => setCopied(false), 1200);
    return () => window.clearTimeout(timeoutId);
  }, [copied]);

  if (!inlineBody && !fileName) {
    return <div className="null" data-testid="body-empty">No body</div>;
  }

  if (bodyQuery.isLoading) {
    return <div className="purple" data-testid="body-loading">Loading...</div>;
  }

  if (bodyQuery.error instanceof Error) {
    return <div className="red" data-testid="body-error">Error: {bodyQuery.error.message}</div>;
  }

  return (
    <div className="body-panel" data-testid="body-panel">
      <div className="body-toolbar">
        <button
          className="body-btn copy"
          data-testid="copy-body"
          onClick={async () => {
            await navigator.clipboard.writeText(content);
            setCopied(true);
          }}
          type="button"
        >
          {copied ? "COPIED" : "COPY"}
        </button>
      </div>
      <pre className="detail-pre">{content}</pre>
    </div>
  );
}

function EventOverview({ entry }: { entry: TrafficEntry }) {
  return (
    <pre className="detail-pre" data-testid="detail-overview">
      {JSON.stringify(
        {
          id: entry.id,
          ts: entry.ts,
          action: entry.action,
          phase: entry.phase,
          method: entry.method,
          domain: entry.domain,
          url: entry.url,
          status: entry.status,
          req_bytes: entry.req_bytes,
          resp_bytes: entry.resp_bytes,
          duration_ms: entry.duration_ms,
          decision_action: entry.decision_action,
          decision_reason: entry.decision_reason,
          alerts: entry.alerts ?? [],
        },
        null,
        2,
      )}
    </pre>
  );
}

function detailTabs(detail: PacketDetail | null): DetailTabKey[] {
  if (!detail) return [];
  if (detail.kind === "body") return ["overview"];

  const tabs: DetailTabKey[] = ["overview"];
  if (detail.entry.req_headers?.length) tabs.push("reqh");
  if (detail.entry.resp_headers?.length) tabs.push("resph");
  if (detail.entry.req_body_file || detail.entry.req_body) tabs.push("req");
  if (detail.entry.resp_body_file || detail.entry.resp_body) tabs.push("resp");
  return tabs;
}

function DetailPane({
  detail,
  detailTab,
  onClose,
  onTabChange,
}: {
  detail: PacketDetail | null;
  detailTab: DetailTabKey;
  onClose: () => void;
  onTabChange: (tab: DetailTabKey) => void;
}) {
  const tabs = detailTabs(detail);
  const open = Boolean(detail);

  return (
    <div
      className={`detail-pane ${open ? "open" : ""}`}
      data-testid="detail-pane"
    >
      {detail && (
        <>
          <div className="detail-header" data-testid="detail-header">
            <span>
              {detail.kind === "event"
                ? `${detail.entry.method ?? ""} ${detail.entry.domain} ${detail.entry.url ?? ""}`.trim()
                : detail.title}
            </span>
            <span className="detail-close" data-testid="detail-close" onClick={onClose}>
              ✕
            </span>
          </div>
          <div className="detail-tabs" data-testid="detail-tabs">
            {tabs.map((tab) => (
              <div
                className={`detail-tab ${detailTab === tab ? "active" : ""}`}
                data-testid={`detail-tab-${tab}`}
                key={tab}
                onClick={() => onTabChange(tab)}
              >
                {tab === "overview"
                  ? "Overview"
                  : tab === "reqh"
                    ? "Request Headers"
                    : tab === "resph"
                      ? "Response Headers"
                      : tab === "req"
                        ? "Request Body"
                        : "Response Body"}
              </div>
            ))}
          </div>
          <div className="detail-body">
            {detail.kind === "body" && <pre className="detail-pre" data-testid="packet-body-viewer">{detail.bodyText}</pre>}
            {detail.kind === "event" && detailTab === "overview" && (
              <EventOverview entry={detail.entry} />
            )}
            {detail.kind === "event" && detailTab === "reqh" && (
              <HeaderGrid headers={detail.entry.req_headers} />
            )}
            {detail.kind === "event" && detailTab === "resph" && (
              <HeaderGrid headers={detail.entry.resp_headers} />
            )}
            {detail.kind === "event" && detailTab === "req" && (
              <BodyPanel
                fileName={detail.entry.req_body_file}
                inlineBody={detail.entry.req_body}
              />
            )}
            {detail.kind === "event" && detailTab === "resp" && (
              <BodyPanel
                fileName={detail.entry.resp_body_file}
                inlineBody={detail.entry.resp_body}
              />
            )}
          </div>
        </>
      )}
    </div>
  );
}

function BodyButtons({
  entry,
  onOpenBody,
}: {
  entry: TrafficEntry;
  onOpenBody: (title: string, bodyText: string) => void;
}) {
  const buttons: React.ReactNode[] = [];

  if (entry.req_body_file || entry.req_body) {
    buttons.push(
      <button
        className="body-btn req"
        data-testid="open-request-body"
        key="req"
        onClick={async (event) => {
          event.stopPropagation();
          const text = entry.req_body
            ? bodyText(entry.req_body)
            : await fetchBody(entry.req_body_file!);
          onOpenBody(`Request Body #${entry.id}`, prettyText(text));
        }}
        type="button"
      >
        REQ
      </button>,
    );
  }

  if (entry.resp_body_file || entry.resp_body) {
    buttons.push(
      <button
        className="body-btn resp"
        data-testid="open-response-body"
        key="resp"
        onClick={async (event) => {
          event.stopPropagation();
          const text = entry.resp_body
            ? bodyText(entry.resp_body)
            : await fetchBody(entry.resp_body_file!);
          onOpenBody(`Response Body #${entry.id}`, prettyText(text));
        }}
        type="button"
      >
        RESP
      </button>,
    );
  }

  if (!buttons.length) return null;
  return <div className="body-actions">{buttons}</div>;
}

function StatsBar({
  stats,
  packetsCount,
}: {
  stats?: Stats;
  packetsCount: number;
}) {
  return (
    <div className="stats" data-testid="stats-bar">
      <div className="stat" data-testid="stat-requests">
        <div className="stat-val cyan">{stats?.total_requests ?? 0}</div>
        <div className="stat-label">requests</div>
      </div>
      <div className="stat" data-testid="stat-blocked">
        <div className="stat-val red">{stats?.total_blocked ?? 0}</div>
        <div className="stat-label">blocked</div>
      </div>
      <div className="stat" data-testid="stat-packets">
        <div className="stat-val orange">{packetsCount}</div>
        <div className="stat-label">packets</div>
      </div>
      <div className="stat" data-testid="stat-domains">
        <div className="stat-val green">{stats ? Object.keys(stats.domains).length : 0}</div>
        <div className="stat-label">domains</div>
      </div>
    </div>
  );
}

export function App() {
  const [tab, setTab] = useState<TabKey>("traffic");
  const [detail, setDetail] = useState<PacketDetail | null>(null);
  const [detailTab, setDetailTab] = useState<DetailTabKey>("overview");

  const statsQuery = useQuery({
    queryKey: ["stats"],
    queryFn: fetchStats,
    refetchInterval: 2_000,
  });
  const trafficQuery = useQuery({
    queryKey: ["traffic"],
    queryFn: fetchTraffic,
    refetchInterval: 2_000,
  });
  const bodiesQuery = useQuery({
    queryKey: ["bodies"],
    queryFn: fetchBodies,
    refetchInterval: 2_000,
  });

  const stats = statsQuery.data;
  const rows = trafficQuery.data ?? [];
  const bodies = bodiesQuery.data ?? [];

  useEffect(() => {
    if (detail?.kind !== "event") return;
    const next = trafficQuery.data?.find((entry) => entry.id === detail.entry.id);
    if (next) {
      startTransition(() => setDetail({ kind: "event", entry: next }));
    }
  }, [detail, trafficQuery.data]);

  return (
    <div className="legacy-dashboard" data-testid="dashboard-root">
      <h1 data-testid="dashboard-title">Agent Shield</h1>
      <StatsBar packetsCount={bodies.length} stats={stats} />
      <div className="tabs" data-testid="main-tabs">
        <div
          className={`tab ${tab === "traffic" ? "active" : ""}`}
          data-testid="tab-traffic"
          onClick={() => {
            setTab("traffic");
            setDetail(null);
          }}
        >
          Traffic
        </div>
        <div
          className={`tab ${tab === "packets" ? "active" : ""}`}
          data-testid="tab-packets"
          onClick={() => {
            setTab("packets");
            setDetail(null);
          }}
        >
          Packets
        </div>
      </div>
      <div className="main" data-testid="dashboard-main">
        <div className="list-pane" data-testid="list-pane">
          {tab === "packets" ? (
            <div data-testid="packet-body-list">
              {bodies.map((body: BodyFile) => (
                <div
                  className="body-item"
                  data-testid="packet-body-list-item"
                  key={body.name}
                  onClick={async () => {
                    const text = await fetchBody(body.name);
                    setDetail({
                      kind: "body",
                      title: body.name,
                      bodyText: prettyText(text),
                    });
                    setDetailTab("overview");
                  }}
                >
                  <span
                    className={`body-name ${
                      body.name.includes("_req")
                        ? "body-req"
                        : body.name.includes("_resp")
                          ? "body-resp"
                          : body.name.includes("_blocked")
                            ? "body-blocked"
                            : ""
                    }`}
                  >
                    {body.name}
                  </span>
                  <span className="body-size">{formatBytes(body.size)}</span>
                </div>
              ))}
            </div>
          ) : (
            <table data-testid="traffic-table">
              <thead>
                <tr>
                  <th>Time</th>
                  <th>Action</th>
                  <th>Method</th>
                  <th>Domain</th>
                  <th>URL</th>
                  <th>Bodies</th>
                  <th>Size</th>
                  <th>ms</th>
                </tr>
              </thead>
              <tbody>
                {rows.slice(-200).reverse().map((entry) => (
                  <tr
                    className={entry.alerts?.length ? "alert-row" : ""}
                    data-testid="traffic-row"
                    key={`${entry.id}-${entry.ts}`}
                    onClick={() => {
                      setDetail({ kind: "event", entry });
                      setDetailTab("overview");
                    }}
                  >
                    <td>{eventTime(entry.ts)}</td>
                    <td className={`action-${(entry.action ?? "").replace(/\s/g, "-")}`}>
                      {entry.action}
                    </td>
                    <td>{entry.method ?? ""}</td>
                    <td>{entry.domain}</td>
                    <td title={entry.url ?? ""}>
                      <div className="cell-main">
                        {(entry.url ?? "").slice(0, 80)}{" "}
                        {(entry.alerts ?? []).map((alert, index) => (
                          <span className="red" key={`${alert.pattern}-${index}`}>
                            [{alert.pattern}]
                          </span>
                        ))}
                      </div>
                      {entry.preview ? (
                        <div className="cell-preview" title={entry.preview}>
                          {entry.preview}
                        </div>
                      ) : null}
                    </td>
                    <td>
                      <BodyButtons
                        entry={entry}
                        onOpenBody={(title, bodyTextValue) => {
                          setDetail({ kind: "body", title, bodyText: bodyTextValue });
                          setDetailTab("overview");
                        }}
                      />
                    </td>
                    <td>{eventSize(entry)}</td>
                    <td>{entry.duration_ms ?? ""}</td>
                  </tr>
                ))}
              </tbody>
            </table>
          )}
        </div>
        <DetailPane
          detail={detail}
          detailTab={detailTab}
          onClose={() => setDetail(null)}
          onTabChange={setDetailTab}
        />
      </div>
      {(statsQuery.error instanceof Error ||
        trafficQuery.error instanceof Error ||
        bodiesQuery.error instanceof Error) && (
        <div className="error-banner" data-testid="error-banner">
          {(statsQuery.error instanceof Error && statsQuery.error.message) ||
            (trafficQuery.error instanceof Error && trafficQuery.error.message) ||
            (bodiesQuery.error instanceof Error && bodiesQuery.error.message)}
        </div>
      )}
    </div>
  );
}
