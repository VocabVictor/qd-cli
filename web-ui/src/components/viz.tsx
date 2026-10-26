import { useState, ReactNode } from "react";
import { clsx } from "clsx";

/* ---------- 进度条 ---------- */
export function Meter({ value, total, tone = "brand" }: { value: number; total: number; tone?: "brand" | "ok" | "warn" | "danger" }) {
  const pct = total > 0 ? Math.min(100, Math.max(0, (value / total) * 100)) : 0;
  const bg = { brand: "bg-brand", ok: "bg-ok", warn: "bg-warn", danger: "bg-danger" }[tone];
  return <div className="meter" role="meter" aria-valuenow={value} aria-valuemin={0} aria-valuemax={total}><i className={bg} style={{ width: pct + "%" }} /></div>;
}

/* ---------- 环形进度（保留给旧调用方） ---------- */
export function Ring({ pct, size = 40 }: { pct: number; label?: string; tone?: string; size?: number }) {
  const r = (size - 5) / 2, c = 2 * Math.PI * r;
  const v = Math.max(0, Math.min(100, pct));
  return (
    <div className="relative shrink-0" style={{ width: size, height: size }}>
      <svg width={size} height={size} className="-rotate-90">
        <circle cx={size / 2} cy={size / 2} r={r} fill="none" stroke="#E4E4E7" strokeWidth={3} />
        <circle cx={size / 2} cy={size / 2} r={r} fill="none" stroke="#0A66D8" strokeWidth={3} strokeDasharray={c} strokeDashoffset={c * (1 - v / 100)} />
      </svg>
      <div className="absolute inset-0 grid place-items-center text-[10px] font-semibold">{Math.round(v)}%</div>
    </div>
  );
}

/* ---------- 迷你曲线 ---------- */
export function Spark({ points, color = "#0A66D8", h = 24, w = 96 }: { points: number[]; color?: string; h?: number; w?: number }) {
  const pad = 1.5, n = points.length;
  // 纵轴按数据范围自适应，并保证最小量程，避免噪声被放大成剧烈波动。
  const top = Math.max((n ? Math.max(...points) : 0) * 1.2, 5);
  const path = n > 1 ? points.map((p, i) => {
    const x = pad + (i / (n - 1)) * (w - pad * 2);
    const y = h - pad - (p / top) * (h - pad * 2);
    return `${i === 0 ? "M" : "L"}${x.toFixed(1)},${y.toFixed(1)}`;
  }).join(" ") : "";
  return (
    <svg viewBox={`0 0 ${w} ${h}`} width={w} height={h} className="block" preserveAspectRatio="none" aria-hidden>
      {path
        ? <path d={path} fill="none" stroke={color} strokeWidth={1.25} strokeLinejoin="round" strokeLinecap="round" />
        : <line x1={pad} x2={w - pad} y1={h - pad} y2={h - pad} stroke="#E4E4E7" strokeWidth={1} />}
    </svg>
  );
}

/* ---------- 节点可用性热力图 ---------- */
export type HeatNode = { name: string; free: number; total: number; model?: string; group?: string; groupId?: string; spaceId?: string };

const heatClass = (free: number) =>
  free <= 0 ? "bg-heat-0" : free <= 2 ? "bg-heat-1" : free <= 4 ? "bg-heat-2" : free <= 6 ? "bg-heat-3" : "bg-heat-4";

export const HEAT_LEGEND: [string, string][] = [["0", "bg-heat-0"], ["1–2", "bg-heat-1"], ["3–4", "bg-heat-2"], ["5–6", "bg-heat-3"], ["7+", "bg-heat-4"]];

export function HeatLegend() {
  return (
    <span className="inline-flex items-center gap-2 text-[11px] text-ink-faint">
      空闲卡
      {HEAT_LEGEND.map(([l, c]) => (
        <span key={l} className="inline-flex items-center gap-1"><i className={clsx("w-2.5 h-2.5 rounded-[2px] inline-block", c)} />{l}</span>
      ))}
    </span>
  );
}

/** 每行一个资源组，每个方块一个节点，颜色深浅 = 空闲整卡数。 */
export function Heatmap({ nodes, onPick }: { nodes: HeatNode[]; onPick?: (n: HeatNode) => void }) {
  const [hover, setHover] = useState<{ n: HeatNode; x: number; y: number } | null>(null);

  const groups = new Map<string, HeatNode[]>();
  for (const n of nodes) {
    const key = n.group || "未分组";
    if (!groups.has(key)) groups.set(key, []);
    groups.get(key)!.push(n);
  }
  const rows = [...groups.entries()]
    .map(([name, list]) => ({
      name,
      list: [...list].sort((a, b) => b.free - a.free),
      free: list.reduce((a, n) => a + n.free, 0),
      total: list.reduce((a, n) => a + n.total, 0),
      model: list.find((n) => n.model)?.model,
    }))
    .sort((a, b) => b.free - a.free || b.total - a.total);

  return (
    <div className="relative">
      <div className="divide-y divide-line-soft">
        {rows.map((row) => (
          <div key={row.name} className="flex items-start gap-4 px-4 py-2.5">
            <div className="w-44 shrink-0 pt-px">
              <div className="text-[13px] font-medium truncate" title={row.name}>{row.name}</div>
              <div className="text-[11px] text-ink-faint truncate">
                <span className={row.free ? "text-ink font-medium" : ""}>{row.free}</span> / {row.total} 卡空闲{row.model ? `，${row.model}` : ""}
              </div>
            </div>
            <div className="flex flex-wrap gap-[3px] pt-0.5 min-w-0">
              {row.list.map((n, i) => (
                <button key={i}
                  onMouseEnter={(e) => { const r = (e.currentTarget as HTMLElement).getBoundingClientRect(); setHover({ n, x: r.left + r.width / 2, y: r.top }); }}
                  onMouseLeave={() => setHover(null)}
                  onFocus={(e) => { const r = e.currentTarget.getBoundingClientRect(); setHover({ n, x: r.left + r.width / 2, y: r.top }); }}
                  onBlur={() => setHover(null)}
                  onClick={() => onPick?.(n)}
                  className={clsx("w-4 h-4 rounded-cell outline-none hover:outline hover:outline-2 hover:outline-ink/70 focus-visible:outline focus-visible:outline-2 focus-visible:outline-brand", heatClass(n.free))}
                  aria-label={`${n.name}，空闲 ${n.free} / ${n.total} 卡`} />
              ))}
            </div>
          </div>
        ))}
        {!rows.length && <div className="px-4 py-6 text-aux text-ink-faint">没有带 GPU 的节点</div>}
      </div>
      {hover && (
        <div className="fixed z-50 pointer-events-none -translate-x-1/2 -translate-y-full" style={{ left: hover.x, top: hover.y - 6 }}>
          <div className="bg-ink text-white rounded-ctl px-2.5 py-1.5 text-[12px] whitespace-nowrap">
            <div className="font-medium">{hover.n.name}</div>
            <div className="text-white/75">空闲 {hover.n.free} / {hover.n.total} 卡{hover.n.model ? `，${hover.n.model}` : ""}</div>
          </div>
        </div>
      )}
    </div>
  );
}

/* ---------- 右侧栏（保留给尚未改版的页面） ---------- */
export function Rail({ children }: { children: ReactNode }) {
  return <aside className="w-[320px] shrink-0 space-y-3">{children}</aside>;
}
export function RailCard({ title, action, children }: { title: string; action?: ReactNode; children: ReactNode }) {
  return (
    <section className="card">
      <div className="panel-head"><h3 className="text-h2">{title}</h3><div className="flex-1" />{action}</div>
      <div className="p-4">{children}</div>
    </section>
  );
}
