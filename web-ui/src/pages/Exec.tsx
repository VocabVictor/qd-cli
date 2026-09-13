import { useState } from "react";
import { Play, ExternalLink } from "lucide-react";
import { api, AppState, qget, spacesOf } from "../lib/api";
import { PageBar, Card, useToast } from "../components/ui";

export function Exec({ state }: { state: AppState }) {
  const toast = useToast();
  const sps = spacesOf(state);
  const [kind, setKind] = useState("dev");
  const [space, setSpace] = useState(state.spaceId || sps[0]?.id || "");
  const [id, setId] = useState("");
  const [inst, setInst] = useState("0");
  const [cmd, setCmd] = useState("nvidia-smi");
  const [out, setOut] = useState("（输出会显示在这里）");
  const [busy, setBusy] = useState(false);

  const run = async () => {
    if (!id || !cmd) return toast("请填写 ID 和命令", true);
    setBusy(true); setOut("执行中…");
    try {
      const r = await api.exec({ kind, id, space, instance: Number(inst || 0), command: ["bash", "-lc", cmd] });
      setOut((r.output || "(无输出)") + "\n\n[exit " + r.exitCode + "]");
    } catch (e: any) { setOut("失败：" + e.message); }
    finally { setBusy(false); }
  };
  const term = async () => {
    if (!id) return toast("请填写 ID", true);
    try { const r = await qget(`/api/terminal-url?kind=${kind}&id=${encodeURIComponent(id)}&instance=${inst || 0}${space ? "&space=" + space : ""}`); window.open(r.terminalUrl, "_blank"); }
    catch (e: any) { toast(e.message, true); }
  };

  return (
    <>
      <PageBar title="远程执行" />
      <Card className="p-4 mb-4">
        <div className="flex items-center gap-2 flex-wrap">
          <select className="field w-32" value={kind} onChange={(e) => setKind(e.target.value)}><option value="dev">开发环境</option><option value="job">作业实例</option></select>
          <select className="field w-32" value={space} onChange={(e) => setSpace(e.target.value)}>{sps.map((s) => <option key={s.id} value={s.id}>{s.name}</option>)}</select>
          <input className="field w-44" placeholder="环境/作业 ID" value={id} onChange={(e) => setId(e.target.value)} />
          <input className="field w-20" type="number" placeholder="实例号" value={inst} onChange={(e) => setInst(e.target.value)} title="仅作业需要" />
          <input className="field flex-1 min-w-[200px]" placeholder="命令，如 nvidia-smi" value={cmd} onChange={(e) => setCmd(e.target.value)} onKeyDown={(e) => e.key === "Enter" && run()} />
          <button className="btn btn-pri" onClick={run} disabled={busy}><Play size={14} />执行</button>
          <button className="btn" onClick={term}><ExternalLink size={14} />网页终端</button>
        </div>
        <p className="text-xs text-ink-faint mt-3">命令经 bash 单行执行并捕获输出；平台网关约 2 分钟断长连接，长任务请 nohup 后台跑再看日志。</p>
      </Card>
      <pre className="card p-4 text-xs leading-relaxed overflow-x-auto whitespace-pre-wrap min-h-[200px] text-ink-soft">{out}</pre>
    </>
  );
}
