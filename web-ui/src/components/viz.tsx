import { useState, ReactNode } from "react";
import { clsx } from "clsx";

/* ---------- 进度条 ---------- */
export function Meter({ value, total, tone = "brand" }: { value: number; total: number; tone?: "brand" | "ok" | "warn" | "danger" }) {
  const pct = total > 0 ? Math.min(100, Math.max(0, (value / total) * 100)) : 0;
  const bg = { brand: "bg-brand", ok: "bg-ok", warn: "bg-warn", danger: "bg-danger" }[tone];
  return <div className="meter"><i className={bg} style={{ width: pct + "%" }} /></div>;
}

/* ---------- 环形进度 ---------- */
export function Ring({ pct, label, tone = "ok", size = 46 }: { pct: number; label?: string; tone?: "brand" | "ok" | "warn"; size?: number }) {
  const r = (size - 6) / 2, c = 2 * Math.PI * r;
  const v = Math.max(0, Math.min(100, pct));
  const stroke = { brand: "#1070FE", ok: "#2AA364", warn: "#E4900B" }[tone];
  return (
    <div className="relative shrink-0" style={{ width: size, height: size }}>
      <svg width={size} height={size} className="-rotate-90">
        <circle cx={size / 2} cy={size / 2} r={r} fill="none" stroke="#ECEFF3" strokeWidth={4} />
        <circle cx={size / 2} cy={size / 2} r={r} fill="none" stroke={stroke} strokeWidth={4}
          strokeLinecap="round" strokeDasharray={c} strokeDashoffset={c * (1 - v / 100)} />
      </svg>
      <div className="absolute inset-0 grid place-items-center text-[11px] font-semibold leading-none">
        {Math.round(v)}%{label && <span className="text-[9px] font-normal text-ink-faint mt-0.5">{label}</span>}
      </div>
    </div>
  );
}

/* ---------- 迷你曲线 ---------- */
export function Spark({ points, color = "#1070FE", h = 34 }: { points: number[]; color?: string; h?: number }) {
  const w = 120, pad = 2, n = points.length;
  const top = Math.max(1, ...points);
  const path = n > 1 ? points.map((p, i) => {
    const x = pad + (i / (n - 1)) * (w - pad * 2);
    const y = h - pad - (p / top) * (h - pad * 2);
    return `${i === 0 ? "M" : "L"}${x.toFixed(1)},${y.toFixed(1)}`;
  }).join(" ") : "";
  return (
    <svg viewBox={`0 0 ${w} ${h}`} className="w-full" style={{ height: h }} preserveAspectRatio="none">
      {path && <path d={`${path} L${w - pad},${h - pad} L${pad},${h - pad} Z`} fill={color} opacity={0.08} />}
      {path
        ? <path d={path} fill="none" stroke={color} strokeWidth={1.5} strokeLinejoin="round" strokeLinecap="round" />
        : <text x={w / 2} y={h / 2 + 3} textAnchor="middle" fontSize="9" fill="#8A94A6">暂无数据</text>}
    </svg>
  );
}

/* ---------- 节点可用性热力图 ---------- */
export type HeatNode = { name: string; free: number; total: number; model?: string; group?: string };

const heatClass = (free: number) =>
  free <= 0 ? "bg-heat-0" : free <= 2 ? "bg-heat-1" : free <= 4 ? "bg-heat-2" : free <= 6 ? "bg-heat-3" : "bg-heat-4";

export function Heatmap({ nodes, onPick }: { nodes: HeatNode[]; onPick?: (n: HeatNode) => void }) {
  const [hover, setHover] = useState<{ n: HeatNode; x: number; y: number } | null>(null);

  // 按资源组分行，有空闲卡的组排前面
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
    }))
    .sort((a, b) => b.free - a.free || b.total - a.total);

  return (
    <div className="relative">
      <div className="space-y-1.5">
        {rows.map((row) => (
          <div key={row.name} className="flex items-center gap-3 py-0.5">
            <div className="w-40 shrink-0 text-right">
              <div className="text-[13px] font-medium truncate" title={row.name}>{row.name}</div>
              <div className="text-[11px] text-ink-faint">
                <b className={row.free ? "text-ok" : ""}>{row.free}</b> / {row.total} 张
              </div>
            </div>
            <div className="flex flex-wrap gap-1.5">
              {row.list.map((n, i) => (
                <button key={i}
                  onMouseEnter={(e) => { const r = (e.target as HTMLElement).getBoundingClientRect(); setHover({ n, x: r.left, y: r.top }); }}
                  onMouseLeave={() => setHover(null)}
                  onClick={() => onPick?.(n)}
                  className={clsx("w-5 h-5 rounded-cell transition-transform hover:scale-125 hover:ring-2 hover:ring-brand/40", heatClass(n.free))}
                  aria-label={`${n.name} 可用 ${n.free}/${n.total}`} />
              ))}
            </div>
          </div>
        ))}
        {!rows.length && <span className="text-aux text-ink-faint">暂无有 GPU 的节点</span>}
      </div>
      <div className="flex items-center gap-3 mt-3 text-[11px] text-ink-faint">
        <span>每个方块 = 一个节点，颜色深浅 = 空闲卡数</span>
        {[["0", "bg-heat-0"], ["1-2", "bg-heat-1"], ["3-4", "bg-heat-2"], ["5-6", "bg-heat-3"], ["7-8", "bg-heat-4"]].map(([l, c]) => (
          <span key={l} className="inline-flex items-center gap-1"><i className={clsx("w-3 h-3 rounded-cell inline-block", c)} />{l}</span>
        ))}
      </div>
      {hover && (
        <div className="fixed z-50 pointer-events-none" style={{ left: hover.x - 40, top: hover.y - 78 }}>
          <div className="bg-ink text-white rounded-ctl px-3 py-2 shadow-pop text-[12px] whitespace-nowrap">
            <div className="font-semibold">{hover.n.name}</div>
            <div className="text-white/80">可用 <b className="text-heat-2">{hover.n.free}</b> / {hover.n.total} 张</div>
            {hover.n.model && <div className="text-white/60 text-[11px]">{hover.n.model}</div>}
          </div>
        </div>
      )}
    </div>
  );
}

/* ---------- 右侧常驻栏容器 ---------- */
export function Rail({ children }: { children: ReactNode }) {
  return <aside className="w-[340px] shrink-0 space-y-3">{children}</aside>;
}
export function RailCard({ title, action, children }: { title: string; action?: ReactNode; children: ReactNode }) {
  return (
    <div className="card">
      <div className="flex items-center gap-2 px-4 pt-4 pb-2">
        <h3 className="text-h2">{title}</h3><div className="flex-1" />{action}
      </div>
      <div className="px-4 pb-4">{children}</div>
    </div>
  );
}
