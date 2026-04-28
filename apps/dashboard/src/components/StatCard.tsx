type StatCardProps = {
  label: string;
  value: number | string;
  tone: "amber" | "blue" | "emerald" | "rose";
};

const toneMap = {
  amber: "text-amber-300 ring-amber-500/20",
  blue: "text-sky-300 ring-sky-500/20",
  emerald: "text-emerald-300 ring-emerald-500/20",
  rose: "text-rose-300 ring-rose-500/20",
};

export function StatCard({ label, value, tone }: StatCardProps) {
  return (
    <div
      className={`rounded-xl border border-white/10 bg-white/5 p-3 shadow-[inset_0_1px_0_rgba(255,255,255,0.04)] ring-1 ${toneMap[tone]}`}
    >
      <div className="text-[10px] uppercase tracking-[0.18em] text-white/45">
        {label}
      </div>
      <div className="mt-1 text-2xl font-semibold tracking-tight text-white">
        {value}
      </div>
    </div>
  );
}
