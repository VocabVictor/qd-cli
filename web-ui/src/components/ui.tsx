import { clsx } from "clsx";
import { createContext, useContext, useState, useCallback, useEffect, ReactNode } from "react";
import { ChevronLeft, ChevronRight, CheckCircle2, AlertCircle, X, Search, Inbox } from "lucide-react";

export function Card({ className, children }: { className?: string; children: ReactNode }) {
  return <div className={clsx("card", className)}>{children}</div>;
}

/** 面板：白色卡片，可带标题行（标题 + 说明 + 右侧操作）。 */
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

export function PageBar({ title, hint, right }: { title: string; hint?: ReactNode; right?: ReactNode }) {
  return (
    <div className="flex items-end gap-3 mb-5 min-h-[36px]">
      <div className="min-w-0">
        <h1 className="text-h1">{title}</h1>
        {hint && <p className="text-aux text-ink-faint mt-0.5 truncate">{hint}</p>}
      </div>
      <div className="flex-1" />
      {right}
    </div>
  );
}

/** 资源摘要卡：标签 + 大号数字 + 一行补充说明，右上角一个浅色图标块。 */
export function Stat({ label, children, sub, icon: Icon, tone = "brand" }: { label: string; children: ReactNode; sub?: ReactNode; icon?: typeof Inbox; tone?: string }) {
  return (
    <div className="card px-5 py-4 min-w-0 flex items-start gap-3">
      <div className="min-w-0 flex-1">
        <div className="text-aux text-ink-soft">{label}</div>
        <div className="mt-1.5 text-big tabular-nums">{children}</div>
        {sub && <div className="text-[11px] text-ink-faint truncate mt-0.5">{sub}</div>}
      </div>
      {Icon && <span className={`ico ico-${tone}`}><Icon size={16} strokeWidth={1.9} /></span>}
    </div>
  );
}

const AVATAR = ["ico-brand", "ico-teal", "ico-sky", "ico-amber", "ico-rose", "ico-violet"];
/** 按名字稳定地挑一个低饱和色的头像。 */
export function Avatar({ name, size = 36 }: { name: string; size?: number }) {
  const h = [...name].reduce((a, c) => (a * 31 + c.charCodeAt(0)) >>> 0, 7);
  return <span className={clsx("ico rounded-full font-semibold uppercase", AVATAR[h % AVATAR.length])}
    style={{ width: size, height: size, fontSize: size * 0.38 }}>{name.slice(0, 1)}</span>;
}

/** 页面内的下划线页签。 */
export function Tabs<K extends string>({ items, value, onChange }: { items: [K, string, ReactNode?][]; value: K; onChange: (k: K) => void }) {
  return (
    <div className="flex items-center gap-1 border-b border-line mb-5" role="tablist">
      {items.map(([k, label, extra]) => (
        <button key={k} role="tab" aria-selected={value === k} onClick={() => onChange(k)}
          className={clsx("relative h-10 px-3 text-[13px] transition-colors flex items-center gap-1.5", value === k ? "text-brand font-medium" : "text-ink-soft hover:text-ink")}>
          {label}{extra}
          {value === k && <span className="absolute left-2 right-2 -bottom-px h-[2px] bg-brand rounded-full" />}
        </button>
      ))}
    </div>
  );
}

/** 空状态：图标 + 一句说明 + 可选操作。 */
export function Empty({ children, action }: { children: ReactNode; action?: ReactNode }) {
  return (
    <div className="flex flex-col items-center justify-center gap-2 py-12 text-center">
      <span className="grid place-items-center w-10 h-10 rounded-full bg-canvas text-ink-faint"><Inbox size={18} strokeWidth={1.6} /></span>
      <div className="text-[13px] text-ink-soft max-w-sm">{children}</div>
      {action}
    </div>
  );
}

/** 生成页码列表：始终显示首尾页，当前页附近各展示 1 页，中间用 0 代表省略号。 */
function pageNums(page: number, pages: number): number[] {
  const set = new Set<number>([1, pages]);
  for (let d = -1; d <= 1; d++) { const p = page + d; if (p >= 1 && p <= pages) set.add(p); }
  const sorted = [...set].sort((a, b) => a - b);
  const out: number[] = [];
  sorted.forEach((n, i) => { if (i > 0 && n - sorted[i - 1] > 1) out.push(0); out.push(n); });
  return out;
}

/** 列表底部的分页条：总数、页码按钮、上一页 / 下一页。 */
export function Pager({ page, size, total, count, busy, unit = "个", note, onPage }: {
  page: number; size: number; total: number; count: number; busy?: boolean; unit?: string; note?: ReactNode; onPage: (n: number) => void;
}) {
  const pages = Math.max(1, Math.ceil(total / size));
  const from = (page - 1) * size + (count ? 1 : 0);
  return (
    <div className="flex items-center gap-2 mt-3 text-[12px] text-ink-faint flex-wrap" data-testid="pager">
      <span>共 {total} {unit}{total ? `，第 ${from}–${(page - 1) * size + count} ${unit}` : ""}</span>
      {note}
      <span className="flex-1" />
      <>
        <button className="btn btn-mini btn-icon" aria-label="上一页" disabled={page <= 1 || busy} onClick={() => onPage(page - 1)}><ChevronLeft size={14} /></button>
        {pageNums(page, pages).map((n, i) => n === 0
          ? <span key={`e${i}`} className="w-5 text-center">…</span>
          : <button key={n} className={clsx("btn btn-mini tabular-nums min-w-[28px] px-1.5", n === page && "btn-pri")}
              aria-current={n === page ? "page" : undefined} disabled={busy} onClick={() => n !== page && onPage(n)}>{n}</button>)}
        <button className="btn btn-mini btn-icon" aria-label="下一页" disabled={page >= pages || busy} onClick={() => onPage(page + 1)}><ChevronRight size={14} /></button>
      </>
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

// 平台状态码 → 中文；未知的原样显示
const STATUS_CN: Record<string, string> = {
  RUNNING: "运行中", SUCCEEDED: "已完成", SUCCESS: "已完成", FAILED: "失败", ERROR: "出错", STOPPED: "已停止", STOPPING: "停止中",
  WAITING: "排队中", QUEUED: "排队中", PENDING: "等待中", CREATED: "已创建", CREATING: "创建中", STARTING: "启动中", INIT: "初始化",
  RESTARTING: "重启中", DELETED: "已删除", UNKNOWN: "未知",
};
export const statusText = (v: any) => STATUS_CN[String(v ?? "").toUpperCase()] || String(v ?? "—") || "—";

type Tone = "ok" | "danger" | "warn" | "brand" | "gray";
function toneOf(text: any): Tone {
  const s = String(text ?? "").toUpperCase();
  if (/RUN|运行/.test(s)) return "ok";
  if (/FAIL|ERROR|满|失败|出错/.test(s)) return "danger";
  if (/WAIT|QUEU|PEND|CREAT|START|INIT|STOPPING|等待|排队|启动|创建/.test(s)) return "warn";
  if (/SUCC|成功|完成|空闲|可用/.test(s)) return "brand";
  return "gray";
}
const DOT: Record<Tone, string> = { ok: "bg-ok", danger: "bg-danger", warn: "bg-warn", brand: "bg-brand", gray: "bg-ink-faint" };

export function Badge({ text }: { text: any }) {
  const tone = toneOf(text);
  return (
    <span className={clsx("tag", `tag-${tone === "ok" ? "ok" : tone}`)}>
      <i className={clsx("w-1.5 h-1.5 rounded-full", DOT[tone], tone === "ok" && "animate-pulse")} />{statusText(text)}
    </span>
  );
}

/** 状态圆点：列表行首用，比标签更轻。 */
export function StatusDot({ status }: { status: any }) {
  const tone = toneOf(status);
  return <span className={clsx("inline-block w-2 h-2 rounded-full shrink-0 ring-[3px]", DOT[tone],
    { ok: "ring-ok/15", danger: "ring-danger/15", warn: "ring-warn/15", brand: "ring-brand/15", gray: "ring-ink-faint/15" }[tone])} aria-hidden />;
}

export function Spinner({ label = "加载中…" }: { label?: string }) {
  return (
    <div className="flex items-center justify-center gap-2.5 py-14 text-ink-faint text-[13px]">
      <span className="w-4 h-4 rounded-full border-2 border-line border-t-brand animate-spin" />{label}
    </div>
  );
}

export function Table({ head, children }: { head: ReactNode; children: ReactNode; cols?: number }) {
  return (
    <div className="card overflow-hidden">
      <div className="overflow-x-auto">
        <table className="tbl">
          <thead><tr>{head}</tr></thead>
          <tbody>{children}</tbody>
        </table>
      </div>
    </div>
  );
}
export const Th = ({ children, num, className }: { children?: ReactNode; num?: boolean; className?: string }) =>
  <th className={clsx("font-medium px-4 h-10 whitespace-nowrap", num && "text-right", className)}>{children}</th>;
export const Td = ({ children, num, clip, className }: { children?: ReactNode; num?: boolean; clip?: boolean; className?: string }) =>
  <td className={clsx("px-4 py-2.5", num && "text-right tabular-nums", clip && "max-w-[220px] truncate", className)}>{children}</td>;

export function SortTh({ children, sortKey: k, active, dir, onSort, num }: { children?: ReactNode; sortKey: string; active?: boolean; dir?: "asc" | "desc"; onSort?: (k: string) => void; num?: boolean }) {
  return (
    <th className={clsx("font-medium px-4 h-10 select-none whitespace-nowrap", num && "text-right")}
      aria-sort={active ? (dir === "asc" ? "ascending" : "descending") : "none"}>
      <button className={clsx("inline-flex items-center gap-1 hover:text-ink transition-colors", active && "text-ink", num && "flex-row-reverse")} onClick={() => onSort?.(k)}>
        {children}
        <svg width="8" height="10" viewBox="0 0 8 10" aria-hidden className="shrink-0">
          <path d="M4 1L7 4H1z" fill="currentColor" opacity={active && dir === "asc" ? 1 : 0.3} />
          <path d="M4 9L1 6h6z" fill="currentColor" opacity={active && dir !== "asc" ? 1 : 0.3} />
        </svg>
      </button>
    </th>
  );
}

export function SearchBox({ value, onChange, placeholder }: { value?: string; onChange: (v: string) => void; placeholder?: string }) {
  return (
    <div className="relative">
      <Search size={14} className="absolute left-3 top-1/2 -translate-y-1/2 text-ink-faint pointer-events-none" />
      <input className="field pl-9 pr-8 w-64" placeholder={placeholder || "搜索…"} value={value} onChange={(e) => onChange(e.target.value)}
        onKeyDown={(e) => e.key === "Escape" && onChange("")} aria-label={placeholder || "搜索"} />
      {value && <button className="absolute right-2 top-1/2 -translate-y-1/2 grid place-items-center w-5 h-5 rounded text-ink-faint hover:text-ink hover:bg-hover"
        aria-label="清空搜索" onClick={() => onChange("")}><X size={12} /></button>}
    </div>
  );
}

export function Toolbar({ search, onSearch, placeholder, children }: { search?: string; onSearch?: (v: string) => void; placeholder?: string; children?: ReactNode }) {
  return (
    <div className="flex items-center gap-2 mb-3 flex-wrap">
      {onSearch !== undefined && <SearchBox value={search} onChange={onSearch} placeholder={placeholder} />}
      {children}
    </div>
  );
}

/** 状态筛选的胶囊组：标签 + 计数。 */
export function Chips<K extends string>({ items, value, onChange, counts, label }: {
  items: [K, string][]; value: K; onChange: (k: K) => void; counts?: Record<string, number>; label: string;
}) {
  return (
    <div className="flex items-center gap-1 p-[3px] rounded-full bg-[#EDF0F4]" role="tablist" aria-label={label}>
      {items.map(([k, text]) => (
        <button key={k} role="tab" aria-selected={value === k} onClick={() => onChange(k)} className={clsx("chip", value === k && "chip-on")}>
          {text}<span className="count">{counts ? counts[k] ?? 0 : "…"}</span>
        </button>
      ))}
    </div>
  );
}

/* ---- Toast ---- */
type Toast = { id: number; msg: string; err?: boolean };
const ToastCtx = createContext<(msg: string, err?: boolean) => void>(() => {});
export const useToast = () => useContext(ToastCtx);
/** 让所有提示立即消失（切换登录状态时用，避免上一个页面的报错留到下一个页面）。 */
export const clearToasts = () => window.dispatchEvent(new Event("qd-toast-clear"));

export function ToastHost({ children }: { children: ReactNode }) {
  const [items, setItems] = useState<Toast[]>([]);
  const drop = useCallback((id: number) => setItems((x) => x.filter((t) => t.id !== id)), []);
  const push = useCallback((msg: string, err?: boolean) => {
    const id = Date.now() + Math.random();
    // 同一句话不重复堆叠；最多同时显示 3 条
    setItems((x) => [...x.filter((t) => t.msg !== msg), { id, msg, err }].slice(-3));
    setTimeout(() => drop(id), err ? 6000 : 2800);
  }, [drop]);
  useEffect(() => {
    const clear = () => setItems([]);
    window.addEventListener("qd-toast-clear", clear);
    return () => window.removeEventListener("qd-toast-clear", clear);
  }, []);
  return (
    <ToastCtx.Provider value={push}>
      {children}
      <div className="fixed bottom-5 right-5 flex flex-col items-end gap-2 z-[60] pointer-events-none" role="status" aria-live="polite">
        {items.map((t) => (
          <div key={t.id} className="pointer-events-auto animate-rise flex items-start gap-2.5 pl-3.5 pr-2 py-2.5 rounded-[10px] text-[13px] bg-white shadow-pop max-w-sm">
            {t.err ? <AlertCircle size={16} className="text-danger shrink-0 mt-px" /> : <CheckCircle2 size={16} className="text-ok shrink-0 mt-px" />}
            <span className="min-w-0 break-words">{t.msg}</span>
            <button className="shrink-0 grid place-items-center w-5 h-5 rounded text-ink-faint hover:text-ink hover:bg-hover" aria-label="关闭提示" onClick={() => drop(t.id)}><X size={12} /></button>
          </div>
        ))}
      </div>
    </ToastCtx.Provider>
  );
}
