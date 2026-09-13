import { RefreshCw, Loader2 } from "lucide-react";
import { api, AppState, fmtMiB, asList } from "../lib/api";
import { useState } from "react";
import { useCached } from "../lib/cache";
import { Monitor } from "../components/Monitor";
import { Activity } from "lucide-react";
import { Card, PageBar, Tile, Badge, Spinner, Table, Th, Td } from "../components/ui";

export function Overview({ state }: { state: AppState }) {
  const [mon, setMon] = useState<any>(null);
  const { data, loading, refreshing, error, refresh } = useCached("overview", async () => {
    const [idle, jobs] = await Promise.all([api.idle(), api.jobs("mine", 200)]);
    return { groups: idle.groups || [], jobs: (jobs.data && jobs.data.jobList) || [] };
  });

  if (loading) return <Spinner />;
  if (error && !data) return <Card className="p-4 text-sm">加载失败：{error}</Card>;
  const groups = data?.groups || [], jobs = data?.jobs || [];
  const running = jobs.filter((j: any) => String(j.status).toUpperCase() === "RUNNING").length;
  const freeCards = groups.reduce((a: number, g: any) => a + (Number(g.idle?.gpuWholeCardsFree) || 0), 0);
  const availGroups = groups.filter((g: any) => g.gpuAvailable).length;

  return (
    <>
      <PageBar title="概览" hint="点资源组看节点与占用详情"
        right={<button className="btn" onClick={refresh}>{refreshing ? <Loader2 size={14} className="animate-spin" /> : <RefreshCw size={14} />}刷新</button>} />
      <div className="grid grid-cols-2 md:grid-cols-4 gap-3 mb-4">
        <Tile label="空闲整卡（全部资源组）" value={freeCards} unit="张" tone="mint" />
        <Tile label="有空闲 GPU 的资源组" value={availGroups} unit={`/ ${groups.length}`} />
        <Tile label="我运行中的作业" value={running} unit={`/ ${jobs.length}`} tone="amber" />
        <Tile label="登录身份" value={<span className="text-base">{state.username || "-"}</span>} tone="rose" />
      </div>
      {(() => {
        const run = jobs.filter((j: any) => String(j.status).toUpperCase() === "RUNNING");
        return run.length ? (
          <div className="mb-4">
            <div className="flex items-center gap-2 mb-2 text-sm font-medium text-ink-soft"><Activity size={14} className="text-mint" />运行中的作业 — 点击看实时监控与网页终端</div>
            <div className="flex flex-wrap gap-2">
              {run.map((j: any) => (
                <button key={String(j.jobId || j.id)} onClick={() => setMon({ id: String(j.jobId || j.id), name: j.jobName || j.name, space: j.spaceId || "", kind: "job" })}
                  className="card px-3 py-2 text-left hover:border-brand/40 transition-colors">
                  <div className="text-sm font-medium truncate max-w-[220px]">{j.jobName || j.name}</div>
                  <div className="text-[11px] text-ink-faint">{j.spaceName || ""} · {j.statGpu ?? j.gpuCount ?? 0} GPU</div>
                </button>
              ))}
            </div>
          </div>
        ) : null;
      })()}
      <Table head={<><Th>资源组</Th><Th>空间</Th><Th>GPU 型号</Th><Th num>空闲整卡/总量</Th><Th num>单节点最大</Th><Th num>有卡节点</Th><Th num>空闲CPU</Th><Th num>空闲内存</Th><Th>状态</Th></>}>
        {groups.length ? groups.map((g: any, i: number) => {
          const idle = g.idle || {}, cap = g.capacity || {};
          return (
            <tr key={i} className="hover:bg-canvas/60">
              <Td><div className="font-semibold text-brand-600">{g.rsgroupName || g.rsgroupId}</div><div className="text-[11px] text-ink-faint">{g.rsgroupId}</div></Td>
              <Td>{g.spaceName || "-"}</Td>
              <Td clip>{asList(cap.gpuTypes).join(", ") || "-"}</Td>
              <Td num><b>{idle.gpuWholeCardsFree ?? "-"}</b> / {cap.gpuCount ?? "-"}</Td>
              <Td num>{idle.gpuMaxOnOneNode ?? "-"}</Td>
              <Td num>{idle.nodesWithFreeGpu ?? "-"}</Td>
              <Td num>{idle.cpuCores ?? "-"}</Td>
              <Td num>{fmtMiB(idle.memoryMiB)}</Td>
              <Td><Badge text={g.ok === false ? "查询失败" : g.gpuAvailable ? "有空闲卡" : g.available ? "仅CPU可用" : "已满"} /></Td>
            </tr>
          );
        }) : <tr><Td className="text-ink-faint">无资源组</Td></tr>}
      </Table>
      <p className="text-xs text-ink-faint mt-3">空闲整卡按节点粒度统计；"单节点最大"决定单实例最多能要几张卡。</p>
      <Monitor job={mon} onClose={() => setMon(null)} baseUrl={state.baseUrl} />
    </>
  );
}
