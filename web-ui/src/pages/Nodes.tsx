import { useState, useEffect } from "react";
import { RefreshCw, Loader2, Server, Cpu, MemoryStick, Gauge } from "lucide-react";
import { api, AppState, fmtMiB, fmtCpu, splitList, field } from "../lib/api";
import { useCached } from "../lib/cache";
import { PageBar, Spinner, Table, Th, Td, SortTh, Toolbar, Stat, Pager, Empty } from "../components/ui";

const PAGE_SIZE = 20;

/** 节点是集群级资源：筛选、排序、分页、汇总都交给平台原生接口。 */
export function Nodes({ state: _state }: { state: AppState }) {
  const [page, setPage] = useState(1);
  const [group, setGroup] = useState("");
  const [model, setModel] = useState("");
  const [freeOnly, setFreeOnly] = useState(false);
  const [sort, setSort] = useState("gpu");
  const [dir, setDir] = useState<"asc" | "desc">("desc");
  useEffect(() => setPage(1), [group, model, freeOnly, sort, dir]);

  const { data: facets } = useCached("node-facets", () => api.nodeFacets());
  const { data: groups } = useCached<any[]>("node-groups", async () => (await api.idle()).groups || []);
  const { data, loading, refreshing, error, refresh } = useCached<any>(
    `nodes:${page}:${group}:${model}:${freeOnly}:${sort}:${dir}`,
    () => api.nodes({ page, size: PAGE_SIZE, group, model, minGpu: freeOnly ? 1 : 0, sort, dir }),
    [page, group, model, freeOnly, sort, dir]);

  const nodes: any[] = data?.nodes || [];
  const sum = data?.summary || {};
  const total = Number(sum.total) || 0;
  const onSort = (k: string) => { if (k === sort) setDir((d) => (d === "asc" ? "desc" : "asc")); else { setSort(k); setDir("desc"); } };
  const groupOptions = (groups || []).filter((g: any, i: number, all: any[]) => all.findIndex((x) => x.rsgroupId === g.rsgroupId) === i);

  return (
    <>
      <PageBar title="节点" hint={data ? `${total} 个节点符合条件` : undefined}
        right={<button className="btn" onClick={refresh}>{refreshing ? <Loader2 size={14} className="animate-spin" /> : <RefreshCw size={14} />}刷新</button>} />
      <Toolbar>
        <select className="field w-44" aria-label="资源组" value={group} onChange={(e) => setGroup(e.target.value)}>
          <option value="">全部资源组</option>
          {groupOptions.map((g: any) => <option key={g.rsgroupId} value={g.rsgroupId}>{g.rsgroupName || g.rsgroupId}</option>)}
        </select>
        <select className="field w-48" aria-label="GPU 型号" value={model} onChange={(e) => setModel(e.target.value)}>
          <option value="">全部 GPU 型号</option>
          {(facets?.models || []).map((m) => <option key={m} value={m}>{m}</option>)}
        </select>
        <label className={"inline-flex items-center gap-2 h-9 px-3 rounded-ctl border text-[13px] cursor-pointer select-none transition-colors shadow-ctl " + (freeOnly ? "bg-brand-50 border-brand/30 text-brand" : "bg-white border-line text-ink-soft hover:text-ink")}>
          <input type="checkbox" className="accent-brand w-3.5 h-3.5" checked={freeOnly} onChange={(e) => setFreeOnly(e.target.checked)} />只看有空闲卡的
        </label>
      </Toolbar>

      {error && !data && <div className="card p-4 text-body mb-3">节点列表读取失败：{error}。检查平台连接后点刷新。</div>}
      {loading ? <Spinner /> : <>
        <section className="grid grid-cols-2 xl:grid-cols-4 gap-4 mb-5" aria-label="筛选范围内的资源汇总">
          <Stat label="节点" icon={Server} tone="slate" sub="当前筛选范围">{total}</Stat>
          <Stat label="物理 GPU 已用" icon={Gauge} tone="brand" sub={`共 ${Number(sum.pGpuTotal) || 0} 卡`}>
            {Number(sum.pGpuUsed) || 0}<span className="text-body font-normal text-ink-faint"> / {Number(sum.pGpuTotal) || 0}</span>
          </Stat>
          <Stat label="CPU 已用（核）" icon={Cpu} tone="teal" sub={`共 ${fmtCpu((Number(sum.cpuTotal) || 0) / 1000)} 核`}>{fmtCpu((Number(sum.cpuUsed) || 0) / 1000)}</Stat>
          <Stat label="内存已用" icon={MemoryStick} tone="sky" sub={`共 ${fmtMiB(Number(sum.memoryTotal) || 0)}`}>{fmtMiB(Number(sum.memoryUsed) || 0)}</Stat>
        </section>
        <Table head={<>
          <Th>节点</Th><Th>GPU 型号</Th>
          <SortTh sortKey="gpu" active={sort === "gpu"} dir={dir} onSort={onSort} num>空闲卡</SortTh>
          <Th num>物理卡</Th>
          <SortTh sortKey="cpu" active={sort === "cpu"} dir={dir} onSort={onSort} num>CPU 已申请</SortTh>
          <SortTh sortKey="memory" active={sort === "memory"} dir={dir} onSort={onSort} num>内存已申请</SortTh>
          <Th>资源组</Th><Th num>作业</Th>
        </>}>
          {nodes.length ? nodes.map((n) => (
            <tr key={n.name || n.ip} data-node={n.name || n.ip}>
              <Td><div className="font-medium">{field(n, ["name"])}</div><div className="text-[11px] text-ink-faint font-mono">{field(n, ["ip", "accessIp"], "")}</div></Td>
              <Td clip>{splitList(n.gpuModels).length ? <span className="tag tag-gray">{splitList(n.gpuModels).join("、")}</span> : <span className="text-ink-faint">—</span>}</Td>
              <Td num>{Number(n.gpuLeft) ? <span className="tag tag-ok tabular-nums">{n.gpuLeft}</span> : <span className="text-ink-faint">{n.gpuLeft ?? "—"}</span>}</Td>
              <Td num>{n.gpuUsed ?? "—"} / {n.gpuTotal ?? "—"}</Td>
              <Td num>{fmtCpu(n.cpuRequest)} / {fmtCpu(n.cpuTotal)}</Td>
              <Td num>{fmtMiB(n.memoryRequest)} / {fmtMiB(n.memoryTotal)}</Td>
              <Td clip className="text-ink-soft">{splitList(n.resourceGroups).join("、") || "—"}</Td>
              <Td num>{(n.jobList || []).length}</Td>
            </tr>
          )) : <tr><td colSpan={8}><Empty>没有符合条件的节点，换个资源组或型号试试。</Empty></td></tr>}
        </Table>
        <Pager page={page} size={PAGE_SIZE} total={total} count={nodes.length} busy={refreshing} onPage={setPage} />
      </>}
    </>
  );
}
