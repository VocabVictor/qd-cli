import { useState, useEffect } from "react";
import { RefreshCw, Loader2, Plus, Activity, Terminal as TermIcon, Copy, CopyPlus } from "lucide-react";
import { api, AppState, pcall, spacesOf, fmtTime, field, asList } from "../lib/api";
import { useCached, dropCache } from "../lib/cache";
import { useList } from "../lib/table";
import { PageBar, Spinner, Card, Badge, Toolbar, useToast } from "../components/ui";
import { Meter, Ring, Spark, Rail, RailCard } from "../components/viz";
import { Monitor } from "../components/Monitor";
import { CreateForm } from "../components/CreateForm";

const CHIPS: [string, string, (s: string) => boolean][] = [
  ["running", "运行中", (s) => s === "RUNNING"],
  ["waiting", "排队中", (s) => /WAIT|QUEU|PEND|CREAT|START|INIT/.test(s)],
  ["succeeded", "已完成", (s) => /SUCCEED|SUCCESS/.test(s)],
  ["stopped", "已停止", (s) => /STOP|CANCEL|DELET|FAIL|ERROR/.test(s)],
  ["all", "全部", () => true],
];
const st = (j: any) => String(field(j, ["status"], "")).toUpperCase();
const METRIC_COLOR: Record<string, string> = { gpu_util: "#1070FE", gpu_memory: "#7C5CFC", cpu: "#2AA364", memory: "#E4900B" };

export function Jobs({ state }: { state: AppState }) {
  const toast = useToast();
  const spaces = spacesOf(state);
  const [scope, setScope] = useState("mine");
  const [chip, setChip] = useState("running");
  const [mon, setMon] = useState<any>(null);
  const [create, setCreate] = useState(false);
  const [preset, setPreset] = useState<any>(null);
  const [metrics, setMetrics] = useState<Record<string, any[]>>({});

  const { data: all, loading, refreshing, error, refresh } = useCached<any[]>(
    `jobs:${scope}`, async () => { const d = await api.jobs(scope, 500); return (d.data?.jobList) || []; }, [scope]);

  const { data: cluster } = useCached("jobs-cluster", async () => {
    const [idle, quota] = await Promise.all([
      api.idle(),
      pcall("GET", "core", "/user/quota/available", {}, state.spaceId).then((d) => d.data).catch(() => null),
    ]);
    return { groups: idle.groups || [], quota };
  });

  const { view, search, setSearch, filters, setFilter, sortKey, sortDir, toggleSort } = useList(all || [], {
    searchText: (j) => [field(j, ["jobName", "name"], ""), field(j, ["jobId", "id"], ""), field(j, ["projectName"], ""), st(j), j.spaceName || ""].join(" "),
    sorts: { created: (j) => Number(new Date(j.createTime).getTime()) || 0 },
    initialSort: { key: "created", dir: "desc" },
  });
  const shown = view.filter((j) => CHIPS.find((c) => c[0] === chip)![2](st(j)));

  // 运行中卡片的曲线（最多 6 个）
  useEffect(() => {
    shown.filter((j) => st(j) === "RUNNING").slice(0, 6).forEach((j: any) => {
      const id = String(field(j, ["jobId", "id"], ""));
      if (!id || metrics[id]) return;
      api.jobMetrics(id, 10, j.spaceId || "").then((d) => setMetrics((m) => ({ ...m, [id]: d.metrics || [] }))).catch(() => {});
    });
  }, [all, chip]); // eslint-disable-line

  const act = async (action: string, id: string, sp: string) => {
    try { const r = await api.bulk(action, [id], sp); const bad = r.find((x: any) => !x.ok);
      bad ? toast(bad.error, true) : toast("已" + (action.includes("cancel") ? "取消" : "删除") + " " + id); dropCache("jobs"); dropCache("overview"); refresh(); }
    catch (e: any) { toast(e.message, true); }
  };

  // 复制作业：拉详情，还原成新建表单能吃的配置
  const cloneJob = async (id: string, sp: string, name: string) => {
    toast("正在读取作业配置…");
    try {
      const d = await api.job(id, sp);
      const j = d?.data || d || {};
      const role = (j.taskroleList || j.taskroles || [])[0] || {};
      setPreset({
        jobName: (name || "job") + "-copy",
        spaceId: j.spaceId || sp,
        projectId: String(field(j, ["projectId"], "")),
        rsgroupId: String(field(j, ["rsgroupId"], "")),
        imageId: field(j, ["imageId"], 0),
        maxRunHour: Number(field(j, ["maxRunHour"], 24)) || 24,
        taskroles: [{
          runScript: field(role, ["runScript", "command"], ""),
          cpu: Number(field(role, ["cpu"], 0)) || undefined,
          memory: Number(field(role, ["memory"], 0)) || undefined,
          storage: Number(field(role, ["storage"], 0)) || undefined,
          gpu: Number(field(role, ["gpu", "gpuNumber"], 0)) || 0,
          gpuType: field(role, ["gpuType", "gpuModel"], ""),
        }],
      });
      setCreate(true);
    } catch (e: any) { toast("读取配置失败：" + e.message, true); }
  };

  const q = cluster?.quota;
  const topGroups = (cluster?.groups || []).filter((g: any) => Number(g.capacity?.gpuCount)).slice(0, 3);

  return (
    <div className="flex gap-4 items-start">
      <div className="flex-1 min-w-0">
        <PageBar title="作业" hint="任务卡内嵌实时监控，点卡片看大图与终端"
          right={<div className="flex items-center gap-2">
            <select className="field w-24" value={scope} onChange={(e) => setScope(e.target.value)}>
              {["mine", "shared", "public", "all"].map((s) => <option key={s}>{s}</option>)}</select>
            <button className="btn" onClick={refresh}>{refreshing ? <Loader2 size={14} className="animate-spin" /> : <RefreshCw size={14} />}刷新</button>
          </div>} />

        {/* 集群总览条 */}
        <Card className="card-pad mb-4">
          <div className="flex flex-wrap items-center gap-5">
            {topGroups.map((g: any, i: number) => {
              const total = Number(g.capacity?.gpuCount) || 0, free = Number(g.idle?.gpuWholeCardsFree) || 0;
              return (
                <div key={i} className="flex items-center gap-3">
                  <Ring pct={total ? (free / total) * 100 : 0} tone="ok" />
                  <div>
                    <div className="text-aux text-ink-soft flex items-center gap-1.5">
                      <span className="w-1.5 h-1.5 rounded-full bg-ok" />{g.rsgroupName || g.rsgroupId}</div>
                    <div><b className="text-[22px] leading-tight">{free}</b>
                      <span className="text-body text-ink-faint"> / {total} 张</span></div>
                    <div className="text-[11px] text-ink-faint">{asList(g.capacity?.gpuTypes)[0] || "-"}</div>
                  </div>
                </div>
              );
            })}
            {q && (
              <div className="min-w-[220px]">
                <div className="text-aux text-ink-soft mb-1">我的空间配额</div>
                <div className="text-body mb-1">已用 <b>{q.usedGpuCardCnt ?? 0}</b> / {(q.gpuquotaDataList?.[0]?.cardCnt) ?? q.gpuCardCnt ?? 0} 卡</div>
                <Meter value={q.usedGpuCardCnt || 0} total={(q.gpuquotaDataList?.[0]?.cardCnt) || q.gpuCardCnt || 1} tone="brand" />
              </div>
            )}
            <div className="flex gap-5 ml-auto">
              {[["运行中作业", (all || []).filter((j) => st(j) === "RUNNING").length],
                ["排队中作业", (all || []).filter((j) => /WAIT|QUEU|PEND/.test(st(j))).length],
                ["作业总数", (all || []).length]].map(([l, v]) => (
                <div key={String(l)} className="text-center">
                  <div className="text-[22px] font-semibold leading-tight">{v as number}</div>
                  <div className="text-[11px] text-ink-faint">{l as string}</div>
                </div>
              ))}
            </div>
          </div>
        </Card>

        <Toolbar search={search} onSearch={setSearch} placeholder="搜索作业名 / ID / 项目">
          {spaces.length > 1 && (
            <select className="field w-28" value={filters.space || ""} onChange={(e) => setFilter("space", e.target.value, (j, v) => j.spaceId === v)}>
              <option value="">全部空间</option>{spaces.map((s) => <option key={s.id} value={s.id}>{s.name}</option>)}</select>
          )}
          <div className="flex-1" />
          <button className="btn btn-pri" onClick={() => setCreate(true)}><Plus size={14} />新建作业</button>
        </Toolbar>

        <div className="flex items-center gap-1.5 mb-3 flex-wrap">
          {CHIPS.map(([k, label]) => {
            const n = view.filter((j) => CHIPS.find((c) => c[0] === k)![2](st(j))).length;
            return <button key={k} onClick={() => setChip(k)} className={"chip " + (chip === k ? "chip-on" : "")}>{label}<span className="ml-1 text-ink-faint">{n}</span></button>;
          })}
        </div>

        {error && !all && <Card className="card-pad text-body mb-4">加载失败：{error}</Card>}
        {loading ? <Spinner /> : (
          <div className="space-y-3">
            {shown.length ? shown.map((j: any, i: number) => {
              const id = String(field(j, ["jobId", "id"], "")), sp = j.spaceId || "";
              const isRun = st(j) === "RUNNING";
              const ms = metrics[id] || [];
              return (
                <Card key={i} className="p-4 hover:border-brand/30 transition-colors">
                  <div className="flex items-start gap-4">
                    <div className="min-w-[230px] max-w-[280px]">
                      <div className="flex items-center gap-2 mb-1">
                        <Badge text={field(j, ["statusName", "status"])} />
                        <a className="text-[15px] font-semibold text-brand hover:underline cursor-pointer truncate"
                          onClick={() => setMon({ id, name: field(j, ["jobName", "name"]), space: sp, kind: "job", gpu: Number(field(j, ["statGpu", "gpuCount", "GPU"], 0)) || 0 })}>
                          {field(j, ["jobName", "name"])}</a>
                      </div>
                      <div className="text-[11px] text-ink-faint flex items-center gap-1">
                        #{id}<button className="hover:text-brand" onClick={() => { navigator.clipboard?.writeText(id); toast("已复制作业 ID"); }}><Copy size={11} /></button>
                      </div>
                      <div className="flex flex-wrap gap-1.5 mt-2">
                        <span className="tag tag-gray">{field(j, ["projectName", "projectId"], "-")}</span>
                        <span className="tag tag-gray">{j.spaceName || "-"}</span>
                      </div>
                      <div className="text-[11px] text-ink-faint mt-2">创建 {fmtTime(j.createTime)}</div>
                    </div>

                    <div className="shrink-0 text-center px-3">
                      <div className="text-[20px] font-semibold leading-tight">{field(j, ["statGpu", "gpuCount", "GPU"], 0)}</div>
                      <div className="text-[11px] text-ink-faint">GPU</div>
                    </div>

                    <div className="flex-1 min-w-0">
                      {isRun ? (ms.length ? (() => {
                        const hasGpu = Number(field(j, ["statGpu", "gpuCount", "GPU"], 0)) > 0;
                        const shownMs = ms.filter((m: any) => hasGpu || !m.metric.startsWith("gpu"));
                        return (
                        <div className={"grid gap-3 " + (shownMs.length > 2 ? "grid-cols-4" : "grid-cols-2 max-w-md")}>
                          {shownMs.map((m: any) => (
                            <div key={m.metric}>
                              <div className="text-[11px] text-ink-faint">{m.label}</div>
                              <div className="text-[15px] font-semibold leading-tight">{m.last != null ? m.last.toFixed(0) : "-"}<span className="text-[11px] font-normal text-ink-faint">{m.unit}</span></div>
                              <Spark points={m.points || []} color={METRIC_COLOR[m.metric]} h={30} />
                            </div>
                          ))}
                        </div>
                        ); })()
                      : <div className="text-aux text-ink-faint flex items-center gap-1.5 py-4"><Loader2 size={12} className="animate-spin" />读取实时监控…</div>)
                        : <div className="text-aux text-ink-faint py-4">非运行状态，无实时监控</div>}
                    </div>

                    <div className="shrink-0 flex flex-col gap-1.5">
                      {isRun && <button className="btn btn-mini" onClick={() => setMon({ id, name: field(j, ["jobName", "name"]), space: sp, kind: "job", gpu: Number(field(j, ["statGpu", "gpuCount", "GPU"], 0)) || 0 })}><Activity size={12} />监控</button>}
                      {isRun && <button className="btn btn-mini" onClick={() => setMon({ id, name: field(j, ["jobName", "name"]), space: sp, kind: "job", gpu: Number(field(j, ["statGpu", "gpuCount", "GPU"], 0)) || 0 })}><TermIcon size={12} />终端</button>}
                      <button className="btn btn-mini" onClick={() => cloneJob(id, sp, String(field(j, ["jobName", "name"], "")))}><CopyPlus size={12} />复制</button>
                      <button className="btn btn-mini" onClick={() => act("job-cancel", id, sp)}>取消</button>
                      <button className="btn btn-mini btn-danger" onClick={() => act("job-delete", id, sp)}>删除</button>
                    </div>
                  </div>
                </Card>
              );
            }) : <Card className="card-pad text-body text-ink-faint">没有匹配的作业</Card>}
            <p className="text-aux text-ink-faint">显示 {shown.length} / {(all || []).length} 个（scope={scope}）</p>
          </div>
        )}
      </div>

      <Rail>
        <RailCard title="快速创建">
          <p className="text-aux text-ink-faint mb-3">选镜像与资源规格，一键提交训练作业或申请开发机。</p>
          <button className="btn btn-pri w-full mb-2" onClick={() => setCreate(true)}><Plus size={14} />新建作业</button>
          <button className="btn w-full" onClick={() => { location.hash = "devs"; }}>去申请开发机</button>
        </RailCard>
        <RailCard title="常用工具">
          <div className="grid grid-cols-2 gap-2">
            {[["开发机", "devs"], ["镜像仓库", "images"], ["文件传输", "transfer"], ["配额", "quota"]].map(([l, h]) => (
              <button key={h} className="btn h-auto py-2.5" onClick={() => { location.hash = h; }}>{l}</button>
            ))}
          </div>
        </RailCard>
        <RailCard title="集群资源" action={<button className="text-aux text-brand hover:underline" onClick={() => { location.hash = "nodes"; }}>节点 →</button>}>
          {(cluster?.groups || []).slice(0, 5).map((g: any, i: number) => {
            const total = Number(g.capacity?.gpuCount) || 0, free = Number(g.idle?.gpuWholeCardsFree) || 0;
            return (
              <div key={i} className="mb-2.5 last:mb-0">
                <div className="flex items-baseline justify-between text-aux">
                  <span className="truncate">{g.rsgroupName || g.rsgroupId}</span>
                  <span className="text-ink-faint"><b className={free ? "text-ok" : ""}>{free}</b> / {total}</span>
                </div>
                <div className="mt-1"><Meter value={total - free} total={total || 1} tone={total && (total - free) / total > 0.9 ? "danger" : "brand"} /></div>
              </div>
            );
          })}
        </RailCard>
      </Rail>

      <Monitor job={mon} onClose={() => setMon(null)} baseUrl={state.baseUrl} />
      <CreateForm kind="job" state={state} open={create} initial={preset} onClose={() => { setCreate(false); setPreset(null); }} onDone={refresh} />
    </div>
  );
}
