import { useState } from "react";
import { RefreshCw, Loader2, Plus } from "lucide-react";
import { api, AppState, asList, splitList, field, fmtTime, getMe, isAdmin } from "../lib/api";
import { useCached } from "../lib/cache";
import { PageBar, Spinner, Panel, Table, Th, Td, Badge, StatusDot, Stat } from "../components/ui";
import { Meter, Heatmap, HeatLegend, HeatNode } from "../components/viz";
import { Monitor } from "../components/Monitor";
import { CreateForm } from "../components/CreateForm";

const gpuOf = (j: any) => Number(field(j, ["statGpu", "gpuCount", "GPU"], 0)) || 0;
const statusOf = (j: any) => String(field(j, ["status"], "")).toUpperCase();

/** 热力图要画全部节点：按原生分页翻完整个集群。 */
async function allNodes() {
  const out: any[] = [];
  for (let page = 1; page <= 50; page++) {
    const d = await api.nodes({ page, size: 200 });
    const list = d.nodes || [];
    out.push(...list);
    if (list.length < 200 || out.length >= (Number(d.summary?.total) || 0)) break;
  }
  return out;
}

export function Overview({ state }: { state: AppState }) {
  const [mon, setMon] = useState<any>(null);
  // 预设对象只在打开时生成一次，避免页面重绘时反复覆盖表单里已改的选项
  const [create, setCreate] = useState<null | { initial?: any; label?: string }>(null);
  const [showCpu, setShowCpu] = useState(false);

  const { data, loading, refreshing, error, refresh } = useCached("overview", async () => {
    // 概览只要计数和最近几条：用原生分页按状态各取一页，listCount 就是精确总数
    const [idle, running, waiting, recent, devs] = await Promise.all([
      api.idle(),
      api.jobs({ status: "RUNNING", size: 6 }),
      api.jobs({ status: "CREATED,WAITING", size: 6 }),
      api.jobs({ size: 6 }),
      api.devs("mine").catch(() => ({ data: { devList: [] } })),
    ]);
    const list = (r: any) => (r.data?.jobList || []) as any[];
    const count = (r: any) => Number(r.data?.listCount) || 0;
    return {
      groups: idle.groups || [], devs: devs.data?.devList || [],
      running: list(running), runningCount: count(running), waitingCount: count(waiting),
      recent: [...list(running), ...list(waiting), ...list(recent)], totalCount: count(recent),
    };
  });

  const { data: nodes } = useCached<HeatNode[]>("overview-nodes", async () =>
    (await allNodes())
      .filter((n) => Number(n.gpuTotal) > 0)
      .map((n) => ({ name: n.name || n.ip, free: Number(n.gpuLeft) || 0, total: Number(n.gpuTotal) || 0,
        model: splitList(n.gpuModels)[0], group: splitList(n.resourceGroups)[0] })));

  if (loading) return <Spinner />;
  if (error && !data) return <Panel title="概览"><div className="p-4 text-body">加载失败：{error}。检查平台连接后点刷新。</div></Panel>;

  const groups = data!.groups;
  const gpuGroups = groups.filter((g: any) => Number(g.capacity?.gpuCount) > 0)
    .sort((a: any, b: any) => (Number(b.idle?.gpuWholeCardsFree) || 0) - (Number(a.idle?.gpuWholeCardsFree) || 0));
  const cpuGroups = groups.filter((g: any) => !Number(g.capacity?.gpuCount));
  const free = gpuGroups.reduce((a: number, g: any) => a + (Number(g.idle?.gpuWholeCardsFree) || 0), 0);
  const total = gpuGroups.reduce((a: number, g: any) => a + (Number(g.capacity?.gpuCount) || 0), 0);
  const models = [...new Set(gpuGroups.flatMap((g: any) => asList(g.capacity?.gpuTypes)))];

  const seen = new Set<string>();
  const recent = (data!.recent as any[]).filter((j) => { const id = String(field(j, ["jobId", "id"], "")); if (seen.has(id)) return false; seen.add(id); return true; }).slice(0, 6);
  const runningCount = data!.runningCount as number, waitingCount = data!.waitingCount as number, totalCount = data!.totalCount as number;
  const devs = data!.devs as any[];
  const devRunning = devs.filter((d) => String(field(d, ["jobenvStatus"], "")).toUpperCase() === "RUNNING").length;

  const me = getMe();
  const member = me.required && !isAdmin();
  const pickNode = (n: HeatNode) => {
    const g = groups.find((x: any) => (x.rsgroupName || x.rsgroupId) === n.group);
    setCreate(g ? { initial: { rsgroupId: String(g.rsgroupId), spaceId: g.spaceId }, label: g.rsgroupName || String(g.rsgroupId) } : {});
  };

  return (
    <>
      <PageBar title="概览"
        right={<div className="flex items-center gap-2">
          <button className="btn" onClick={refresh} aria-label="刷新">{refreshing ? <Loader2 size={14} className="animate-spin" /> : <RefreshCw size={14} />}刷新</button>
          <button className="btn btn-pri" onClick={() => setCreate({})}><Plus size={14} />提交作业</button>
        </div>} />

      <section className="card mb-4 grid grid-cols-2 md:grid-cols-4 divide-x divide-line">
        <Stat label="空闲 GPU" sub={models.join("、") || "没有 GPU 资源组"}>
          {free}<span className="text-body font-normal text-ink-faint"> / {total} 卡</span>
        </Stat>
        <Stat label="我的作业" sub={`排队 ${waitingCount}，共 ${totalCount} 个`}>
          <a href="#jobs" className="hover:text-brand">{runningCount}<span className="text-body font-normal text-ink-faint"> 运行中</span></a>
        </Stat>
        <Stat label="开发机" sub={`共 ${devs.length} 台`}>
          <a href="#devs" className="hover:text-brand">{devRunning}<span className="text-body font-normal text-ink-faint"> 运行中</span></a>
        </Stat>
        {member ? (
          <Stat label="我的 GPU 配额" sub={(me.quota?.total ?? 0) === 0 ? "未分配，联系管理员" : undefined}>
            {me.usage?.total ?? 0}<span className="text-body font-normal text-ink-faint"> / {me.quota?.total ?? 0} 卡</span>
          </Stat>
        ) : (
          <Stat label="GPU 使用率" sub={`${gpuGroups.length} 个 GPU 资源组`}>
            {total ? (((total - free) / total) * 100).toFixed(0) : "0"}<span className="text-body font-normal text-ink-faint"> %</span>
          </Stat>
        )}
      </section>

      <Panel className="mb-4" title="节点可用性" meta={`${(nodes || []).length} 个 GPU 节点，点方块在该资源组提交作业`} action={<HeatLegend />}>
        {nodes ? <Heatmap nodes={nodes} onPick={pickNode} /> : <Spinner label="读取节点…" />}
      </Panel>

      <div className="grid xl:grid-cols-[minmax(0,1fr)_360px] gap-4 items-start">
        <div>
          <Table head={<><Th>资源组</Th><Th>空间</Th><Th>GPU 型号</Th><Th num>空闲 / 总量</Th><Th num>单节点最多</Th><Th>使用率</Th><Th>状态</Th></>}>
            {(showCpu ? [...gpuGroups, ...cpuGroups] : gpuGroups).map((g: any, i: number) => {
              const idle = g.idle || {}, cap = g.capacity || {};
              const t = Number(cap.gpuCount) || 0, f = Number(idle.gpuWholeCardsFree) || 0, used = Math.max(0, t - f);
              return (
                <tr key={i} className="hover:bg-hover">
                  <Td><div className="font-medium">{g.rsgroupName || g.rsgroupId}</div></Td>
                  <Td className="text-ink-soft">{g.spaceName || "—"}</Td>
                  <Td clip className="text-ink-soft">{asList(cap.gpuTypes).join("、") || "—"}</Td>
                  <Td num><span className={f ? "font-medium" : "text-ink-faint"}>{f}</span><span className="text-ink-faint"> / {t}</span></Td>
                  <Td num>{idle.gpuMaxOnOneNode ?? "—"}</Td>
                  <Td><div className="flex items-center gap-2 w-28"><Meter value={used} total={t || 1} tone={t && used / t > 0.9 ? "danger" : "brand"} />
                    <span className="text-[11px] text-ink-faint w-8 text-right tabular-nums">{t ? Math.round((used / t) * 100) : 0}%</span></div></Td>
                  <Td><Badge text={g.ok === false ? "查询失败" : g.gpuAvailable ? "有空闲卡" : g.available ? "仅 CPU 可用" : "已满"} /></Td>
                </tr>
              );
            })}
            {!gpuGroups.length && !showCpu && <tr><Td className="text-ink-faint">没有 GPU 资源组</Td></tr>}
          </Table>
          {cpuGroups.length > 0 && (
            <button className="text-aux text-ink-faint hover:text-ink mt-2" onClick={() => setShowCpu((v) => !v)}>
              {showCpu ? "隐藏" : "显示"} {cpuGroups.length} 个纯 CPU 资源组
            </button>
          )}
        </div>

        <div className="space-y-4">
          <Panel title="我的作业" meta={`${runningCount} 个运行中`} action={<a href="#jobs" className="text-aux text-brand hover:underline">全部作业</a>}>
            {recent.length ? recent.map((j: any, i: number) => {
              const id = String(field(j, ["jobId", "id"], ""));
              return (
                <button key={i} className="list-row" onClick={() => setMon({ id, name: field(j, ["jobName", "name"]), space: j.spaceId || "", kind: "job", gpu: gpuOf(j), owner: j.qdOwner })}>
                  <StatusDot status={statusOf(j)} />
                  <span className="min-w-0 flex-1">
                    <span className="block text-[13px] truncate">{field(j, ["jobName", "name"])}</span>
                    <span className="block text-[11px] text-ink-faint truncate">{field(j, ["statusName", "status"])}，{fmtTime(j.createTime)}</span>
                  </span>
                  <span className="text-aux text-ink-soft tabular-nums shrink-0">{gpuOf(j)} 卡</span>
                </button>
              );
            }) : (
              <div className="px-4 py-4 text-aux text-ink-faint">还没有作业。<button className="text-brand hover:underline" onClick={() => setCreate({})}>提交一个</button></div>
            )}
          </Panel>

          <Panel title="开发机" meta={`${devRunning} 台运行中`} action={<a href="#devs" className="text-aux text-brand hover:underline">全部开发机</a>}>
            {devs.length ? devs.slice(0, 5).map((d: any, i: number) => (
              <a key={i} href="#devs" className="list-row">
                <StatusDot status={field(d, ["jobenvStatus"], "")} />
                <span className="min-w-0 flex-1 text-[13px] truncate">{field(d, ["projectName", "projectId"])}</span>
                <Badge text={field(d, ["jobenvStatus"], "—")} />
              </a>
            )) : <div className="px-4 py-4 text-aux text-ink-faint">还没有开发机，用 qd dev create 申请。</div>}
          </Panel>
        </div>
      </div>

      <Monitor job={mon} onClose={() => setMon(null)} baseUrl={state.baseUrl} />
      {create && <CreateForm kind="job" state={state} open initial={create.initial} groupLabel={create.label} onClose={() => setCreate(null)} onDone={refresh} />}
    </>
  );
}
