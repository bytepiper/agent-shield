import { AlertTriangle, Clock3, FileJson, Globe2, Network, Shield } from "lucide-react";

import type { TrafficEntry } from "../lib/types";
import { BodyViewer } from "./BodyViewer";

type DetailPaneProps = {
  entry: TrafficEntry | null;
  onClose: () => void;
};

function Section({
  title,
  children,
}: {
  title: string;
  children: React.ReactNode;
}) {
  return (
    <section className="space-y-3 rounded-2xl border border-white/10 bg-white/[0.03] p-4">
      <h3 className="text-xs uppercase tracking-[0.2em] text-white/45">{title}</h3>
      {children}
    </section>
  );
}

function MetaRow({
  label,
  value,
}: {
  label: string;
  value: React.ReactNode;
}) {
  return (
    <div className="grid grid-cols-[8rem_1fr] gap-3 text-sm">
      <div className="text-white/45">{label}</div>
      <div className="break-all text-white/90">{value}</div>
    </div>
  );
}

function HeaderList({
  headers,
  emptyLabel,
}: {
  headers?: TrafficEntry["req_headers"];
  emptyLabel: string;
}) {
  if (!headers?.length) {
    return <div className="text-sm text-white/45">{emptyLabel}</div>;
  }

  return (
    <div className="space-y-2">
      {headers.map((header) => (
        <div
          key={`${header.name}:${header.value}`}
          className="grid grid-cols-[10rem_1fr] gap-3 rounded-xl border border-white/5 bg-black/20 px-3 py-2 text-sm"
        >
          <div className="break-all text-sky-300">{header.name}</div>
          <div className="break-all text-white/90">{header.value}</div>
        </div>
      ))}
    </div>
  );
}

export function DetailPane({ entry, onClose }: DetailPaneProps) {
  if (!entry) {
    return null;
  }

  return (
    <aside className="space-y-4 xl:sticky xl:top-6 xl:max-h-[calc(100vh-3rem)] xl:overflow-auto xl:pr-1">
      <div className="flex items-start justify-between gap-4 rounded-[2rem] border border-white/10 bg-[radial-gradient(circle_at_top,_rgba(56,189,248,0.12),_transparent_48%),linear-gradient(180deg,rgba(255,255,255,0.08),rgba(255,255,255,0.03))] p-4">
        <div className="space-y-2">
          <div className="inline-flex items-center gap-2 rounded-full border border-white/10 bg-black/20 px-3 py-1 text-[11px] uppercase tracking-[0.22em] text-white/55">
            <Network className="size-3.5" />
            {entry.phase ?? entry.action}
          </div>
          <h2 className="text-lg font-semibold text-white">
            {entry.method ? `${entry.method} ` : ""}
            {entry.domain}
          </h2>
          <div className="break-all text-sm text-white/55">{entry.url ?? "/"}</div>
        </div>
        <button
          className="rounded-full border border-white/10 bg-black/20 px-4 py-2 text-sm text-white/70 transition hover:border-white/20 hover:text-white"
          onClick={onClose}
          type="button"
        >
          Close
        </button>
      </div>

      <Section title="Meta">
        <MetaRow label="Timestamp" value={entry.ts} />
        <MetaRow label="Action" value={entry.action} />
        <MetaRow label="Decision" value={entry.decision_action ?? "allow"} />
        <MetaRow label="Reason" value={entry.decision_reason ?? "—"} />
        <MetaRow label="Transport" value={entry.transport ?? "—"} />
        <MetaRow label="Direction" value={entry.direction ?? "—"} />
        <MetaRow label="Status" value={entry.status ?? "—"} />
        <MetaRow label="Session" value={entry.session_id ?? "—"} />
        <MetaRow
          label="Size"
          value={`${entry.req_bytes ?? 0} req / ${entry.resp_bytes ?? 0} resp`}
        />
        <MetaRow label="Latency" value={entry.duration_ms ? `${entry.duration_ms}ms` : "—"} />
      </Section>

      <Section title="Alerts">
        {entry.alerts?.length ? (
          <div className="space-y-2">
            {entry.alerts.map((alert, index) => (
              <div
                key={`${alert.pattern}-${index}`}
                className="rounded-xl border border-rose-500/20 bg-rose-500/8 p-3"
              >
                <div className="flex items-center gap-2 text-sm font-medium text-rose-200">
                  <AlertTriangle className="size-4" />
                  {alert.pattern}
                </div>
                <div className="mt-1 text-xs text-rose-100/80">
                  action={alert.action} matched={alert.matched}
                </div>
              </div>
            ))}
          </div>
        ) : (
          <div className="text-sm text-white/45">No alerts on this entry.</div>
        )}
      </Section>

      <Section title="Request Headers">
        <HeaderList headers={entry.req_headers} emptyLabel="No request headers." />
      </Section>

      <Section title="Response Headers">
        <HeaderList headers={entry.resp_headers} emptyLabel="No response headers." />
      </Section>

      <Section title="Request Body">
        <BodyViewer inlineBody={entry.req_body} fileName={entry.req_body_file} />
      </Section>

      <Section title="Response Body">
        <BodyViewer inlineBody={entry.resp_body} fileName={entry.resp_body_file} />
      </Section>

      <Section title="Quick Tags">
        <div className="flex flex-wrap gap-2">
          <div className="inline-flex items-center gap-2 rounded-full border border-white/10 bg-black/20 px-3 py-1 text-xs text-white/65">
            <Shield className="size-3.5" />
            {entry.decision_action ?? "allow"}
          </div>
          <div className="inline-flex items-center gap-2 rounded-full border border-white/10 bg-black/20 px-3 py-1 text-xs text-white/65">
            <Globe2 className="size-3.5" />
            {entry.domain}
          </div>
          <div className="inline-flex items-center gap-2 rounded-full border border-white/10 bg-black/20 px-3 py-1 text-xs text-white/65">
            <Clock3 className="size-3.5" />
            {entry.phase ?? entry.action}
          </div>
          <div className="inline-flex items-center gap-2 rounded-full border border-white/10 bg-black/20 px-3 py-1 text-xs text-white/65">
            <FileJson className="size-3.5" />
            {entry.content_type ?? "unknown"}
          </div>
        </div>
      </Section>
    </aside>
  );
}
