import {
  flexRender,
  getCoreRowModel,
  getSortedRowModel,
  useReactTable,
  type ColumnDef,
  type SortingState,
} from "@tanstack/react-table";
import { ArrowDown, ArrowUp, ArrowUpDown, FileSearch } from "lucide-react";
import { startTransition, useDeferredValue, useMemo, useState } from "react";

import type { TrafficEntry } from "../lib/types";

type TrafficTableProps = {
  data: TrafficEntry[];
  selectedId: number | null;
  onSelect: (entry: TrafficEntry) => void;
};

function decisionTone(entry: TrafficEntry) {
  if (entry.decision_action === "block") return "text-rose-300";
  if (entry.decision_action === "modify" || entry.decision_action === "replace") {
    return "text-amber-300";
  }
  return "text-emerald-300";
}

function sortableHeader(label: string, sorted: false | "asc" | "desc") {
  if (sorted === "asc") return <ArrowUp className="size-3.5" />;
  if (sorted === "desc") return <ArrowDown className="size-3.5" />;
  return (
    <>
      <span>{label}</span>
      <ArrowUpDown className="size-3.5 opacity-40" />
    </>
  );
}

export function TrafficTable({
  data,
  selectedId,
  onSelect,
}: TrafficTableProps) {
  const [search, setSearch] = useState("");
  const [sorting, setSorting] = useState<SortingState>([
    { id: "ts", desc: true },
  ]);
  const deferredSearch = useDeferredValue(search);
  const needle = deferredSearch.trim().toLowerCase();

  const indexedEntries = useMemo(
    () =>
      data.map((entry) => ({
        entry,
        haystack: [
          entry.domain,
          entry.method,
          entry.url,
          entry.action,
          entry.phase,
          entry.decision_action,
          entry.decision_reason,
          entry.preview,
        ]
          .filter(Boolean)
          .join(" ")
          .toLowerCase(),
      })),
    [data],
  );

  const filtered = useMemo(() => {
    if (!needle) return data;
    return indexedEntries
      .filter((indexed) => indexed.haystack.includes(needle))
      .map((indexed) => indexed.entry);
  }, [data, indexedEntries, needle]);

  const renderData = useMemo(() => filtered.slice(0, 400), [filtered]);

  const columns = useMemo<ColumnDef<TrafficEntry>[]>(
    () => [
      {
        accessorKey: "ts",
        header: ({ column }) => (
          <button
            className="inline-flex items-center gap-2"
            onClick={() => column.toggleSorting(column.getIsSorted() === "asc")}
            type="button"
          >
            {sortableHeader("Time", column.getIsSorted())}
          </button>
        ),
        cell: ({ row }) => (
          <div className="font-medium text-white/85">{row.original.ts}</div>
        ),
      },
      {
        accessorKey: "domain",
        header: ({ column }) => (
          <button
            className="inline-flex items-center gap-2"
            onClick={() => column.toggleSorting(column.getIsSorted() === "asc")}
            type="button"
          >
            {sortableHeader("Domain", column.getIsSorted())}
          </button>
        ),
        cell: ({ row }) => (
          <div className="space-y-1">
            <div className="font-medium text-white">{row.original.domain}</div>
            <div className="text-xs text-white/45">{row.original.url ?? "/"}</div>
          </div>
        ),
      },
      {
        id: "flow",
        header: "Flow",
        cell: ({ row }) => (
          <div className="space-y-1">
            <div className="text-sm text-white/80">
              {(row.original.method ?? row.original.action).toUpperCase()}
            </div>
            <div className="text-xs uppercase tracking-[0.18em] text-white/40">
              {row.original.phase ?? row.original.action}
            </div>
          </div>
        ),
      },
      {
        id: "decision",
        header: "Decision",
        cell: ({ row }) => (
          <div className={`space-y-1 ${decisionTone(row.original)}`}>
            <div className="font-medium">
              {row.original.decision_action ?? "allow"}
            </div>
            <div className="text-xs text-white/45">
              {row.original.decision_reason ?? "—"}
            </div>
          </div>
        ),
      },
      {
        id: "size_total",
        accessorFn: (row) => (row.req_bytes ?? 0) + (row.resp_bytes ?? 0),
        header: ({ column }) => (
          <button
            className="inline-flex items-center gap-2"
            onClick={() => column.toggleSorting(column.getIsSorted() === "asc")}
            type="button"
          >
            {sortableHeader("Size", column.getIsSorted())}
          </button>
        ),
        cell: ({ row }) => (
          <div className="text-xs text-white/60">
            {row.original.req_bytes ?? 0} / {row.original.resp_bytes ?? 0}
          </div>
        ),
      },
      {
        accessorKey: "duration_ms",
        header: ({ column }) => (
          <button
            className="inline-flex items-center gap-2"
            onClick={() => column.toggleSorting(column.getIsSorted() === "asc")}
            type="button"
          >
            {sortableHeader("ms", column.getIsSorted())}
          </button>
        ),
        cell: ({ row }) => (
          <div className="text-xs text-white/60">
            {row.original.duration_ms ? `${row.original.duration_ms}ms` : "—"}
          </div>
        ),
      },
    ],
    [],
  );

  const table = useReactTable({
    data: renderData,
    columns,
    state: { sorting },
    onSortingChange: setSorting,
    getCoreRowModel: getCoreRowModel(),
    getSortedRowModel: getSortedRowModel(),
  });

  return (
    <section className="space-y-4">
      <div className="flex flex-wrap items-center justify-between gap-3">
        <div>
          <h2 className="text-base font-semibold text-white">Packets</h2>
          <p className="text-sm text-white/45">
            Interceptor traffic.
            {filtered.length > renderData.length
              ? ` Showing first ${renderData.length} of ${filtered.length} rows.`
              : ` Showing ${filtered.length} rows.`}
          </p>
        </div>
        <label className="flex min-w-[18rem] items-center gap-2 rounded-full border border-white/10 bg-black/30 px-4 py-2 text-sm text-white/65 shadow-[inset_0_1px_0_rgba(255,255,255,0.03)]">
          <FileSearch className="size-4" />
          <input
            className="w-full bg-transparent outline-none placeholder:text-white/30"
            onChange={(event) => {
              const value = event.target.value;
              startTransition(() => setSearch(value));
            }}
            placeholder="Search domain, phase, decision, reason…"
            value={search}
          />
        </label>
      </div>

      <div className="overflow-hidden rounded-[1.6rem] border border-white/10 bg-black/25">
        <div className="max-h-[72vh] overflow-auto">
          <table className="min-w-full border-collapse">
            <thead className="sticky top-0 z-10 bg-[#0b1220]">
              {table.getHeaderGroups().map((headerGroup) => (
                <tr key={headerGroup.id}>
                  {headerGroup.headers.map((header) => (
                    <th
                      className="px-4 py-3 text-left text-[11px] uppercase tracking-[0.2em] text-white/45"
                      key={header.id}
                    >
                      {header.isPlaceholder
                        ? null
                        : flexRender(
                            header.column.columnDef.header,
                            header.getContext(),
                          )}
                    </th>
                  ))}
                </tr>
              ))}
            </thead>
            <tbody>
              {table.getRowModel().rows.map((row) => {
                const selected = row.original.id === selectedId;
                return (
                  <tr
                    className={`cursor-pointer border-t border-white/5 transition ${
                      selected
                        ? "bg-sky-500/10"
                        : "hover:bg-white/[0.035]"
                    }`}
                    key={row.id}
                    onClick={() => onSelect(row.original)}
                  >
                    {row.getVisibleCells().map((cell) => (
                      <td className="px-4 py-3 align-top" key={cell.id}>
                        {flexRender(
                          cell.column.columnDef.cell,
                          cell.getContext(),
                        )}
                      </td>
                    ))}
                  </tr>
                );
              })}
            </tbody>
          </table>
        </div>
      </div>
    </section>
  );
}
