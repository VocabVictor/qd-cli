import { ReactNode, useEffect } from "react";
import { X } from "lucide-react";

export function Drawer({ open, onClose, title, sub, children }: { open: boolean; onClose: () => void; title?: ReactNode; sub?: ReactNode; children?: ReactNode }) {
  useEffect(() => {
    const h = (e: KeyboardEvent) => e.key === "Escape" && onClose();
    if (open) window.addEventListener("keydown", h);
    return () => window.removeEventListener("keydown", h);
  }, [open, onClose]);
  if (!open) return null;
  return (
    <div className="fixed inset-0 z-40">
      <div className="absolute inset-0 bg-ink/20" onClick={onClose} />
      <div className="absolute top-0 right-0 h-full w-full max-w-3xl bg-canvas border-l border-line shadow-pop overflow-y-auto">
        <div className="sticky top-0 bg-canvas/90 backdrop-blur border-b border-line px-6 py-4 flex items-center gap-3">
          <div className="min-w-0">
            <div className="font-semibold truncate">{title}</div>
            {sub && <div className="text-xs text-ink-faint truncate mt-0.5">{sub}</div>}
          </div>
          <div className="flex-1" />
          <button className="btn btn-mini" onClick={onClose}><X size={14} />关闭</button>
        </div>
        <div className="p-6">{children}</div>
      </div>
    </div>
  );
}

export function Json({ value }: { value: any }) {
  return <pre className="card p-4 text-xs overflow-x-auto leading-relaxed text-ink-soft">{JSON.stringify(value, null, 2)}</pre>;
}
