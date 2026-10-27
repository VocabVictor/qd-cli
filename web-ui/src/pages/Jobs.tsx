import { useState, useEffect } from "react";
import { RefreshCw, Loader2, Plus, Terminal as TermIcon, CopyPlus, Square, Trash2 } from "lucide-react";
import { api, AppState, spacesOf, fmtTime, field, canManageJob, getMe } from "../lib/api";
import { useCached, dropCache } from "../lib/cache";
import { PageBar, Spinner, Badge, Toolbar, StatusDot, Th, Pager, Chips, Empty, useToast } from "../components/ui";
import { Spark } from "../components/viz";
import { Monitor, MonitorJob } from "../components/Monitor";
import { CreateForm } from "../components/CreateForm";

// 平台原生状态（来自平台前端的枚举）。筛选、计数、分页都交给平台做。
const FILTERS: [string, string, string][] = [
  ["running", "运行中", "RUNNING"],
  ["waiting", "排队中", "CREATED,WAITING"],
  ["succeeded", "已完成", "SUCCEEDED"],
  ["stopped", "已停止", "STOPPING,STOPPED,FAILED"],
  ["all", "全部", ""],
];
const PAGE_SIZE = 20;
const st = (j: any) => String(field(j, ["status"], "")).toUpperCase();
const gpuOf = (j: any) => Number(field(j, ["statGpu", "gpuCount", "GPU"], 0)) || 0;
const NOT_MINE = "不是你通过 qd web 提交的作业，只能查看";

export function Jobs({ state }: { state: AppState }) {
  const toast = useToast();
  const spaces = spacesOf(state);
  const me = getMe();
  const [scope, setScope] = useState("mine");
  const [filter, setFilterKey] = useState("running");
  const [space, setSpace] = useState("");
  const [input, setInput] = useState("");
  const [q, setQ] = useState("");
  const [page, setPage] = useState(1);
  const [mon, setMon] = useState<{ job: MonitorJob; tab?: "metrics" | "terminal" } | null>(null);
  const [create, setCreate] = useState(false);
  const [preset, setPreset] = useState<any>(null);
  const [util, setUtil] = useState<Record<string, number[]>>({});

  // 关键字输入防抖 300ms 再发给平台；换筛选条件时回到第一页
  useEffect(() => { const t = setTimeout(() => setQ(input.trim()), 300); return () => clearTimeout(t); }, [input]);
  useEffect(() => setPage(1), [scope, filter, space, q]);

  const status = FILTERS.find((f) => f[0] === filter)![2];
  const { data, loading, refreshing, error, refresh } = useCached<{ list: any[]; total: number; skipped: number; failed: string[] }>(
    `jobs:${scope}:${filter}:${space}:${q}:${page}`, async () => {
      const d = await api.jobs({ scope, status, q, space, page, size: PAGE_SIZE });
      return { list: d.data?.jobList || [], total: Number(d.data?.listCount) || 0, skipped: Number(d.data?.skippedProjects) || 0, failed: d.data?.failedSpaces || [] };
    }, [scope, filter, space, q, page]);

  // 每个状态的精确数量：各自要一条（size=1），只看平台返回的 listCount
  const { data: counts } = useCached<Record<string, number>>(`jobs-count:${scope}:${space}:${q}`, async () => {
    const pairs = await Promise.all(FILTERS.map(async ([k, , st]) => [k, Number((await api.jobs({ scope, status: st, q, space, size: 1 })).data?.listCount) || 0] as const));
    return Object.fromEntries(pairs);
  }, [scope, space, q]);

  const shown = data?.list || [];
  const total = data?.total || 0;

  // 运行中的作业补一条利用率迷你曲线（最多 12 个，渲染后再拉）
  useEffect(() => {
    shown.filter((j) => st(j) === "RUNNING").slice(0, 12).forEach((j: any) => {
      const id = String(field(j, ["jobId", "id"], ""));
      if (!id || util[id]) return;
      api.jobMetrics(id, 10, j.spaceId || "").then((d) => {
        const m = (d.metrics || []).find((x: any) => x.metric === (gpuOf(j) ? "gpu_util" : "cpu"));
        setUtil((u) => ({ ...u, [id]: m?.points || [] }));
      }).catch(() => {});
    });
  }, [data]); // eslint-disable-line

  const act = async (action: "job-cancel" | "job-delete", j: any) => {
    const id = String(field(j, ["jobId", "id"], "")), name = field(j, ["jobName", "name"], id);
    if (!confirm(action === "job-cancel" ? `取消作业 ${name}？运行中的进程会被停止。` : `删除作业 ${name}？记录和日志将无法再查看。`)) return;
    try {
      const r = await api.bulk(action, [id], j.spaceId || "");
      const bad = r.find((x: any) => !x.ok);
      if (bad) toast(bad.error, true); else toast((action === "job-cancel" ? "已取消 " : "已删除 ") + name);
      dropCache("jobs"); dropCache("overview"); refresh();
    } catch (e: any) { toast(e.message, true); }
  };

  // 复制作业：拉详情，还原成提交表单能用的配置
  const cloneJob = async (j: any) => {
    const id = String(field(j, ["jobId", "id"], "")), sp = j.spaceId || "";
    try {
      const d = await api.job(id, sp);
      const x = d?.data || d || {};
      const role = (x.taskroleList || x.taskroles || [])[0] || {};
      setPreset({
        jobName: field(j, ["jobName", "name"], "job") + "-copy", spaceId: x.spaceId || sp,
        projectId: String(field(x, ["projectId"], "")), rsgroupId: String(field(x, ["rsgroupId"], "")),
        imageId: field(x, ["imageId"], 0), maxRunHour: Number(field(x, ["maxRunHour"], 24)) || 24,
        taskroles: [{ runScript: field(role, ["runScript", "command"], ""), cpu: Number(field(role, ["cpu"], 0)) || undefined,
          memory: Number(field(role, ["memory"], 0)) || undefined, storage: Number(field(role, ["storage"], 0)) || undefined,
          gpu: Number(field(role, ["gpu", "gpuNumber"], 0)) || 0, gpuType: field(role, ["gpuType", "gpuModel"], "") }],
      });
      setCreate(true);
    } catch (e: any) { toast("读取作业配置失败：" + e.message, true); }
  };

  const monitorJob = (j: any): MonitorJob => ({ id: String(field(j, ["jobId", "id"], "")), name: field(j, ["jobName", "name"]), space: j.spaceId || "", kind: "job", gpu: gpuOf(j), owner: j.qdOwner });

  return (
    <>
      <PageBar title="作业" hint={counts ? `${counts.running} 个运行中，${counts.waiting} 个排队` : undefined}
        right={<div className="flex items-center gap-2">
          <button className="btn" onClick={refresh}>{refreshing ? <Loader2 size={14} className="animate-spin" /> : <RefreshCw size={14} />}刷新</button>
          <button className="btn btn-pri" onClick={() => { setPreset(null); setCreate(true); }}><Plus size={14} />提交作业</button>
        </div>} />

      <Toolbar search={input} onSearch={setInput} placeholder="按作业名搜索">
        <Chips label="按状态筛选" items={FILTERS.map(([k, l]) => [k, l] as [string, string])} value={filter} onChange={setFilterKey} counts={counts || undefined} />
        <div className="flex-1" />
        {spaces.length > 1 && (
          <select className="field w-36" aria-label="空间" value={space} onChange={(e) => setSpace(e.target.value)}>
            <option value="">全部空间</option>{spaces.map((s) => <option key={s.id} value={s.id}>{s.name}</option>)}</select>
        )}
        <select className="field w-32" aria-label="范围" value={scope} onChange={(e) => setScope(e.target.value)}>
          <option value="mine">我的项目</option><option value="shared">共享项目</option><option value="public">公开项目</option><option value="all">全部项目</option>
        </select>
      </Toolbar>

      {error && !data && <div className="card p-4 text-body mb-3">作业列表读取失败：{error}。检查平台连接后点刷新。</div>}
      {loading ? <Spinner /> : (
        <div className="card overflow-hidden">
          <div className="overflow-x-auto">
            <table className="tbl">
              <thead>
                <tr>
                  <Th>作业</Th>
                  <Th>状态</Th>
                  <Th num>GPU</Th>
                  <Th>近 10 分钟利用率</Th>
                  <Th>项目 / 空间</Th>
                  <Th>提交者</Th>
                  <Th>创建时间</Th>
                  <Th><span className="sr-only">操作</span></Th>
                </tr>
              </thead>
              <tbody>
                {shown.map((j: any) => {
                  const id = String(field(j, ["jobId", "id"], ""));
                  const run = st(j) === "RUNNING";
                  const active = run || /WAIT|QUEU|PEND|CREAT|START|INIT/.test(st(j));
                  const mine = canManageJob(j);
                  const selfOwned = !!j.qdOwner && j.qdOwner.toLowerCase() === (me.username || "").toLowerCase();
                  const pts = util[id];
                  return (
                    <tr key={id} className="group" data-job={id}>
                      <td className="px-4 py-3 max-w-[340px]">
                        <button className="flex items-center gap-3 min-w-0 text-left" onClick={() => setMon({ job: monitorJob(j) })}>
                          <StatusDot status={st(j)} />
                          <span className="min-w-0">
                            <span className="block font-medium truncate group-hover:text-brand transition-colors">{field(j, ["jobName", "name"])}</span>
                            <span className="block text-[11px] text-ink-faint font-mono">#{id}</span>
                          </span>
                        </button>
                      </td>
                      <td className="px-4 py-3"><Badge text={st(j) || field(j, ["statusName"])} /></td>
                      <td className="px-4 py-3 text-right tabular-nums">{gpuOf(j) ? <span className="font-medium">{gpuOf(j)}</span> : <span className="text-ink-faint">—</span>}</td>
                      <td className="px-4 py-3">
                        {run ? (pts ? <span className="flex items-center gap-2"><Spark points={pts} w={88} h={22} color={gpuOf(j) ? "#4F46E5" : "#0F766E"} /><span className="text-[11px] text-ink-soft tabular-nums w-8">{pts.length ? Math.round(pts[pts.length - 1]) + "%" : "—"}</span></span>
                          : <Loader2 size={12} className="animate-spin text-ink-faint" />) : <span className="text-ink-faint">—</span>}
                      </td>
                      <td className="px-4 py-3 max-w-[200px]">
                        <span className="block truncate">{field(j, ["projectName", "projectId"], "—")}</span>
                        <span className="block text-[11px] text-ink-faint truncate">{j.spaceName || "—"}</span>
                      </td>
                      <td className="px-4 py-3 whitespace-nowrap">{j.qdOwner ? (selfOwned ? <span className="tag tag-brand">我提交的</span> : j.qdOwner) : <span className="text-ink-faint">—</span>}</td>
                      <td className="px-4 py-3 whitespace-nowrap text-ink-soft tabular-nums text-[12px]">{fmtTime(j.createTime)}</td>
                      <td className="px-3 py-3 whitespace-nowrap text-right">
                        <span className="inline-flex items-center gap-1">
                          {run && (mine
                            ? <button className="btn btn-mini" title="打开网页终端" onClick={() => setMon({ job: monitorJob(j), tab: "terminal" })}><TermIcon size={13} />终端</button>
                            : <span title={NOT_MINE} className="cursor-not-allowed"><button className="btn btn-mini pointer-events-none" disabled><TermIcon size={13} />终端</button></span>)}
                          <button className="btn btn-mini" title="按这个作业的配置新建一个" onClick={() => cloneJob(j)}><CopyPlus size={13} />复制</button>
                          {mine
                            ? (active
                              ? <button className="btn btn-mini btn-danger" title="停止这个作业" onClick={() => act("job-cancel", j)}><Square size={11} />取消</button>
                              : <button className="btn btn-mini btn-danger" title="删除作业记录" onClick={() => act("job-delete", j)}><Trash2 size={12} />删除</button>)
                            : <span title={NOT_MINE} className="cursor-not-allowed"><button className="btn btn-mini pointer-events-none" disabled>{active ? <><Square size={11} />取消</> : <><Trash2 size={12} />删除</>}</button></span>}
                        </span>
                      </td>
                    </tr>
                  );
                })}
                {!shown.length && (
                  <tr><td colSpan={8}>
                    {q || filter !== "all"
                      ? <Empty>没有符合条件的作业，换个状态或清空搜索。</Empty>
                      : <Empty action={<button className="btn btn-pri btn-mini" onClick={() => setCreate(true)}><Plus size={13} />提交作业</button>}>还没有作业</Empty>}
                  </td></tr>
                )}
              </tbody>
            </table>
          </div>
        </div>
      )}
      {data && (
        <Pager page={page} size={PAGE_SIZE} total={total} count={shown.length} busy={refreshing} onPage={setPage}
          note={<>
            <span>点作业名查看监控、日志和终端。</span>
            {(data.skipped > 0 || data.failed.length > 0) && <span className="text-warn">{data.skipped ? `${data.skipped} 个项目读取失败未计入` : ""}{data.failed.length ? `空间 ${data.failed.join("、")} 读取失败` : ""}</span>}
          </>} />
      )}

      <Monitor job={mon?.job || null} initialTab={mon?.tab} onClose={() => setMon(null)} />
      {create && <CreateForm kind="job" state={state} open initial={preset} onClose={() => { setCreate(false); setPreset(null); }} onDone={refresh} />}
    </>
  );
}
