import { clsx } from "clsx";
import { createContext, useContext, useState, useCallback, ReactNode } from "react";

export function Card({ className, children }: { className?: string; children: ReactNode }) {
  return <div className={clsx("card", className)}>{children}</div>;
}

export function PageBar({ title, hint, right }: { title: string; hint?: string; right?: ReactNode }) {
  return (
    <div className="flex items-center gap-3 mb-5">
      <h1 className="text-xl font-semibold tracking-tight">{title}</h1>
      {hint && <span className="text-sm text-ink-faint">{hint}</span>}
      <div className="flex-1" />
      {right}
    </div>
  );
}

export function Tile({ label, value, unit, tone = "brand" }: { label: string; value: ReactNode; unit?: string; tone?: "brand" | "mint" | "rose" | "amber" }) {
  const wash = {
    brand: "from-brand-50 to-white", mint: "from-mint/10 to-white",
    rose: "from-rose/10 to-white", amber: "from-amber/10 to-white",
  }[tone];
  const dot = { brand: "bg-brand", mint: "bg-mint", rose: "bg-rose", amber: "bg-amber" }[tone];
  return (
    <div className={clsx("rounded-xl2 border border-line p-4 bg-gradient-to-br", wash)}>
      <div className="flex items-center gap-2 text-xs font-medium text-ink-soft">
        <span className={clsx("w-1.5 h-1.5 rounded-full", dot)} />{label}
      </div>
      <div className="mt-2 text-[28px] leading-none font-semibold tracking-tight">
        {value}{unit && <span className="text-sm font-normal text-ink-faint ml-1">{unit}</span>}
      </div>
    </div>
  );
}

const TONES: Record<string, string> = {
  green: "bg-mint/10 text-mint", red: "bg-rose/10 text-rose", amber: "bg-amber/10 text-amber",
  brand: "bg-brand-50 text-brand-600", gray: "bg-canvas text-ink-soft",
};
export function Badge({ text }: { text: any }) {
  const s = String(text ?? "").toUpperCase();
  let tone = "gray";
  if (/RUN/.test(s)) tone = "green";
  else if (/FAIL|ERROR|满|失败/.test(s)) tone = "red";
  else if (/WAIT|QUEU|PEND|CREAT|START|INIT|等待/.test(s)) tone = "amber";
  else if (/SUCC|成功|空闲|可用/.test(s)) tone = "brand";
  return <span className={clsx("tag", TONES[tone])}>{String(text ?? "-")}</span>;
}

export function Spinner({ label = "加载中…" }: { label?: string }) {
  return (
    <div className="flex items-center justify-center gap-3 py-16 text-ink-faint text-sm">
      <span className="w-4 h-4 rounded-full border-2 border-brand-100 border-t-brand animate-spin" />{label}
    </div>
  );
}

export function Table({ head, children, cols }: { head: ReactNode; children: ReactNode; cols?: number }) {
  return (
    <Card className="overflow-hidden">
      <div className="overflow-x-auto">
        <table className="w-full text-sm border-collapse">
          <thead><tr className="text-left text-[11px] uppercase tracking-wide text-ink-faint border-b border-line bg-canvas/40">{head}</tr></thead>
          <tbody>{children}</tbody>
        </table>
      </div>
    </Card>
  );
}
export const Th = ({ children, num }: { children?: ReactNode; num?: boolean }) =>
  <th className={clsx("font-semibold px-4 py-3", num && "text-right")}>{children}</th>;
export const Td = ({ children, num, clip, className }: { children?: ReactNode; num?: boolean; clip?: boolean; className?: string }) =>
  <td className={clsx("px-4 py-3 border-b border-line/60", num && "text-right tabular-nums", clip && "max-w-[220px] truncate", className)}>{children}</td>;


export function SortTh({ children, sortKey: k, active, dir, onSort, num }: { children?: ReactNode; sortKey: string; active?: boolean; dir?: "asc" | "desc"; onSort?: (k: string) => void; num?: boolean }) {
  return (
    <th className={clsx("font-semibold px-4 py-3 select-none cursor-pointer hover:text-ink transition-colors", num && "text-right")} onClick={() => onSort?.(k)}>
      <span className={clsx("inline-flex items-center gap-1", num && "flex-row-reverse")}>{children}
        <span className={clsx("text-[9px] leading-none", active ? "text-brand" : "text-line")}>{active ? (dir === "asc" ? "▲" : "▼") : "▾"}</span>
      </span>
    </th>
  );
}

export function Toolbar({ search, onSearch, placeholder, children }: { search?: string; onSearch?: (v: string) => void; placeholder?: string; children?: ReactNode }) {
  return (
    <div className="flex items-center gap-2 mb-3 flex-wrap">
      {onSearch !== undefined && (
        <div className="relative">
          <span className="absolute left-3 top-1/2 -translate-y-1/2 text-ink-faint">⌕</span>
          <input className="field pl-8 w-64" placeholder={placeholder || "搜索…"} value={search} onChange={(e) => onSearch(e.target.value)} />
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
      <div className="fixed bottom-5 right-5 flex flex-col gap-2 z-50">
        {items.map((t) => (
          <div key={t.id} className={clsx("px-4 py-2.5 rounded-xl text-sm shadow-pop text-white max-w-sm",
            t.err ? "bg-rose" : "bg-ink")}>{t.msg}</div>
        ))}
      </div>
    </ToastCtx.Provider>
  );
}
