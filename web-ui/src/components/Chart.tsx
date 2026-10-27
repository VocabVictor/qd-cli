// 轻量内联 SVG 折线图：0–max 固定量程，三条细网格线与刻度，便于不同指标横向比较。
export function LineChart({ points, unit = "%", color = "#4F46E5", max = 100, minutes = 10 }: { points: number[]; unit?: string; color?: string; max?: number; minutes?: number }) {
  const w = 320, h = 96, left = 26, right = 4, top = 6, bottom = 16;
  const n = points.length;
  const ceil = Math.max(max, ...points, 1);
  const x = (i: number) => left + (n > 1 ? (i / (n - 1)) * (w - left - right) : 0);
  const y = (v: number) => top + (1 - v / ceil) * (h - top - bottom);
  const path = n > 1 ? points.map((p, i) => `${i ? "L" : "M"}${x(i).toFixed(1)},${y(p).toFixed(1)}`).join(" ") : "";
  const grid = [0, 0.5, 1].map((f) => ceil * f);
  return (
    <svg viewBox={`0 0 ${w} ${h}`} className="w-full h-24" preserveAspectRatio="none" role="img" aria-label={`近 ${minutes} 分钟曲线`}>
      {grid.map((g) => (
        <g key={g}>
          <line x1={left} x2={w - right} y1={y(g)} y2={y(g)} stroke="#EEF1F5" strokeWidth={1} vectorEffect="non-scaling-stroke" />
          <text x={left - 4} y={y(g) + 3} textAnchor="end" fontSize="8" fill="#94A3B8">{Math.round(g)}{unit === "%" ? "" : ""}</text>
        </g>
      ))}
      <text x={left} y={h - 3} fontSize="8" fill="#94A3B8">-{minutes} 分钟</text>
      <text x={w - right} y={h - 3} textAnchor="end" fontSize="8" fill="#94A3B8">现在</text>
      {path && <path d={`${path} L${x(n - 1).toFixed(1)},${y(0).toFixed(1)} L${x(0).toFixed(1)},${y(0).toFixed(1)} Z`} fill={color} opacity={0.07} />}
      {path && <path d={path} fill="none" stroke={color} strokeWidth={1.5} strokeLinejoin="round" strokeLinecap="round" vectorEffect="non-scaling-stroke" />}
      {!n && <text x={(w + left) / 2} y={h / 2} textAnchor="middle" fontSize="10" fill="#94A3B8">暂无数据</text>}
    </svg>
  );
}
