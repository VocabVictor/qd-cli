import { useState, useEffect } from "react";
import { RefreshCw, Loader2 } from "lucide-react";
import { api, AppState, pcall, spacesOf, field, isAdmin } from "../lib/api";
import { useCached } from "../lib/cache";
import { PageBar, Spinner, Table, Th, Td, Toolbar, Pager, Chips, Empty } from "../components/ui";
import { Drawer } from "../components/Drawer";

const ACCESS: Record<number, string> = { 1: "私有", 2: "空间共享", 3: "公开" };
const ACCESS_TAG: Record<number, string> = { 1: "tag-gray", 2: "tag-brand", 3: "tag-ok" };
// 平台原生的 source 取值：current 全部可见，public 公开，shared 空间共享，mine 我创建的
const SOURCES: [string, string][] = [["current", "全部"], ["public", "公开"], ["shared", "空间共享"], ["mine", "我创建的"]];
const PAGE_SIZE = 20;

export function Images({ state }: { state: AppState }) {
  const spaces = spacesOf(state);
  const [source, setSource] = useState("current");
  const [space, setSpace] = useState("");
  const [input, setInput] = useState("");
  const [q, setQ] = useState("");
  const [page, setPage] = useState(1);
  const [repo, setRepo] = useState<any>(null);
  useEffect(() => { const t = setTimeout(() => setQ(input.trim()), 300); return () => clearTimeout(t); }, [input]);
  useEffect(() => setPage(1), [source, space, q]);

  const { data, loading, refreshing, error, refresh } = useCached<any>(
    `images:${source}:${space}:${q}:${page}`, () => api.images({ page, size: PAGE_SIZE, source, space, q }), [source, space, q, page]);
  const { data: counts } = useCached<Record<string, number>>(`image-count:${space}:${q}`, async () => {
    const pairs = await Promise.all(SOURCES.map(async ([k]) => [k, Number((await api.images({ size: 1, source: k, space, q })).data?.listCount) || 0] as const));
    return Object.fromEntries(pairs);
  }, [space, q]);

  const admin = isAdmin();
  const list: any[] = data?.data?.imageList || [];
  const total = Number(data?.data?.listCount) || 0;
  const skipped = Number(data?.data?.skippedSpaces) || 0;

  const openRepo = async (id: number, sp: string, name: string) => {
    setRepo({ loading: true, name });
    try { const d = await pcall("GET", "core", "/imageRepository/detail/" + id, {}, sp); setRepo({ name, data: d.data || {} }); } catch (e: any) { setRepo({ name, error: e.message }); }
  };

  return (
    <>
      <PageBar title="镜像" hint={data ? `${total} 个镜像仓库` : undefined}
        right={<button className="btn" onClick={refresh}>{refreshing ? <Loader2 size={14} className="animate-spin" /> : <RefreshCw size={14} />}刷新</button>} />
      <Toolbar search={input} onSearch={setInput} placeholder="按仓库名搜索">
        <Chips label="按可见性筛选" items={SOURCES} value={source} onChange={setSource} counts={counts || undefined} />
        <div className="flex-1" />
        {spaces.length > 1 && (
          <select className="field w-36" aria-label="空间" value={space} onChange={(e) => setSpace(e.target.value)}>
            <option value="">全部空间</option>{spaces.map((s) => <option key={s.id} value={s.id}>{s.name}</option>)}
          </select>
        )}
      </Toolbar>
      {error && !data && <div className="card p-4 text-body mb-3">镜像列表读取失败：{error}。检查平台连接后点刷新。</div>}
      {loading ? <Spinner /> : <>
        <Table head={<><Th>镜像仓库</Th><Th>空间</Th><Th num>镜像数</Th><Th>可见性</Th>{admin && <Th>创建人</Th>}<Th>创建时间</Th></>}>
          {list.length ? list.map((r) => (
            <tr key={`${r.spaceId}-${r.imageRepositoryId}`} className="cursor-pointer group" data-repo={r.imageRepositoryId}
              onClick={() => openRepo(Number(r.imageRepositoryId) || 0, r.spaceId, field(r, ["originalName", "repositoryName"]))}>
              <Td clip><div className="font-medium group-hover:text-brand transition-colors">{field(r, ["originalName", "repositoryName"])}</div><div className="text-[11px] text-ink-faint truncate">{field(r, ["repositoryName"], "")}</div></Td>
              <Td className="text-ink-soft">{r.spaceName || "—"}</Td>
              <Td num>{field(r, ["imageNum"], "—")}</Td>
              <Td><span className={"tag " + (ACCESS_TAG[r.accessType] || "tag-gray")}>{ACCESS[r.accessType] || r.accessType || "—"}</span></Td>
              {admin && <Td className="text-ink-soft">{field(r, ["displayName", "userName"], "—")}</Td>}
              <Td className="text-ink-soft tabular-nums whitespace-nowrap">{field(r, ["createTime"], "—")}</Td>
            </tr>
          )) : <tr><td colSpan={admin ? 6 : 5}><Empty>没有符合条件的镜像仓库，换个可见性或清空搜索。</Empty></td></tr>}
        </Table>
        <Pager page={page} size={PAGE_SIZE} total={total} count={list.length} busy={refreshing} onPage={setPage}
          note={<><span>点仓库查看镜像版本。</span>{skipped > 0 && <span className="text-warn">{skipped} 个空间读取失败未计入</span>}</>} />
      </>}
      <Drawer open={!!repo} onClose={() => setRepo(null)} title={repo?.name} sub={repo?.data?.repositoryName}>
        {repo?.loading ? <Spinner /> : repo?.error ? <p className="p-5 text-body text-danger">读取失败：{repo.error}</p> : repo?.data && (
          <div className="p-5">
            <Table head={<><Th>镜像 ID</Th><Th>版本</Th><Th num>大小</Th><Th>更新时间</Th></>}>
              {(repo.data.imageList || []).length ? repo.data.imageList.map((im: any) => (
                <tr key={field(im, ["imageId"])}><Td><span className="text-[11px] text-ink-faint">{field(im, ["imageId"])}</span></Td><Td><span className="font-medium">{field(im, ["imageTag"])}</span></Td>
                  <Td num>{im.imageSize ? (Number(im.imageSize) / 1073741824).toFixed(2) + " GiB" : "—"}</Td><Td className="text-ink-soft">{field(im, ["updateTime"], "—")}</Td></tr>
              )) : <tr><td colSpan={4} className="px-4 py-6 text-center text-ink-faint">这个仓库还没有镜像。</td></tr>}
            </Table>
          </div>
        )}
      </Drawer>
    </>
  );
}
