import { useState } from "react";
import { RefreshCw, Loader2 } from "lucide-react";
import { AppState, pcall, eachSpace, spacesOf, field } from "../lib/api";
import { useCached } from "../lib/cache";
import { useList } from "../lib/table";
import { PageBar, Spinner, Table, Th, Td, SortTh, Toolbar, Card } from "../components/ui";
import { Drawer } from "../components/Drawer";

const ACCESS: Record<number, string> = { 1: "私有", 2: "空间共享", 3: "公开" };
const VIS: [string, string, (a: number) => boolean][] = [
  ["all", "全部", () => true], ["3", "公开", (a) => a === 3], ["2", "空间共享", (a) => a === 2], ["1", "私有", (a) => a === 1],
];

export function Images({ state }: { state: AppState }) {
  const spaces = spacesOf(state);
  const [chip, setChip] = useState("all");
  const [repo, setRepo] = useState<any>(null);

  const { data: cache, loading, refreshing, error, refresh } = useCached<any[]>(
    "images", async () => {
      const per = await eachSpace(state, (sp) => pcall("GET", "core", "/imageRepository/list", { spaceId: sp.id, source: "current", keyWords: "", pageNum: 1, pageSize: 500 }, sp.id));
      return per.flatMap(({ space, data }) => ((data.data?.imageList) || []).map((r: any) => ({ ...r, _sp: space })));
    }
  );

  const { view, search, setSearch, filters, setFilter, sortKey, sortDir, toggleSort } = useList(cache || [], {
    searchText: (r) => [field(r, ["originalName"], ""), field(r, ["repositoryName"], ""), field(r, ["displayName", "userName"], ""), ACCESS[r.accessType] || "", r._sp?.name].join(" "),
    sorts: {
      name: (r) => field(r, ["originalName", "repositoryName"], ""), num: (r) => Number(field(r, ["imageNum"], 0)) || 0,
      updated: (r) => Number(new Date(field(r, ["updateTime"], 0)).getTime()) || 0,
    },
    initialSort: { key: "updated", dir: "desc" },
  });
  const vis = VIS.find((v) => v[0] === chip)![2];
  const shown = view.filter((r) => vis(Number(r.accessType)));

  const openRepo = async (id: number, sp: string, name: string) => {
    setRepo({ loading: true, name });
    try { const d = await pcall("GET", "core", "/imageRepository/detail/" + id, {}, sp); setRepo({ name, data: d.data || {} }); } catch (e: any) { setRepo({ name, error: e.message }); }
  };

  return (
    <>
      <PageBar title="镜像" right={<button className="btn" onClick={refresh}>{refreshing ? <Loader2 size={14} className="animate-spin" /> : <RefreshCw size={14} />}刷新</button>} />
      <Toolbar search={search} onSearch={setSearch} placeholder="搜索 仓库名 / 路径 / 创建人">
        {spaces.length > 1 && (
          <select className="field w-32" value={filters.space || ""} onChange={(e) => setFilter("space", e.target.value, (r, v) => r._sp?.id === v)}>
            <option value="">全部空间</option>{spaces.map((s) => <option key={s.id} value={s.id}>{s.name}</option>)}
          </select>
        )}
      </Toolbar>
      <div className="flex items-center gap-1.5 mb-3 flex-wrap">
        {VIS.map(([k, label]) => {
          const n = view.filter((r) => (VIS.find((c) => c[0] === k)![2])(Number(r.accessType))).length;
          return <button key={k} onClick={() => setChip(k)} className={"chip " + (chip === k ? "chip-on" : "")}>{label}<span className="ml-1 text-ink-faint">{n}</span></button>;
        })}
      </div>
      {error && !cache && <Card className="p-4 text-sm mb-4">加载失败：{error}</Card>}
      {loading ? <Spinner /> : <>
        <Table head={<>
          <SortTh sortKey="name" active={sortKey === "name"} dir={sortDir} onSort={toggleSort}>镜像仓库</SortTh>
          <Th>空间</Th>
          <SortTh sortKey="num" active={sortKey === "num"} dir={sortDir} onSort={toggleSort} num>镜像数</SortTh>
          <Th>可见性</Th><Th>创建人</Th>
          <SortTh sortKey="updated" active={sortKey === "updated"} dir={sortDir} onSort={toggleSort}>更新时间</SortTh>
        </>}>
          {shown.length ? shown.map((r, i) => (
            <tr key={i} className="hover:bg-canvas/60 cursor-pointer" onClick={() => openRepo(Number(r.imageRepositoryId) || 0, r._sp.id, field(r, ["originalName", "repositoryName"]))}>
              <Td clip><div className="font-semibold text-brand-600">{field(r, ["originalName", "repositoryName"])}</div><div className="text-[11px] text-ink-faint">{field(r, ["repositoryName"], "")}</div></Td>
              <Td>{r._sp.name}</Td>
              <Td num>{field(r, ["imageNum"])}</Td>
              <Td>{ACCESS[r.accessType] || r.accessType || "-"}</Td>
              <Td>{field(r, ["displayName", "userName"])}</Td>
              <Td>{field(r, ["updateTime"])}</Td>
            </tr>
          )) : <tr><Td className="text-ink-faint">没有匹配的镜像仓库</Td></tr>}
        </Table>
        <p className="text-xs text-ink-faint mt-3">显示 {shown.length} / {(cache || []).length} 个仓库。点行看镜像 tag，点表头排序。</p>
      </>}
      <Drawer open={!!repo} onClose={() => setRepo(null)} title={repo?.name} sub={repo?.data && `${repo.data.repositoryName || ""} · ${repo.data.displayName || ""}`}>
        {repo?.loading ? <Spinner /> : repo?.error ? <Card className="p-4 text-sm">加载失败：{repo.error}</Card> : repo?.data && (
          <Table head={<><Th>镜像 ID</Th><Th>Tag</Th><Th num>大小</Th><Th>更新时间</Th></>}>
            {(repo.data.imageList || []).length ? repo.data.imageList.map((im: any, i: number) => (
              <tr key={i}><Td><span className="text-[11px] text-ink-faint">{field(im, ["imageId"])}</span></Td><Td><b>{field(im, ["imageTag"])}</b></Td>
                <Td num>{im.imageSize ? (Number(im.imageSize) / 1073741824).toFixed(2) + " GiB" : "-"}</Td><Td>{field(im, ["updateTime"])}</Td></tr>
            )) : <tr><Td className="text-ink-faint">仓库为空</Td></tr>}
          </Table>
        )}
      </Drawer>
    </>
  );
}
