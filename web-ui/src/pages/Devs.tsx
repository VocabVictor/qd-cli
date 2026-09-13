import { useState, useMemo } from "react";
import { RefreshCw, ExternalLink, Loader2 } from "lucide-react";
import { api, AppState, pcall, qget, field, spacesOf } from "../lib/api";
import { useCached } from "../lib/cache";
import { useList } from "../lib/table";
import { PageBar, Spinner, Table, Th, Td, SortTh, Toolbar, Badge, Card, useToast } from "../components/ui";
import { Drawer, Json } from "../components/Drawer";

export function Devs({ state }: { state: AppState }) {
  const toast = useToast();
  const spaces = spacesOf(state);
  const [scope, setScope] = useState("mine");
  const [detail, setDetail] = useState<any>(null);
  const { data: list, loading, refreshing, error, refresh } = useCached<any[]>(
    `devs:${scope}`, async () => { const d = await api.devs(scope); return (d.data?.devList) || []; }, [scope]
  );

  const statuses = useMemo(() => [...new Set((list || []).map((d) => String(field(d, ["jobenvStatus"], ""))))].filter(Boolean), [list]);
  const { view, search, setSearch, filters, setFilter, sortKey, sortDir, toggleSort } = useList(list || [], {
    searchText: (d) => [field(d, ["projectName", "projectId"], ""), field(d, ["jobenvId"], ""), field(d, ["jobenvStatus"], ""), field(d, ["ownerDisplayName", "createDisplayName"], ""), d.spaceName || ""].join(" "),
    sorts: { project: (d) => field(d, ["projectName", "projectId"], ""), id: (d) => Number(field(d, ["jobenvId"], 0)) || 0, status: (d) => field(d, ["jobenvStatus"], "") },
    initialSort: { key: "id", dir: "desc" },
  });

  const act = async (action: string, id: string, sp: string) => {
    try { const r = await api.bulk(action, [id], sp); const bad = r.find((x: any) => !x.ok); bad ? toast(bad.error, true) : toast("已" + (action.includes("start") ? "启动" : "停止") + " " + id); refresh(); }
    catch (e: any) { toast(e.message, true); }
  };
  const term = async (id: string, sp: string) => {
    try { const r = await qget(`/api/terminal-url?kind=dev&id=${encodeURIComponent(id)}&instance=0${sp ? "&space=" + sp : ""}`); window.open(r.terminalUrl, "_blank"); } catch (e: any) { toast(e.message, true); }
  };
  const openDetail = async (id: string, sp: string) => {
    setDetail({ loading: true, id });
    try { const d = await pcall("GET", "core", "/jobenv/refresh/" + id, {}, sp); setDetail({ id, data: d }); } catch (e: any) { setDetail({ id, error: e.message }); }
  };

  return (
    <>
      <PageBar title="开发环境" right={
        <div className="flex items-center gap-2">
          <select className="field w-28" value={scope} onChange={(e) => setScope(e.target.value)}>{["mine", "shared", "public", "all"].map((s) => <option key={s}>{s}</option>)}</select>
          <button className="btn" onClick={refresh}>{refreshing ? <Loader2 size={14} className="animate-spin" /> : <RefreshCw size={14} />}刷新</button>
        </div>} />
      <Toolbar search={search} onSearch={setSearch} placeholder="搜索 项目 / ID / 负责人 / 状态">
        {spaces.length > 1 && (
          <select className="field w-32" value={filters.space || ""} onChange={(e) => setFilter("space", e.target.value, (d, v) => d.spaceId === v)}>
            <option value="">全部空间</option>{spaces.map((s) => <option key={s.id} value={s.id}>{s.name}</option>)}
          </select>
        )}
        {statuses.length > 1 && (
          <select className="field w-32" value={filters.status || ""} onChange={(e) => setFilter("status", e.target.value, (d, v) => String(field(d, ["jobenvStatus"], "")) === v)}>
            <option value="">全部状态</option>{statuses.map((s) => <option key={s} value={s}>{s}</option>)}
          </select>
        )}
      </Toolbar>
      {error && !list && <Card className="p-4 text-sm mb-4">加载失败：{error}</Card>}
      {loading ? <Spinner /> :
        <Table head={<>
          <SortTh sortKey="project" active={sortKey === "project"} dir={sortDir} onSort={toggleSort}>项目</SortTh>
          <SortTh sortKey="id" active={sortKey === "id"} dir={sortDir} onSort={toggleSort}>环境 ID</SortTh>
          <Th>空间</Th>
          <SortTh sortKey="status" active={sortKey === "status"} dir={sortDir} onSort={toggleSort}>状态</SortTh>
          <Th>负责人</Th><Th>操作</Th>
        </>}>
          {view.length ? view.map((d, i) => {
            const id = String(field(d, ["jobenvId"], "")), sp = d.spaceId || "";
            return (
              <tr key={i} className="hover:bg-canvas/60">
                <Td clip><b>{field(d, ["projectName", "projectId"])}</b></Td>
                <Td><span className="text-[11px] text-ink-faint">{id}</span></Td>
                <Td>{d.spaceName || "-"}</Td>
                <Td><Badge text={field(d, ["jobenvStatus"], "-")} /></Td>
                <Td clip>{field(d, ["ownerDisplayName", "createDisplayName"])}</Td>
                <Td className="whitespace-nowrap">
                  <button className="btn btn-mini mr-1" onClick={() => act("dev-start", id, sp)}>启动</button>
                  <button className="btn btn-mini mr-1" onClick={() => act("dev-stop", id, sp)}>停止</button>
                  <button className="btn btn-mini mr-1" onClick={() => term(id, sp)}><ExternalLink size={12} />终端</button>
                  <button className="btn btn-mini" onClick={() => openDetail(id, sp)}>详情</button>
                </Td>
              </tr>
            );
          }) : <tr><Td className="text-ink-faint">没有匹配的开发环境</Td></tr>}
        </Table>}
      <Drawer open={!!detail} onClose={() => setDetail(null)} title={detail && `开发环境 ${detail.id}`}>
        {detail?.loading ? <Spinner /> : detail?.error ? <Card className="p-4 text-sm">加载失败：{detail.error}</Card> : detail && <Json value={detail.data} />}
      </Drawer>
    </>
  );
}
