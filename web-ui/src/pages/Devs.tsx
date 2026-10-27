import { useState, useMemo } from "react";
import { RefreshCw, ExternalLink, Loader2 } from "lucide-react";
import { AppState, pcall, qget, field, spacesOf } from "../lib/api";
import { useCached } from "../lib/cache";
import { useList } from "../lib/table";
import { PageBar, Spinner, Table, Th, Td, SortTh, Toolbar, Badge, Card, Empty, statusText, useToast } from "../components/ui";
import { Drawer, Json } from "../components/Drawer";
import { Copy } from "lucide-react";

export function Devs({ state }: { state: AppState }) {
  const toast = useToast();
  const spaces = spacesOf(state);
  const [scope, setScope] = useState("mine");
  const [detail, setDetail] = useState<any>(null);
  const [showAll, setShowAll] = useState(false);
  const { data: res, loading, refreshing, error, refresh } = useCached<any>(
    `devs:${scope}:${showAll}`,
    async () => {
      const d = await qget(`/api/devs?scope=${scope}${showAll ? "&all=1" : ""}`);
      return { list: d.data?.devList || [], hidden: d.data?.hiddenCount || 0 };
    },
    [scope, showAll]
  );
  const list = res?.list as any[] | undefined;
  const hidden = res?.hidden || 0;

  const statuses = useMemo(() => [...new Set((list || []).map((d) => String(field(d, ["jobenvStatus"], ""))))].filter(Boolean), [list]);
  const { view, search, setSearch, filters, setFilter, sortKey, sortDir, toggleSort } = useList(list || [], {
    searchText: (d) => [field(d, ["projectName", "projectId"], ""), field(d, ["jobenvId"], ""), field(d, ["jobenvStatus"], ""), field(d, ["ownerDisplayName", "createDisplayName"], ""), d.spaceName || ""].join(" "),
    sorts: { project: (d) => field(d, ["projectName", "projectId"], ""), id: (d) => Number(field(d, ["jobenvId"], 0)) || 0, status: (d) => field(d, ["jobenvStatus"], "") },
    initialSort: { key: "id", dir: "desc" },
  });

  const term = async (id: string, sp: string) => {
    try { const r = await qget(`/api/terminal-url?kind=dev&id=${encodeURIComponent(id)}&instance=0${sp ? "&space=" + sp : ""}`); window.open(r.terminalUrl, "_blank"); } catch (e: any) { toast(e.message, true); }
  };
  const openDetail = async (id: string, sp: string) => {
    setDetail({ loading: true, id });
    try { const d = await pcall("GET", "core", "/jobenv/refresh/" + id, {}, sp); setDetail({ id, sp, data: d }); } catch (e: any) { setDetail({ id, sp, error: e.message }); }
  };

  return (
    <>
      <PageBar title="开发环境" right={
        <div className="flex items-center gap-2">
          <label className={"inline-flex items-center gap-2 h-8 px-3 rounded-ctl border text-[13px] cursor-pointer select-none transition-colors shadow-ctl " + (showAll ? "bg-brand-50 border-brand/30 text-brand" : "bg-white border-line text-ink-soft hover:text-ink")}>
            <input type="checkbox" className="accent-brand w-3.5 h-3.5" checked={showAll} onChange={(e) => setShowAll(e.target.checked)} />显示全部项目
          </label>
          <select className="field field-sm w-32" aria-label="范围" value={scope} onChange={(e) => setScope(e.target.value)}>{[["mine", "我的项目"], ["shared", "共享项目"], ["public", "公开项目"], ["all", "全部项目"]].map(([k, l]) => <option key={k} value={k}>{l}</option>)}</select>
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
            <option value="">全部状态</option>{statuses.map((s) => <option key={s} value={s}>{statusText(s)}</option>)}
          </select>
        )}
      </Toolbar>
      {error && !list && <Card className="p-4 text-sm mb-4">加载失败：{error}</Card>}
      {!showAll && hidden > 0 && (
        <p className="text-aux text-ink-faint mb-3">
          已隐藏 {hidden} 台不属于同名项目的开发机（约定：每个空间只用与空间同名的那个项目）。勾选「显示全部项目」可查看。
        </p>
      )}
      {loading ? <Spinner /> :
        <Table head={<>
          <SortTh sortKey="project" active={sortKey === "project"} dir={sortDir} onSort={toggleSort}>项目</SortTh>
          <SortTh sortKey="id" active={sortKey === "id"} dir={sortDir} onSort={toggleSort}>环境 ID</SortTh>
          <Th>空间</Th>
          <SortTh sortKey="status" active={sortKey === "status"} dir={sortDir} onSort={toggleSort}>状态</SortTh>
          <Th>负责人</Th><Th><span className="sr-only">操作</span></Th>
        </>}>
          {view.length ? view.map((d, i) => {
            const id = String(field(d, ["jobenvId"], "")), sp = d.spaceId || "";
            return (
              <tr key={i}>
                <Td clip><span className="font-medium">{field(d, ["projectName", "projectId"])}</span></Td>
                <Td><span className="text-[12px] text-ink-faint font-mono">{id}</span></Td>
                <Td>{d.spaceName || "-"}</Td>
                <Td><Badge text={field(d, ["jobenvStatus"], "-")} /></Td>
                <Td clip>{field(d, ["ownerDisplayName", "createDisplayName"])}</Td>
                <Td className="whitespace-nowrap text-right">
                  <span className="inline-flex gap-1">
                    <button className="btn btn-mini" title="在新标签页打开网页终端" onClick={() => term(id, sp)}><ExternalLink size={12} />终端</button>
                    <button className="btn btn-mini" onClick={() => openDetail(id, sp)}>详情与 SSH</button>
                  </span>
                </Td>
              </tr>
            );
          }) : <tr><td colSpan={6}><Empty>{search ? "没有匹配的开发环境，清空搜索试试。" : "还没有开发环境。"}</Empty></td></tr>}
        </Table>}
      <Drawer open={!!detail} onClose={() => setDetail(null)} title={detail && `开发环境 ${detail.id}`}>
        {detail?.loading ? <Spinner /> : detail?.error ? <p className="p-6 text-body text-danger">加载失败：{detail.error}</p> : detail && (() => {
          const d = detail.data?.data || detail.data || {};
          const inst = (((d.devJobDetail || {}).taskroleList || [])[0] || {}).instanceList || [];
          const raw = String(field(inst[0] || {}, ["sshConnection"], "") || "").trim();
          // 平台网关只认 ssh-rsa 签名，OpenSSH 8.8+ 默认拒绝，原样粘贴会 Permission denied
          const ssh = raw && raw !== "-" ? raw.replace(/^ssh\s+/, "ssh -o PubkeyAcceptedAlgorithms=+ssh-rsa ") : "";
          return (
            <div className="p-6 space-y-4">
              <Card className="p-5">
                <div className="mb-3 text-h2">VS Code / SSH 连接</div>
                {ssh ? (
                  <>
                    <div className="flex items-center gap-2">
                      <code className="code flex-1">{ssh}</code>
                      <button className="btn btn-mini" onClick={() => { navigator.clipboard?.writeText(ssh); toast("已复制 SSH 连接"); }}><Copy size={12} />复制</button>
                    </div>
                    <p className="text-xs text-ink-faint mt-2">VS Code 装 Remote-SSH 扩展 → 命令面板「Remote-SSH: Connect to Host」→ 粘贴上面的 host（先把它加入 ~/.ssh/config），即可连到申请的 GPU 环境写代码。</p>
                  </>
                ) : <p className="text-sm text-ink-faint">未启用 SSH。在终端执行 qd dev ssh-config {detail.id} --enable 后刷新详情获取连接串。</p>}
              </Card>
              <details><summary className="text-[13px] text-ink-soft cursor-pointer hover:text-ink">原始详情 JSON</summary><div className="mt-2"><Json value={detail.data} /></div></details>
            </div>
          );
        })()}
      </Drawer>
    </>
  );
}
