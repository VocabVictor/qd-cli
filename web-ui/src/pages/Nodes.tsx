import { useState, useMemo } from "react";
import { RefreshCw, Loader2 } from "lucide-react";
import { AppState, pcall, eachSpace, spacesOf, fmtMiB, fmtCpu, asList, field } from "../lib/api";
import { useCached } from "../lib/cache";
import { useList } from "../lib/table";
import { PageBar, Tile, Spinner, Table, Th, Td, SortTh, Toolbar, Card } from "../components/ui";

export function Nodes({ state }: { state: AppState }) {
  const spaces = spacesOf(state);
  const { data: nodes, loading, refreshing, refresh } = useCached<any[]>(
    "nodes", async () => {
      const query: any = { pageNum: 1, pageSize: 500, sortField: "left_gpu_count", sortOrder: "desc" };
      const perSpace = await eachSpace(state, (sp) => pcall("GET", "core", "/jobQueue/node/job/list", query, sp.id));
      const seen = new Set<string>(); const out: any[] = [];
      for (const { space, data } of perSpace)
        for (const n of (data.data?.nodes || [])) {
          const key = n.name || n.ip; if (seen.has(key)) continue; seen.add(key); out.push({ ...n, _sp: space });
        }
      return out;
    }
  );

  const models = useMemo(() => [...new Set((nodes || []).flatMap((n) => asList(n.gpuModels)))].filter(Boolean), [nodes]);
  const { view, search, setSearch, filters, setFilter, sortKey, sortDir, toggleSort } = useList(nodes || [], {
    searchText: (n) => [field(n, ["name"], ""), field(n, ["ip", "accessIp"], ""), asList(n.gpuModels).join(" "), asList(n.resourceGroups).join(" "), n._sp?.name].join(" "),
    sorts: {
      name: (n) => field(n, ["name"], ""), gpuLeft: (n) => Number(n.gpuLeft) || 0, gpuUsed: (n) => Number(n.gpuUsed) || 0,
      cpu: (n) => Number(n.cpuRequest) || 0, mem: (n) => Number(n.memoryRequest) || 0, jobs: (n) => (n.jobList || []).length,
    },
    initialSort: { key: "gpuLeft", dir: "desc" },
  });

  const sum = (k: string) => view.reduce((a, n) => a + (Number(n[k]) || 0), 0);
  const sh = { active: (k: string) => sortKey === k, dir: sortDir, onSort: toggleSort };

  return (
    <>
      <PageBar title="节点" right={<button className="btn" onClick={refresh}>{refreshing ? <Loader2 size={14} className="animate-spin" /> : <RefreshCw size={14} />}刷新</button>} />
      <Toolbar search={search} onSearch={setSearch} placeholder="搜索 节点名 / IP / 型号 / 资源组">
        {spaces.length > 1 && (
          <select className="field w-32" value={filters.space || ""} onChange={(e) => setFilter("space", e.target.value, (n, v) => n._sp?.id === v)}>
            <option value="">全部空间</option>{spaces.map((s) => <option key={s.id} value={s.id}>{s.name}</option>)}
          </select>
        )}
        <select className="field w-44" value={filters.model || ""} onChange={(e) => setFilter("model", e.target.value, (n, v) => asList(n.gpuModels).includes(v))}>
          <option value="">全部 GPU 型号</option>{models.map((m) => <option key={m} value={m}>{m}</option>)}
        </select>
        <label className="flex items-center gap-1.5 text-sm text-ink-soft ml-1 cursor-pointer">
          <input type="checkbox" checked={filters.free === "1"} onChange={(e) => setFilter("free", e.target.checked ? "1" : "", (n) => (Number(n.gpuLeft) || 0) > 0)} />仅有空闲卡
        </label>
      </Toolbar>
      {loading ? <Spinner /> : <>
        <div className="grid grid-cols-2 md:grid-cols-4 gap-3 mb-4">
          <Tile label="剩余整卡（筛选内）" value={sum("gpuLeft")} unit="张" tone="mint" />
          <Tile label="物理 GPU 已用/总量" value={sum("gpuUsed")} unit={`/ ${sum("gpuTotal")}`} />
          <Tile label="CPU 已用/总量（核）" value={Math.round(sum("cpuRequest"))} unit={`/ ${Math.round(sum("cpuTotal"))}`} tone="amber" />
          <Tile label="节点数（筛选内）" value={view.length} tone="rose" />
        </div>
        <Table head={<>
          <SortTh sortKey="name" active={sh.active("name")} dir={sortDir} onSort={toggleSort}>节点</SortTh>
          <Th>空间</Th><Th>GPU 型号</Th>
          <SortTh sortKey="gpuLeft" active={sh.active("gpuLeft")} dir={sortDir} onSort={toggleSort} num>剩余整卡</SortTh>
          <SortTh sortKey="gpuUsed" active={sh.active("gpuUsed")} dir={sortDir} onSort={toggleSort} num>物理卡</SortTh>
          <SortTh sortKey="cpu" active={sh.active("cpu")} dir={sortDir} onSort={toggleSort} num>CPU</SortTh>
          <SortTh sortKey="mem" active={sh.active("mem")} dir={sortDir} onSort={toggleSort} num>内存</SortTh>
          <Th>资源组</Th>
          <SortTh sortKey="jobs" active={sh.active("jobs")} dir={sortDir} onSort={toggleSort} num>任务</SortTh>
        </>}>
          {view.length ? view.map((n, i) => (
            <tr key={i} className="hover:bg-canvas/60">
              <Td><div className="font-semibold">{field(n, ["name"])}</div><div className="text-[11px] text-ink-faint">{field(n, ["ip", "accessIp"], "")}</div></Td>
              <Td>{n._sp.name}</Td>
              <Td clip>{asList(n.gpuModels).join(", ") || "-"}</Td>
              <Td num><b>{n.gpuLeft ?? "-"}</b></Td>
              <Td num>{n.gpuUsed ?? "-"} / {n.gpuTotal ?? "-"}</Td>
              <Td num>{fmtCpu(n.cpuRequest)} / {fmtCpu(n.cpuTotal)}</Td>
              <Td num>{fmtMiB(n.memoryRequest)} / {fmtMiB(n.memoryTotal)}</Td>
              <Td clip>{asList(n.resourceGroups).join(", ") || "-"}</Td>
              <Td num>{(n.jobList || []).length}</Td>
            </tr>
          )) : <tr><Td className="text-ink-faint">没有匹配的节点</Td></tr>}
        </Table>
        <p className="text-xs text-ink-faint mt-3">显示 {view.length} / {(nodes || []).length} 个节点。点表头排序，搜索空格分词。</p>
      </>}
    </>
  );
}
