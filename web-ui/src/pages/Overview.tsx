import { useState, useEffect } from "react";
import { RefreshCw, Loader2, Plus, Boxes, Upload, ExternalLink } from "lucide-react";
import { api, AppState, pcall, eachSpace, asList, field, qget } from "../lib/api";
import { useCached } from "../lib/cache";
import { Card, PageBar, Spinner, Table, Th, Td, Badge, useToast } from "../components/ui";
import { Meter, Heatmap, Spark, Rail, RailCard, HeatNode } from "../components/viz";
import { Monitor } from "../components/Monitor";
import { CreateForm } from "../components/CreateForm";

const METRIC_COLOR: Record<string, string> = { gpu_util: "#1070FE", gpu_memory: "#7C5CFC", cpu: "#2AA364", memory: "#E4900B" };

export function Overview({ state }: { state: AppState }) {
  const toast = useToast();
  const [mon, setMon] = useState<any>(null);
  const [create, setCreate] = useState<null | "job" | "dev">(null);
  const [metrics, setMetrics] = useState<Record<string, any[]>>({});
  const [showCpuOnly, setShowCpuOnly] = useState(false);

  const { data, loading, refreshing, error, refresh } = useCached("overview", async () => {
    const [idle, jobs, devs] = await Promise.all([
      api.idle(),
      api.jobs("mine", 200),
      api.devs("mine").catch(() => ({ data: { devList: [] } })),
    ]);
    return {
      groups: idle.groups || [],
      jobs: (jobs.data && jobs.data.jobList) || [],
      devs: (devs.data && devs.data.devList) || [],
    };
  });

  // 节点热力图数据（独立缓存，失败不影响主页面）
  const { data: nodes } = useCached<HeatNode[]>("overview-nodes", async () => {
    const per = await eachSpace(state, (sp) =>
      pcall("GET", "core", "/jobQueue/node/job/list", { pageNum: 1, pageSize: 500, sortField: "left_gpu_count", sortOrder: "desc" }, sp.id));
    const seen = new Set<string>(); const out: HeatNode[] = [];
    for (const { data } of per)
      for (const n of (data.data?.nodes || [])) {
        const key = n.name || n.ip; if (seen.has(key)) continue; seen.add(key);
        if (!Number(n.gpuTotal)) continue;              // 只画有 GPU 的节点
        out.push({ name: n.name || n.ip, free: Number(n.gpuLeft) || 0, total: Number(n.gpuTotal) || 0,
          model: asList(n.gpuModels)[0], group: asList(n.resourceGroups)[0] });
      }
    return out.sort((a, b) => b.free - a.free);
  });

  const running = (data?.jobs || []).filter((j: any) => String(j.status).toUpperCase() === "RUNNING");

  // 运行中作业的迷你曲线（最多 3 个，渲染后再拉）
  useEffect(() => {
    running.slice(0, 3).forEach((j: any) => {
      const id = String(field(j, ["jobId", "id"], ""));
      if (!id || metrics[id]) return;
      api.jobMetrics(id, 10, j.spaceId || "").then((d) => setMetrics((m) => ({ ...m, [id]: d.metrics || [] }))).catch(() => {});
    });
  }, [data]); // eslint-disable-line

  if (loading) return <Spinner />;
  if (error && !data) return <Card className="card-pad text-body">加载失败：{error}</Card>;

  // 按 GPU 型号聚合可用量
  const byModel = new Map<string, { free: number; total: number }>();
  for (const g of data!.groups) {
    const models = asList(g.capacity?.gpuTypes);
    if (!models.length) continue;
    const m = models[0];
    const cur = byModel.get(m) || { free: 0, total: 0 };
    cur.free += Number(g.idle?.gpuWholeCardsFree) || 0;
    cur.total += Number(g.capacity?.gpuCount) || 0;
    byModel.set(m, cur);
  }
  const models = [...byModel.entries()];
  const allFree = models.reduce((a, [, v]) => a + v.free, 0);
  const allTotal = models.reduce((a, [, v]) => a + v.total, 0);

  // 有 GPU 的资源组排前面，按空闲卡降序；纯 CPU 组收到后面
  const sortedGroups = [...data!.groups].sort((a: any, b: any) => {
    const ga = Number(a.capacity?.gpuCount) || 0, gb = Number(b.capacity?.gpuCount) || 0;
    if ((ga > 0) !== (gb > 0)) return gb - ga;
    return (Number(b.idle?.gpuWholeCardsFree) || 0) - (Number(a.idle?.gpuWholeCardsFree) || 0);
  });
  const gpuGroups = sortedGroups.filter((g: any) => Number(g.capacity?.gpuCount) > 0);
  const cpuGroups = sortedGroups.filter((g: any) => !Number(g.capacity?.gpuCount));
  const listedGroups = showCpuOnly ? sortedGroups : gpuGroups;

  const openTerm = async (kind: "job" | "dev", id: string, sp: string) => {
    try { const r = await qget(`/api/terminal-url?kind=${kind}&id=${encodeURIComponent(id)}&instance=0${sp ? "&space=" + sp : ""}`);
      window.open(r.terminalUrl, "_blank"); }
    catch (e: any) { toast(e.message, true); }
  };

  return (
    <div className="flex gap-4 items-start">
      <div className="flex-1 min-w-0">
        <PageBar title="概览" hint="集群可用性一览，点节点方块可直接开任务"
          right={<button className="btn" onClick={refresh}>{refreshing ? <Loader2 size={14} className="animate-spin" /> : <RefreshCw size={14} />}刷新</button>} />

        {/* 节点可用性热力图 */}
        <Card className="card-pad mb-4">
          <div className="flex items-center flex-wrap gap-x-6 gap-y-2 mb-4">
            <div>
              <h2 className="text-h2">节点 GPU 可用性</h2>
              <span className="text-aux text-ink-faint">{(nodes || []).length} 个有卡节点，悬停看详情，点方块直接开任务</span>
            </div>
            <div className="flex-1" />
            <div className="flex items-end gap-6">
              <div>
                <div className="text-aux text-ink-soft">空闲整卡</div>
                <div><span className="text-big text-ok">{allFree}</span>
                  <span className="text-body text-ink-faint"> / {allTotal} 张</span></div>
              </div>
              <div>
                <div className="text-aux text-ink-soft">空闲率</div>
                <div className="text-[22px] font-semibold leading-tight">{allTotal ? ((allFree / allTotal) * 100).toFixed(1) : "0.0"}<span className="text-body font-normal text-ink-faint">%</span></div>
              </div>
              <div>
                <div className="text-aux text-ink-soft">GPU 型号</div>
                <div className="text-[13px] leading-tight pb-1">{models.map(([m]) => m).join(" / ") || "-"}</div>
              </div>
            </div>
          </div>
          <Heatmap nodes={nodes || []} onPick={(n) => { toast(`${n.name}：可用 ${n.free}/${n.total} 张`); setCreate("job"); }} />
        </Card>

        {/* 资源组表 */}
        <Table head={<><Th>资源组</Th><Th>空间</Th><Th>GPU 型号</Th><Th num>空闲/总量</Th><Th num>单节点最大</Th><Th num>有卡节点</Th><Th>使用率</Th><Th>状态</Th></>}>
          {listedGroups.length ? listedGroups.map((g: any, i: number) => {
            const idle = g.idle || {}, cap = g.capacity || {};
            const total = Number(cap.gpuCount) || 0, free = Number(idle.gpuWholeCardsFree) || 0;
            const used = Math.max(0, total - free);
            return (
              <tr key={i} className="hover:bg-canvas">
                <Td><div className="font-semibold text-brand">{g.rsgroupName || g.rsgroupId}</div>
                  <div className="text-[11px] text-ink-faint">{g.rsgroupId}</div></Td>
                <Td>{g.spaceName || "-"}</Td>
                <Td clip>{asList(cap.gpuTypes).join(", ") || "-"}</Td>
                <Td num><b className={free > 0 ? "text-ok" : ""}>{free}</b> / {total}</Td>
                <Td num>{idle.gpuMaxOnOneNode ?? "-"}</Td>
                <Td num>{idle.nodesWithFreeGpu ?? "-"}</Td>
                <Td><div className="w-24"><Meter value={used} total={total} tone={total && used / total > 0.9 ? "danger" : "brand"} />
                  <span className="text-[11px] text-ink-faint">{total ? ((used / total) * 100).toFixed(1) : "0.0"}%</span></div></Td>
                <Td><Badge text={g.ok === false ? "查询失败" : g.gpuAvailable ? "有空闲卡" : g.available ? "仅CPU可用" : "已满"} /></Td>
              </tr>
            );
          }) : <tr><Td className="text-ink-faint">无资源组</Td></tr>}
        </Table>
        {cpuGroups.length > 0 && (
          <button className="text-aux text-ink-faint hover:text-brand mt-2" onClick={() => setShowCpuOnly((v) => !v)}>
            {showCpuOnly ? "隐藏" : "显示"}纯 CPU 资源组（{cpuGroups.length} 个）
          </button>
        )}
      </div>

      {/* 右侧常驻栏 */}
      <Rail>
        <RailCard title="快速开始">
          <div className="grid grid-cols-2 gap-2">
            <button className="btn btn-pri h-auto py-3 flex-col gap-1" onClick={() => setCreate("job")}>
              <span className="flex items-center gap-1.5"><Plus size={14} />提交训练作业</span>
              <span className="text-[11px] font-normal opacity-80">选镜像、资源，一键启动</span>
            </button>
            <button className="btn btn-ok h-auto py-3 flex-col gap-1" onClick={() => setCreate("dev")}>
              <span className="flex items-center gap-1.5"><Plus size={14} />创建开发机</span>
              <span className="text-[11px] font-normal opacity-80">支持 VS Code 远程连接</span>
            </button>
            <button className="btn h-auto py-2.5 flex-col gap-0.5" onClick={() => { location.hash = "transfer"; }}>
              <span className="flex items-center gap-1.5"><Upload size={13} />文件传输</span>
              <span className="text-[11px] text-ink-faint">SFTP 上传下载</span>
            </button>
            <button className="btn h-auto py-2.5 flex-col gap-0.5" onClick={() => { location.hash = "images"; }}>
              <span className="flex items-center gap-1.5"><Boxes size={13} />镜像仓库</span>
              <span className="text-[11px] text-ink-faint">选训练镜像</span>
            </button>
          </div>
        </RailCard>

        <RailCard title={`我正在运行的作业 (${running.length})`}
          action={<button className="text-aux text-brand hover:underline" onClick={() => { location.hash = "jobs"; }}>查看全部 →</button>}>
          {running.length ? <div className="space-y-3">
            {running.slice(0, 3).map((j: any, i: number) => {
              const id = String(field(j, ["jobId", "id"], ""));
              const ms = metrics[id] || [];
              return (
                <div key={i} className="border border-line rounded-card p-3 hover:border-brand/40 cursor-pointer transition-colors"
                  onClick={() => setMon({ id, name: field(j, ["jobName", "name"]), space: j.spaceId || "", kind: "job", gpu: Number(field(j, ["statGpu", "gpuCount", "GPU"], 0)) || 0 })}>
                  <div className="flex items-center gap-2">
                    <span className="w-1.5 h-1.5 rounded-full bg-ok shrink-0" />
                    <span className="text-body font-medium truncate flex-1">{field(j, ["jobName", "name"])}</span>
                    <span className="text-aux text-ink-faint shrink-0">{field(j, ["statGpu", "gpuCount", "GPU"], 0)} GPU</span>
                  </div>
                  {ms.length ? (
                    <div className="grid grid-cols-2 gap-2 mt-2">
                      {ms.filter((m: any) => Number(field(j, ["statGpu", "gpuCount", "GPU"], 0)) > 0 || !m.metric.startsWith("gpu")).map((m: any) => (
                        <div key={m.metric}>
                          <div className="flex items-baseline justify-between">
                            <span className="text-[11px] text-ink-faint">{m.label}</span>
                            <span className="text-[12px] font-semibold">{m.last != null ? m.last.toFixed(0) + m.unit : "-"}</span>
                          </div>
                          <Spark points={m.points || []} color={METRIC_COLOR[m.metric]} h={26} />
                        </div>
                      ))}
                    </div>
                  ) : <div className="text-[11px] text-ink-faint mt-2 flex items-center gap-1"><Loader2 size={11} className="animate-spin" />读取监控…</div>}
                </div>
              );
            })}
          </div> : <p className="text-aux text-ink-faint">当前没有运行中的作业</p>}
        </RailCard>

        <RailCard title={`我的开发机 (${(data!.devs || []).length})`}
          action={<button className="text-aux text-brand hover:underline" onClick={() => { location.hash = "devs"; }}>查看全部 →</button>}>
          {(data!.devs || []).length ? <div className="space-y-2">
            {data!.devs.slice(0, 3).map((d: any, i: number) => {
              const id = String(field(d, ["jobenvId"], ""));
              return (
                <div key={i} className="border border-line rounded-card p-3">
                  <div className="flex items-center gap-2">
                    <span className="text-body font-medium truncate flex-1">{field(d, ["projectName", "projectId"])}</span>
                    <Badge text={field(d, ["jobenvStatus"], "-")} />
                  </div>
                  <div className="flex items-center gap-1.5 mt-2">
                    <span className="tag tag-gray">ssh</span><span className="tag tag-gray">vscode</span>
                    <div className="flex-1" />
                    {String(field(d, ["jobenvStatus"], "")).toUpperCase() === "RUNNING"
                      ? <button className="btn btn-mini" onClick={() => openTerm("dev", id, d.spaceId || "")}><ExternalLink size={11} />连接</button>
                      : <button className="btn btn-mini" onClick={() => { location.hash = "devs"; }}>去启动</button>}
                  </div>
                </div>
              );
            })}
          </div> : <p className="text-aux text-ink-faint">还没有开发机<button className="text-brand hover:underline ml-1" onClick={() => setCreate("dev")}>去创建</button></p>}
        </RailCard>
      </Rail>

      <Monitor job={mon} onClose={() => setMon(null)} baseUrl={state.baseUrl} />
      {create && <CreateForm kind={create} state={state} open={!!create} onClose={() => setCreate(null)} onDone={refresh} />}
    </div>
  );
}
