import { useEffect, useState, useRef } from "react";
import { ExternalLink, Loader2, Activity } from "lucide-react";
import { api, qget } from "../lib/api";
import { Drawer } from "./Drawer";
import { Card, useToast } from "./ui";
import { LineChart } from "./Chart";

const COLORS: Record<string, string> = { gpu_util: "#7c5cfc", gpu_memory: "#b14eff", cpu: "#22c9a8", memory: "#f6a609" };

export function Monitor({ job, onClose, baseUrl }: { job: { id: string; name?: string; space?: string; kind?: "job" | "dev" } | null; onClose: () => void; baseUrl?: string }) {
  const toast = useToast();
  const [metrics, setMetrics] = useState<any[] | null>(null);
  const [termUrl, setTermUrl] = useState<string>("");
  const [termErr, setTermErr] = useState<string>("");
  const [tab, setTab] = useState<"monitor" | "terminal">("monitor");
  const timer = useRef<any>(null);

  useEffect(() => {
    if (!job) return;
    setMetrics(null); setTermUrl(""); setTermErr(""); setTab("monitor");
    const load = async () => {
      try { const d = await api.jobMetrics(job.id, 10, job.space || ""); setMetrics(d.metrics || []); } catch { /* 保留上次 */ }
    };
    load();
    timer.current = setInterval(load, 5000);
    // 预取终端地址
    qget(`/api/terminal-url?kind=${job.kind || "job"}&id=${encodeURIComponent(job.id)}&instance=0${job.space ? "&space=" + job.space : ""}`)
      .then((r) => setTermUrl(r.terminalUrl)).catch((e) => setTermErr(e.message));
    return () => clearInterval(timer.current);
  }, [job]);

  if (!job) return null;
  const openTerm = () => { if (termUrl) window.open(termUrl, "_blank"); else toast(termErr || "终端地址获取中", true); };

  return (
    <Drawer open={!!job} onClose={onClose} title={<span className="flex items-center gap-2"><Activity size={16} className="text-brand" />{job.name || job.id}</span>} sub={job.id}>
      <div className="flex items-center gap-2 mb-4">
        <button className={"chip " + (tab === "monitor" ? "chip-on" : "")} onClick={() => setTab("monitor")}>实时监控</button>
        <button className={"chip " + (tab === "terminal" ? "chip-on" : "")} onClick={() => setTab("terminal")}>网页终端</button>
        <div className="flex-1" />
        <button className="btn btn-mini" onClick={openTerm}><ExternalLink size={12} />新标签打开终端</button>
      </div>

      {tab === "monitor" ? (
        !metrics ? <div className="flex items-center gap-2 text-ink-faint text-sm py-10 justify-center"><Loader2 size={16} className="animate-spin" />读取监控…</div> : (
          <div className="grid grid-cols-1 sm:grid-cols-2 gap-3">
            {metrics.map((m) => (
              <Card key={m.metric} className="p-4">
                <div className="flex items-baseline justify-between mb-1">
                  <span className="text-sm font-medium text-ink-soft">{m.label}</span>
                  <span className="text-2xl font-semibold tracking-tight">{m.last != null ? m.last.toFixed(0) : "-"}<span className="text-xs text-ink-faint ml-0.5">{m.unit}</span></span>
                </div>
                <LineChart points={m.points || []} color={COLORS[m.metric]} unit={m.unit} />
                <div className="flex gap-4 text-[11px] text-ink-faint mt-1">
                  <span>均值 {m.avg != null ? m.avg.toFixed(0) : "-"}{m.unit}</span>
                  <span>峰值 {m.max != null ? m.max.toFixed(0) : "-"}{m.unit}</span>
                </div>
              </Card>
            ))}
            <p className="col-span-full text-xs text-ink-faint">近 10 分钟，每 5 秒刷新。数据来自平台作业监控（GPU 利用率 / 显存 / CPU / 内存）。</p>
          </div>
        )
      ) : (
        <div>
          {termErr ? <Card className="p-4 text-sm">终端不可用：{termErr}</Card> :
            !termUrl ? <div className="flex items-center gap-2 text-ink-faint text-sm py-10 justify-center"><Loader2 size={16} className="animate-spin" />获取终端地址…</div> : (
              <>
                <div className="rounded-card overflow-hidden border border-line bg-black" style={{ height: 460 }}>
                  <iframe src={termUrl} title="terminal" className="w-full h-full" style={{ border: 0 }} />
                </div>
                <p className="text-xs text-ink-faint mt-2">若终端因平台安全策略无法内嵌显示，请点右上「新标签打开终端」。</p>
              </>
            )}
        </div>
      )}
    </Drawer>
  );
}
