import { ReactNode, useEffect } from "react";
import { X } from "lucide-react";
import { clsx } from "clsx";

/** 右侧抽屉：标题行 + 可选工具栏 + 内容 + 可选底栏。fill=true 时内容区不滚动、撑满剩余高度（终端用）。 */
export function Drawer({ open, onClose, title, sub, actions, tabs, footer, width = "max-w-3xl", fill, children }: {
  open: boolean; onClose: () => void; title?: ReactNode; sub?: ReactNode; actions?: ReactNode; tabs?: ReactNode;
  footer?: ReactNode; width?: string; fill?: boolean; children?: ReactNode;
}) {
  useEffect(() => {
    const h = (e: KeyboardEvent) => e.key === "Escape" && onClose();
    if (open) window.addEventListener("keydown", h);
    return () => window.removeEventListener("keydown", h);
  }, [open, onClose]);
  if (!open) return null;
  return (
    <div className="fixed inset-0 z-40" role="dialog" aria-modal="true">
      <div className="absolute inset-0 bg-black/20" onClick={onClose} />
      <div className={clsx("absolute top-0 right-0 h-full w-full bg-white border-l border-line shadow-pop flex flex-col", width)}>
        <div className="flex items-center gap-3 h-12 px-5 border-b border-line shrink-0">
          <div className="min-w-0 flex items-baseline gap-2">
            <div className="text-[14px] font-semibold truncate">{title}</div>
            {sub && <div className="text-aux text-ink-faint truncate">{sub}</div>}
          </div>
          <div className="flex-1" />
          {actions}
          <button className="btn btn-ghost btn-mini px-1.5" onClick={onClose} aria-label="关闭"><X size={15} /></button>
        </div>
        {tabs && <div className="flex items-center gap-1 h-10 px-4 border-b border-line shrink-0">{tabs}</div>}
        <div className={clsx("flex-1 min-h-0", fill ? "flex flex-col" : "overflow-y-auto")}>{children}</div>
        {footer && <div className="shrink-0 border-t border-line px-5 py-3 bg-canvas">{footer}</div>}
      </div>
    </div>
  );
}

/** 抽屉内的下划线页签。 */
export function DrawerTab({ active, onClick, children, disabled, title }: { active: boolean; onClick: () => void; children: ReactNode; disabled?: boolean; title?: string }) {
  return (
    <button role="tab" aria-selected={active} disabled={disabled} title={title} onClick={onClick}
      className={clsx("relative h-10 px-2.5 text-[13px] transition-colors disabled:opacity-40 disabled:cursor-not-allowed",
        active ? "text-ink font-medium" : "text-ink-soft hover:text-ink")}>
      {children}
      {active && <span className="absolute left-2 right-2 -bottom-px h-[2px] bg-ink rounded-full" />}
    </button>
  );
}

export function Json({ value }: { value: any }) {
  return <pre className="card p-4 text-xs overflow-x-auto leading-relaxed text-ink-soft font-mono">{JSON.stringify(value, null, 2)}</pre>;
}
