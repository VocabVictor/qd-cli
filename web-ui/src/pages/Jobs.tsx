import { useState } from "react";
import { RefreshCw, Loader2 } from "lucide-react";
import { api, AppState, fmtTime, field, spacesOf } from "../lib/api";
import { useCached } from "../lib/cache";
import { useList } from "../lib/table";
import { PageBar, Spinner, Table, Th, Td, SortTh, Toolbar, Badge, Card, useToast } from "../components/ui";
import { Drawer, Json } from "../components/Drawer";
import { Monitor } from "../components/Monitor";
import { CreateForm } from "../components/CreateForm";
import { Plus } from "lucide-react";
import { Activity } from "lucide-react";

const CHIPS: [string, string, (s: string) => boolean][] = [
  ["all", "全部", () => true],
  ["running", "运行中", (s) => s === "RUNNING"],
  ["waiting", "等待中", (s) => /WAIT|QUEU|PEND|CREAT|START|INIT/.test(s)],
  ["failed", "失败", (s) => /FAIL|ERROR/.test(s)],
  ["succeeded", "成功", (s) => /SUCCEED|SUCCESS/.test(s)],
  ["stopped", "已停止", (s) => /STOP|CANCEL|DELET/.test(s)],
];
const st = (j: any) => String(field(j, ["status"], "")).toUpperCase();

export function Jobs({ state }: { state: AppState }) {
  const toast = useToast();
  const spaces = spacesOf(state);
  const [scope, setScope] = useState("mine");
  const [chip, setChip] = useState("all");
  const [detail, setDetail] = useState<any>(null);
  const [mon, setMon] = useState<any>(null);
  const [create, setCreate] = useState(false);

  const { data: all, loading, refreshing, error, refresh } = useCached<any[]>(
    `jobs:${scope}`, async () => { const d = await api.jobs(scope, 500); return (d.data?.jobList) || []; }, [scope]
  );

  const { view, search, setSearch, filters, setFilter, sortKey, sortDir, toggleSort } = useList(all || [], {
    searchText: (j) => [field(j, ["jobName", "name"], ""), field(j, ["jobId", "id"], ""), field(j, ["projectName"], ""), field(j, ["statusName"], ""), st(j), j.spaceName || ""].join(" "),
    sorts: {
      name: (j) => field(j, ["jobName", "name"], ""), status: (j) => st(j),
      gpu: (j) => Number(field(j, ["statGpu", "gpuCount", "GPU"], 0)) || 0,
      created: (j) => Number(new Date(j.createTime).getTime()) || 0,
    },
    initialSort: { key: "created", dir: "desc" },
  });
  const chipMatch = CHIPS.find((c) => c[0] === chip)![2];
  const shown = view.filter((j) => chipMatch(st(j)));

  const act = async (action: string, id: string, sp: string) => {
    try { const r = await api.bulk(action, [id], sp); const bad = r.find((x: any) => !x.ok); bad ? toast(bad.error, true) : toast("已" + (action.includes("cancel") ? "取消" : "删除") + " " + id); refresh(); }
    catch (e: any) { toast(e.message, true); }
  };
  const openDetail = async (id: string, sp: string) => {
    setDetail({ loading: true, id });
    try { const d = await api.job(id, sp); setDetail({ id, data: d }); } catch (e: any) { setDetail({ id, error: e.message }); }
  };

  return (
    <>
      <PageBar title="作业" right={
        <div className="flex items-center gap-2">
          <button className="btn btn-pri" onClick={() => setCreate(true)}><Plus size={14} />提交作业</button>
          <select className="field w-28" value={scope} onChange={(e) => setScope(e.target.value)}>{["mine", "shared", "public", "all"].map((s) => <option key={s}>{s}</option>)}</select>
          <button className="btn" onClick={refresh}>{refreshing ? <Loader2 size={14} className="animate-spin" /> : <RefreshCw size={14} />}刷新</button>
        </div>} />
      <Toolbar search={search} onSearch={setSearch} placeholder="搜索 名称 / ID / 项目 / 状态">
        {spaces.length > 1 && (
          <select className="field w-32" value={filters.space || ""} onChange={(e) => setFilter("space", e.target.value, (j, v) => j.spaceId === v)}>
            <option value="">全部空间</option>{spaces.map((s) => <option key={s.id} value={s.id}>{s.name}</option>)}
          </select>
        )}
      </Toolbar>
      <div className="flex items-center gap-1.5 mb-3 flex-wrap">
        {CHIPS.map(([k, label]) => {
          const n = view.filter((j) => (CHIPS.find((c) => c[0] === k)![2])(st(j))).length;
          return <button key={k} onClick={() => setChip(k)} className={"chip " + (chip === k ? "chip-on" : "")}>{label}<span className="ml-1 text-ink-faint">{n}</span></button>;
        })}
      </div>
      {error && !all && <Card className="p-4 text-sm mb-4">加载失败：{error}</Card>}
      {loading ? <Spinner /> : <>
        <Table head={<>
          <SortTh sortKey="name" active={sortKey === "name"} dir={sortDir} onSort={toggleSort}>作业</SortTh>
          <Th>ID</Th><Th>空间</Th><Th>项目</Th>
          <SortTh sortKey="status" active={sortKey === "status"} dir={sortDir} onSort={toggleSort}>状态</SortTh>
          <SortTh sortKey="gpu" active={sortKey === "gpu"} dir={sortDir} onSort={toggleSort} num>GPU</SortTh>
          <SortTh sortKey="created" active={sortKey === "created"} dir={sortDir} onSort={toggleSort}>创建时间</SortTh>
          <Th></Th>
        </>}>
          {shown.length ? shown.map((j, i) => {
            const id = String(field(j, ["jobId", "id"], "")), sp = j.spaceId || "";
            return (
              <tr key={i} className="hover:bg-canvas/60">
                <Td clip><a className="text-brand-600 cursor-pointer hover:underline" onClick={() => openDetail(id, sp)}>{field(j, ["jobName", "name"])}</a></Td>
                <Td><span className="text-[11px] text-ink-faint">{id}</span></Td>
                <Td>{j.spaceName || "-"}</Td>
                <Td clip>{field(j, ["projectName", "projectId"])}</Td>
                <Td><Badge text={field(j, ["statusName", "status"])} /></Td>
                <Td num>{field(j, ["statGpu", "gpuCount", "GPU"])}</Td>
                <Td>{fmtTime(j.createTime)}</Td>
                <Td className="whitespace-nowrap">
                  {st(j) === "RUNNING" && <button className="btn btn-mini mr-1 text-mint border-mint/30" onClick={() => setMon({ id, name: field(j, ["jobName", "name"]), space: sp, kind: "job" })}><Activity size={12} />监控</button>}
                  <button className="btn btn-mini mr-1" onClick={() => openDetail(id, sp)}>详情</button>
                  <button className="btn btn-mini mr-1" onClick={() => act("job-cancel", id, sp)}>取消</button>
                  <button className="btn btn-mini btn-danger" onClick={() => act("job-delete", id, sp)}>删除</button>
                </Td>
              </tr>
            );
          }) : <tr><Td className="text-ink-faint">没有匹配的作业</Td></tr>}
        </Table>
        <p className="text-xs text-ink-faint mt-3">显示 {shown.length} / {(all || []).length} 个（scope={scope}）。点表头排序。</p>
      </>}
      <CreateForm kind="job" state={state} open={create} onClose={() => setCreate(false)} onDone={refresh} />
      <Monitor job={mon} onClose={() => setMon(null)} baseUrl={state.baseUrl} />
      <Drawer open={!!detail} onClose={() => setDetail(null)} title={detail && `作业 ${detail.id}`}>
        {detail?.loading ? <Spinner /> : detail?.error ? <Card className="p-4 text-sm">加载失败：{detail.error}</Card> : detail && <Json value={detail.data} />}
      </Drawer>
    </>
  );
}
