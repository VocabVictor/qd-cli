import { useEffect, useState, useRef } from "react";
import { ExternalLink, Loader2, RotateCw } from "lucide-react";
import { api, qget, canManageJob } from "../lib/api";
import { Drawer, DrawerTab } from "./Drawer";
import { LineChart } from "./Chart";

const COLORS: Record<string, string> = { gpu_util: "#4F46E5", gpu_memory: "#7C3AED", cpu: "#0F766E", memory: "#0369A1" };
const fmt = (v: number | null | undefined, unit: string) => (v == null ? "—" : `${v.toFixed(0)}${unit}`);

type Tab = "metrics" | "logs" | "terminal";
export type MonitorJob = { id: string; name?: string; space?: string; kind?: "job" | "dev"; gpu?: number; owner?: string | null };

/** 作业详情抽屉：性能监控 / 运行日志 / 网页终端三个页签。 */
export function Monitor({ job, onClose, initialTab = "metrics" }: { job: MonitorJob | null; onClose: () => void; baseUrl?: string; initialTab?: Tab }) {
  const [tab, setTab] = useState<Tab>(initialTab);
  const [metrics, setMetrics] = useState<any[] | null>(null);
  const [metricErr, setMetricErr] = useState("");
  const [logs, setLogs] = useState<string[] | null>(null);
  const [logErr, setLogErr] = useState("");
  const [termUrl, setTermUrl] = useState("");
  const [termErr, setTermErr] = useState("");
  const [termKey, setTermKey] = useState(0);
  const timer = useRef<any>(null);
  const logBox = useRef<HTMLPreElement>(null);

  const kind = job?.kind || "job";
  const canTerminal = !!job && (kind === "dev" || canManageJob({ qdOwner: job.owner }));

  useEffect(() => {
    if (!job) return;
    setTab(initialTab); setMetrics(null); setMetricErr(""); setLogs(null); setLogErr(""); setTermUrl(""); setTermErr("");
    const load = () => api.jobMetrics(job.id, 10, job.space || "")
      .then((d) => { setMetrics(d.metrics || []); setMetricErr(""); })
      .catch((e) => setMetricErr(e.message));
    load();
    timer.current = setInterval(load, 5000);
    return () => clearInterval(timer.current);
  }, [job]); // eslint-disable-line

  const loadLogs = () => {
    if (!job) return;
    setLogErr("");
    api.jobLogs(job.id, 500, job.space || "")
      .then((d) => { setLogs(d.lines || []); requestAnimationFrame(() => logBox.current?.scrollTo(0, 1e9)); })
      .catch((e) => setLogErr(e.message));
  };
  const loadTerminal = () => {
    if (!job || !canTerminal) return;
    setTermErr(""); setTermUrl("");
    qget(`/api/terminal-url?kind=${kind}&id=${encodeURIComponent(job.id)}&instance=0${job.space ? "&space=" + job.space : ""}`)
      .then((r) => { setTermUrl(r.terminalUrl); setTermKey((k) => k + 1); })
      .catch((e) => setTermErr(e.message));
  };
  useEffect(() => {
    if (!job) return;
    if (tab === "logs" && logs === null) loadLogs();
    if (tab === "terminal" && !termUrl && !termErr) loadTerminal();
  }, [tab, job]); // eslint-disable-line

  if (!job) return null;
  const shown = (metrics || []).filter((m: any) => (job.gpu ?? 1) > 0 || !m.metric.startsWith("gpu"));

  return (
    <Drawer open onClose={onClose} title={job.name || job.id} sub={`#${job.id}${job.gpu != null ? `，${job.gpu} 卡` : ""}`} width="max-w-4xl" fill={tab !== "metrics"}
      tabs={<>
        <DrawerTab active={tab === "metrics"} onClick={() => setTab("metrics")}>性能监控</DrawerTab>
        {kind === "job" && <DrawerTab active={tab === "logs"} onClick={() => setTab("logs")}>运行日志</DrawerTab>}
        <DrawerTab active={tab === "terminal"} onClick={() => setTab("terminal")} disabled={!canTerminal}
          title={canTerminal ? undefined : "只能进入自己提交的作业的终端"}>网页终端</DrawerTab>
        <div className="flex-1" />
        {tab === "metrics" && <span className="text-[11px] text-ink-faint">近 10 分钟，每 5 秒刷新</span>}
        {tab === "logs" && <button className="btn btn-ghost btn-mini" onClick={loadLogs}><RotateCw size={12} />刷新日志</button>}
        {tab === "terminal" && termUrl && <>
          <button className="btn btn-ghost btn-mini" onClick={loadTerminal}><RotateCw size={12} />重新连接</button>
          <button className="btn btn-ghost btn-mini" onClick={() => window.open(termUrl, "_blank")}><ExternalLink size={12} />新标签打开</button>
        </>}
      </>}>
      {tab === "metrics" && (
        metricErr && !metrics ? <p className="p-5 text-body text-danger">监控读取失败：{metricErr}</p> :
        !metrics ? <div className="flex items-center gap-2 text-ink-faint text-[13px] p-5"><Loader2 size={14} className="animate-spin" />读取监控…</div> :
        <div className="grid sm:grid-cols-2 sm:divide-x divide-line">
          {shown.map((m: any) => (
            <section key={m.metric} className="px-6 py-5 border-b border-line-soft" data-metric={m.metric}>
              <div className="flex items-baseline gap-3">
                <h3 className="text-h2 flex items-center gap-2"><i className="w-2 h-2 rounded-full" style={{ background: COLORS[m.metric] }} />{m.label}</h3>
                <span className="flex-1" />
                <span className="text-[11px] text-ink-faint tabular-nums">均值 {fmt(m.avg, m.unit)}，峰值 {fmt(m.max, m.unit)}</span>
                <span className="text-[20px] font-semibold tabular-nums w-16 text-right" style={{ color: COLORS[m.metric] }}>{fmt(m.last, m.unit)}</span>
              </div>
              <LineChart points={m.points || []} color={COLORS[m.metric]} unit={m.unit} />
            </section>
          ))}
          {!shown.length && <p className="p-5 text-aux text-ink-faint">平台没有返回监控数据，作业可能还没开始运行。</p>}
        </div>
      )}

      {tab === "logs" && (
        logErr ? <p className="p-5 text-body text-danger">日志读取失败：{logErr}</p> :
        logs === null ? <div className="flex items-center gap-2 text-ink-faint text-[13px] p-5"><Loader2 size={14} className="animate-spin" />读取日志…</div> :
        <pre ref={logBox} data-testid="job-logs" className="m-0 flex-1 overflow-auto px-6 py-4 font-mono text-[12px] leading-[19px] text-[#CBD5E1] whitespace-pre-wrap break-all bg-[#0B1020]">
          {logs.length ? logs.join("\n") : <span className="text-[#64748B]">还没有日志输出。</span>}
        </pre>
      )}

      {tab === "terminal" && (
        termErr ? <div className="p-5 text-body"><p className="text-danger mb-3">终端不可用：{termErr}</p><button className="btn btn-mini" onClick={loadTerminal}><RotateCw size={12} />重试</button></div> :
        !termUrl ? <div className="flex items-center gap-2 text-ink-faint text-[13px] p-5"><Loader2 size={14} className="animate-spin" />连接终端…</div> :
        <iframe key={termKey} src={termUrl} title="网页终端" className="flex-1 w-full bg-[#0B1020]" style={{ border: 0 }} />
      )}
    </Drawer>
  );
}
