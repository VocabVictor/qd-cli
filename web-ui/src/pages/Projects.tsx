import { useState } from "react";
import { RefreshCw, Loader2 } from "lucide-react";
import { AppState, pcall, eachSpace, spacesOf, field } from "../lib/api";
import { useCached } from "../lib/cache";
import { useList } from "../lib/table";
import { PageBar, Spinner, Table, Th, Td, SortTh, Toolbar, Badge, Card } from "../components/ui";
import { Drawer, Json } from "../components/Drawer";

export function Projects({ state }: { state: AppState }) {
  const spaces = spacesOf(state);
  const [scope, setScope] = useState("mine");
  const [detail, setDetail] = useState<any>(null);
  const { data: list, loading, refreshing, error, refresh } = useCached<any[]>(
    `projects:${scope}`, async () => {
      const query: any = { pageNum: 1, pageSize: 500 };
      if (scope !== "all") query.accessType = ({ mine: 1, shared: 2, public: 3 } as any)[scope];
      const per = await eachSpace(state, (sp) => pcall("GET", "core", "/project/list", query, sp.id));
      return per.flatMap(({ space, data }) => ((data.data?.projectList || data.data?.list) || []).map((p: any) => ({ ...p, _sp: space })));
    }, [scope]
  );

  const { view, search, setSearch, filters, setFilter, sortKey, sortDir, toggleSort } = useList(list || [], {
    searchText: (p) => [field(p, ["projectName", "name"], ""), field(p, ["projectId", "id"], ""), field(p, ["ownerDisplayName", "createDisplayName"], ""), p._sp?.name].join(" "),
    sorts: { name: (p) => field(p, ["projectName", "name"], ""), jobs: (p) => Number(field(p, ["jobCount"], 0)) || 0 },
    initialSort: { key: "name", dir: "asc" },
  });

  const openDetail = async (id: string, sp: string) => {
    setDetail({ loading: true, id });
    try { const d = await pcall("GET", "core", "/project/detail/" + id, {}, sp); setDetail({ id, data: d }); } catch (e: any) { setDetail({ id, error: e.message }); }
  };

  return (
    <>
      <PageBar title="项目" right={
        <div className="flex items-center gap-2">
          <select className="field w-28" value={scope} onChange={(e) => setScope(e.target.value)}>{["mine", "shared", "public", "all"].map((s) => <option key={s}>{s}</option>)}</select>
          <button className="btn" onClick={refresh}>{refreshing ? <Loader2 size={14} className="animate-spin" /> : <RefreshCw size={14} />}刷新</button>
        </div>} />
      <Toolbar search={search} onSearch={setSearch} placeholder="搜索 项目名 / ID / 负责人">
        {spaces.length > 1 && (
          <select className="field w-32" value={filters.space || ""} onChange={(e) => setFilter("space", e.target.value, (p, v) => p._sp?.id === v)}>
            <option value="">全部空间</option>{spaces.map((s) => <option key={s.id} value={s.id}>{s.name}</option>)}
          </select>
        )}
      </Toolbar>
      {error && !list && <Card className="p-4 text-sm mb-4">加载失败：{error}</Card>}
      {loading ? <Spinner /> :
        <Table head={<>
          <SortTh sortKey="name" active={sortKey === "name"} dir={sortDir} onSort={toggleSort}>项目</SortTh>
          <Th>ID</Th><Th>空间</Th><Th>开发环境</Th>
          <SortTh sortKey="jobs" active={sortKey === "jobs"} dir={sortDir} onSort={toggleSort} num>任务数</SortTh>
          <Th>负责人</Th><Th></Th>
        </>}>
          {view.length ? view.map((p, i) => {
            const id = String(field(p, ["projectId", "id"], ""));
            return (
              <tr key={i} className="hover:bg-canvas/60">
                <Td clip><b>{field(p, ["projectName", "name"])}</b></Td>
                <Td><span className="text-[11px] text-ink-faint">{id}</span></Td>
                <Td>{p._sp.name}</Td>
                <Td>{field(p, ["jobenvStatus"], "") ? <Badge text={p.jobenvStatus} /> : <span className="text-ink-faint">未创建</span>}</Td>
                <Td num>{field(p, ["jobCount"])}</Td>
                <Td clip>{field(p, ["ownerDisplayName", "createDisplayName"])}</Td>
                <Td><button className="btn btn-mini" onClick={() => openDetail(id, p._sp.id)}>详情</button></Td>
              </tr>
            );
          }) : <tr><Td className="text-ink-faint">没有匹配的项目</Td></tr>}
        </Table>}
      <Drawer open={!!detail} onClose={() => setDetail(null)} title={detail && `项目 ${detail.id}`}>
        {detail?.loading ? <Spinner /> : detail?.error ? <Card className="p-4 text-sm">加载失败：{detail.error}</Card> : detail && <Json value={detail.data} />}
      </Drawer>
    </>
  );
}
