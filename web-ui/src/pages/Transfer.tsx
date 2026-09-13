import { RefreshCw, Loader2, Copy, Info } from "lucide-react";
import { AppState, pcall, spacesOf, field } from "../lib/api";
import { useCached } from "../lib/cache";
import { PageBar, Card, Spinner, Table, Th, Td, useToast } from "../components/ui";

export function Transfer({ state }: { state: AppState }) {
  const toast = useToast();
  const spaces = spacesOf(state);
  const { data, loading, refreshing, refresh } = useCached<any[]>("sftp", async () =>
    Promise.all(spaces.map(async (sp) => {
      try {
        const d = await pcall("GET", "core", "/user/sftp/channel/list", { pageNum: 1, pageSize: 50 }, sp.id);
        return { sp, list: d.data?.sftpList || [], count: d.data?.listCount ?? 0 };
      } catch { return { sp, list: [], count: 0 }; }
    }))
  );

  const copy = (t: string) => { navigator.clipboard?.writeText(t); toast("已复制"); };
  const total = (data || []).reduce((a, x) => a + (x.list?.length || 0), 0);

  if (loading) return <Spinner />;
  return (
    <>
      <PageBar title="文件传输" hint="SFTP 通道（平台限单用户同时最多 3 个）"
        right={<button className="btn" onClick={refresh}>{refreshing ? <Loader2 size={14} className="animate-spin" /> : <RefreshCw size={14} />}刷新</button>} />

      {total === 0 && (
        <Card className="p-4 mb-4">
          <div className="flex gap-3">
            <Info size={16} className="text-brand shrink-0 mt-0.5" />
            <div className="text-sm text-ink-soft">
              <p className="mb-1">当前没有已开通的 SFTP 通道。</p>
              <p className="text-xs text-ink-faint">开通入口在平台的数据集上传流程里（数据 → 进入数据集 → 上传 → SFTP）。开通后回到本页即可看到通道的连接信息，用 FileZilla / WinSCP / <code>sftp</code> 命令传大文件比网页上传快很多。</p>
            </div>
          </div>
        </Card>
      )}

      {(data || []).map(({ sp, list }) => list.length ? (
        <div key={sp.id} className="mb-5">
          <div className="flex items-center gap-2 mb-2"><b className="text-sm">空间 {sp.name}</b><span className="text-xs text-ink-faint">{list.length} 个通道</span></div>
          <Table head={<><Th>通道</Th><Th>主机</Th><Th num>端口</Th><Th>账号</Th><Th>状态</Th><Th>到期</Th><Th></Th></>}>
            {list.map((c: any, i: number) => {
              const host = field(c, ["host", "ip", "address"], "-");
              const port = field(c, ["port"], "-");
              const user = field(c, ["userName", "user", "account"], "-");
              const conn = `sftp -P ${port} ${user}@${host}`;
              return (
                <tr key={i} className="hover:bg-canvas/60">
                  <Td><b>{field(c, ["channelName", "name", "id"], "-")}</b></Td>
                  <Td>{host}</Td>
                  <Td num>{port}</Td>
                  <Td>{user}</Td>
                  <Td>{field(c, ["status", "state"], "-")}</Td>
                  <Td>{field(c, ["expireTime", "endTime"], "-")}</Td>
                  <Td><button className="btn btn-mini" onClick={() => copy(conn)}><Copy size={12} />复制命令</button></Td>
                </tr>
              );
            })}
          </Table>
        </div>
      ) : null)}
    </>
  );
}
