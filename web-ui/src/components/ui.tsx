import { clsx } from "clsx";
import { createContext, useContext, useState, useCallback, ReactNode } from "react";
import { ChevronLeft, ChevronRight } from "lucide-react";

export function Card({ className, children }: { className?: string; children: ReactNode }) {
  return <div className={clsx("card", className)}>{children}</div>;
}

/** 面板：白底细线容器，可带标题行（标题 + 右侧操作）。 */
export function Panel({ title, meta, action, className, children }: { title?: ReactNode; meta?: ReactNode; action?: ReactNode; className?: string; children: ReactNode }) {
  return (
    <section className={clsx("card overflow-hidden", className)}>
      {title !== undefined && (
        <div className="panel-head">
          <h2 className="text-h2 shrink-0">{title}</h2>
          {meta && <span className="text-aux text-ink-faint truncate">{meta}</span>}
          <div className="flex-1" />
          {action}
        </div>
      )}
      {children}
    </section>
  );
}

export function PageBar({ title, hint, right }: { title: string; hint?: string; right?: ReactNode }) {
  return (
    <div className="flex items-center gap-3 h-9 mb-4">
      <h1 className="text-h1">{title}</h1>
      {hint && <span className="text-aux text-ink-faint">{hint}</span>}
      <div className="flex-1" />
      {right}
    </div>
  );
}

/** 资源摘要条里的一格：标签 + 大号数字 + 一行补充说明。 */
export function Stat({ label, children, sub }: { label: string; children: ReactNode; sub?: ReactNode }) {
  return (
    <div className="px-5 py-3 min-w-0">
      <div className="text-aux text-ink-soft">{label}</div>
      <div className="text-[20px] leading-7 font-semibold tabular-nums">{children}</div>
      {sub && <div className="text-[11px] text-ink-faint truncate">{sub}</div>}
    </div>
  );
}

/** 生成页码列表：始终显示首尾页，当前页附近各展示 1 页，中间用 0 代表省略号。 */
function pageNums(page: number, pages: number): number[] {
  const set = new Set<number>();
  set.add(1); set.add(pages);
  for (let d = -1; d <= 1; d++) {
    const p = page + d;
    if (p >= 1 && p <= pages) set.add(p);
  }
  const sorted = [...set].sort((a, b) => a - b);
  const out: number[] = [];
  for (let i = 0; i < sorted.length; i++) {
    if (i > 0 && sorted[i] - sorted[i - 1] > 1) out.push(0);
    out.push(sorted[i]);
  }
  return out;
}

/** 列表底部的分页条：总数、页码按钮、上一页 / 下一页。 */
export function Pager({ page, size, total, count, busy, unit = "个", note, onPage }: {
  page: number; size: number; total: number; count: number; busy?: boolean; unit?: string; note?: ReactNode; onPage: (n: number) => void;
}) {
  const pages = Math.max(1, Math.ceil(total / size));
  const from = (page - 1) * size + (count ? 1 : 0);
  const nums = pageNums(page, pages);
  return (
    <div className="flex items-center gap-2 mt-2 text-[12px] text-ink-faint" data-testid="pager">
      <span>共 {total} {unit}，第 {from}–{(page - 1) * size + count} {unit}。</span>
      {note}
      <span className="flex-1" />
      <button className="btn btn-mini" aria-label="上一页" disabled={page <= 1 || busy} onClick={() => onPage(page - 1)}><ChevronLeft size={13} /></button>
      {nums.map((n, i) => n === 0
        ? <span key={`e${i}`} className="px-0.5">…</span>
        : <button key={n} className={"btn btn-mini tabular-nums min-w-[26px]" + (n === page ? " bg-brand text-white border-brand" : "")}
            aria-current={n === page ? "page" : undefined} disabled={busy} onClick={() => onPage(n)}>{n}</button>
      )}
      <button className="btn btn-mini" aria-label="下一页" disabled={page >= pages || busy} onClick={() => onPage(page + 1)}><ChevronRight size={13} /></button>
    </div>
  );
}

export function Tile({ label, value, unit }: { label: string; value: ReactNode; unit?: string; tone?: string }) {
  return (
    <div className="card p-4">
      <div className="text-aux text-ink-soft">{label}</div>
      <div className="mt-1 text-big">{value}{unit && <span className="text-body font-normal text-ink-faint ml-1">{unit}</span>}</div>
    </div>
  );
}

const TONES: Record<string, string> = {
  green: "tag-ok", red: "tag-danger", amber: "tag-warn", brand: "tag-brand", gray: "tag-gray",
};
export function Badge({ text }: { text: any }) {
  const s = String(text ?? "").toUpperCase();
  let tone = "gray";
  if (/RUN/.test(s)) tone = "green";
  else if (/FAIL|ERROR|满|失败/.test(s)) tone = "red";
  else if (/WAIT|QUEU|PEND|CREAT|START|INIT|等待/.test(s)) tone = "amber";
  else if (/SUCC|成功|空闲|可用/.test(s)) tone = "brand";
  return <span className={clsx("tag", TONES[tone])}>{String(text ?? "—")}</span>;
}

/** 状态圆点：列表行首用，比标签更轻。 */
export function StatusDot({ status }: { status: any }) {
  const s = String(status ?? "").toUpperCase();
  const c = /RUN/.test(s) ? "bg-ok" : /FAIL|ERROR/.test(s) ? "bg-danger" : /WAIT|QUEU|PEND|CREAT|START|INIT/.test(s) ? "bg-warn" : "bg-ink-faint/50";
  return <span className={clsx("inline-block w-2 h-2 rounded-full shrink-0", c)} aria-hidden />;
}

export function Spinner({ label = "加载中…" }: { label?: string }) {
  return (
    <div className="flex items-center justify-center gap-2.5 py-12 text-ink-faint text-[13px]">
      <span className="w-3.5 h-3.5 rounded-full border-2 border-line border-t-brand animate-spin" />{label}
    </div>
  );
}

export function Table({ head, children }: { head: ReactNode; children: ReactNode; cols?: number }) {
  return (
    <div className="card overflow-hidden">
      <div className="overflow-x-auto">
        <table className="w-full text-[13px] border-collapse">
          <thead><tr className="text-left text-[12px] text-ink-faint border-b border-line bg-canvas/60">{head}</tr></thead>
          <tbody>{children}</tbody>
        </table>
      </div>
    </div>
  );
}
export const Th = ({ children, num }: { children?: ReactNode; num?: boolean }) =>
  <th className={clsx("font-medium px-4 h-9 whitespace-nowrap", num && "text-right")}>{children}</th>;
export const Td = ({ children, num, clip, className }: { children?: ReactNode; num?: boolean; clip?: boolean; className?: string }) =>
  <td className={clsx("px-4 py-2 border-b border-line-soft", num && "text-right tabular-nums", clip && "max-w-[220px] truncate", className)}>{children}</td>;

export function SortTh({ children, sortKey: k, active, dir, onSort, num }: { children?: ReactNode; sortKey: string; active?: boolean; dir?: "asc" | "desc"; onSort?: (k: string) => void; num?: boolean }) {
  return (
    <th className={clsx("font-medium px-4 h-9 select-none cursor-pointer hover:text-ink transition-colors whitespace-nowrap", num && "text-right")}
      onClick={() => onSort?.(k)} aria-sort={active ? (dir === "asc" ? "ascending" : "descending") : "none"}>
      <span className={clsx("inline-flex items-center gap-1", num && "flex-row-reverse")}>{children}
        <span className={clsx("text-[9px] leading-none", active ? "text-ink" : "text-line")}>{active ? (dir === "asc" ? "▲" : "▼") : "▾"}</span>
      </span>
    </th>
  );
}

export function Toolbar({ search, onSearch, placeholder, children }: { search?: string; onSearch?: (v: string) => void; placeholder?: string; children?: ReactNode }) {
  return (
    <div className="flex items-center gap-2 mb-3 flex-wrap">
      {onSearch !== undefined && (
        <div className="relative">
          <svg className="absolute left-2.5 top-1/2 -translate-y-1/2 text-ink-faint" width="13" height="13" viewBox="0 0 16 16" fill="none" aria-hidden>
            <circle cx="7" cy="7" r="5" stroke="currentColor" strokeWidth="1.6" /><path d="M11 11l3.5 3.5" stroke="currentColor" strokeWidth="1.6" strokeLinecap="round" />
          </svg>
          <input className="field pl-8 w-64" placeholder={placeholder || "搜索…"} value={search} onChange={(e) => onSearch(e.target.value)} aria-label={placeholder || "搜索"} />
        </div>
      )}
      {children}
    </div>
  );
}

/* ---- Toast ---- */
type Toast = { id: number; msg: string; err?: boolean };
const ToastCtx = createContext<(msg: string, err?: boolean) => void>(() => {});
export const useToast = () => useContext(ToastCtx);
export function ToastHost({ children }: { children: ReactNode }) {
  const [items, setItems] = useState<Toast[]>([]);
  const push = useCallback((msg: string, err?: boolean) => {
    const id = Date.now() + Math.random();
    setItems((x) => [...x, { id, msg, err }]);
    setTimeout(() => setItems((x) => x.filter((t) => t.id !== id)), err ? 6000 : 2600);
  }, []);
  return (
    <ToastCtx.Provider value={push}>
      {children}
      <div className="fixed bottom-5 right-5 flex flex-col gap-2 z-50" role="status" aria-live="polite">
        {items.map((t) => (
          <div key={t.id} className={clsx("px-3.5 py-2.5 rounded-ctl text-[13px] shadow-pop max-w-sm border",
            t.err ? "bg-white text-danger border-danger/30" : "bg-ink text-white border-ink")}>{t.msg}</div>
        ))}
      </div>
    </ToastCtx.Provider>
  );
}
