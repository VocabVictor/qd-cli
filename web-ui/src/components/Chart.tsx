// 轻量内联 SVG 折线图，无第三方库。
export function LineChart({ points, unit = "%", color = "#7c5cfc", max = 100 }: { points: number[]; unit?: string; color?: string; max?: number }) {
  const w = 260, h = 60, pad = 4;
  const n = points.length;
  const top = Math.max(max, ...points, 1);
  const path = n > 1 ? points.map((p, i) => {
    const x = pad + (i / (n - 1)) * (w - pad * 2);
    const y = h - pad - (p / top) * (h - pad * 2);
    return `${i === 0 ? "M" : "L"}${x.toFixed(1)},${y.toFixed(1)}`;
  }).join(" ") : "";
  const area = path ? `${path} L${w - pad},${h - pad} L${pad},${h - pad} Z` : "";
  return (
    <svg viewBox={`0 0 ${w} ${h}`} className="w-full h-14" preserveAspectRatio="none">
      {area && <path d={area} fill={color} opacity={0.08} />}
      {path && <path d={path} fill="none" stroke={color} strokeWidth={1.6} strokeLinejoin="round" strokeLinecap="round" />}
      {!n && <text x={w / 2} y={h / 2} textAnchor="middle" className="fill-current text-ink-faint" fontSize="9">暂无数据</text>}
    </svg>
  );
}
