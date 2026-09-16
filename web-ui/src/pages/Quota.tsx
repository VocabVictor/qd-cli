import { AppState, pcall, spacesOf, fmtMiB } from "../lib/api";
import { useCached } from "../lib/cache";
import { PageBar, Card, Spinner, Table, Th, Td, Tile } from "../components/ui";

const UNLIMITED = 4;
const cores = (milli: number) => (Number(milli) || 0) >= 1000 ? (Number(milli) / 1000).toFixed(0) : String(Number(milli) || 0);

function Bar({ used, total }: { used: number; total: number }) {
  const pct = total > 0 ? Math.min(100, (used / total) * 100) : 0;
  return <div className="h-1.5 rounded-full bg-line mt-2 overflow-hidden"><div className="h-full rounded-full bg-brand" style={{ width: pct + "%" }} /></div>;
}

function QuotaCard({ label, limitType, total, used, left, fmt, unit }: { label: string; limitType: number; total: number; used: number; left: number; fmt: (n: number) => string; unit: string }) {
  const unlimited = limitType === UNLIMITED;
  return (
    <Card className="p-4">
      <div className="flex items-baseline justify-between">
        <span className="text-sm font-medium text-ink-soft">{label}</span>
        {unlimited ? <span className="tag bg-brand-50 text-brand">无限制</span> : <span className="text-xs text-ink-faint">配额 {fmt(total)}{unit}</span>}
      </div>
      <div className="mt-2 text-2xl font-semibold tracking-tight">{fmt(used)}<span className="text-sm text-ink-faint">{unit} 已用</span></div>
      {!unlimited && <><Bar used={used} total={total} /><div className="text-[11px] text-ink-faint mt-1">剩余 {fmt(left)}{unit}</div></>}
    </Card>
  );
}

export function Quota({ state }: { state: AppState }) {
  const spaces = spacesOf(state);
  const { data, loading } = useCached<any[]>("quota", async () =>
    Promise.all(spaces.map(async (sp) => {
      try { const d = await pcall("GET", "core", "/user/quota/available", {}, sp.id); return { sp, q: d.data || {} }; }
      catch { return { sp, q: null }; }
    }))
  );

  if (loading) return <Spinner />;
  return (
    <>
      <PageBar title="配额" hint="每个空间的资源配额与占用（含各 GPU 型号明细）" />
      {(data || []).map(({ sp, q }) => {
        if (!q) return <Card key={sp.id} className="p-4 mb-4 text-sm text-ink-faint">空间 {sp.name}：无配额信息或无权限</Card>;
        const gpuModels = q.gpuquotaDataList || [];
        // 平台顶层的 gpuCardCnt/usedGpuCardCnt/leftGpuCardCnt 常年返回 0，
        // 真实数字只在按型号拆分的 gpuquotaDataList 里，有明细就以明细汇总为准
        const gpuSum = (key: string) => gpuModels.reduce((acc: number, g: any) => acc + (Number(g[key]) || 0), 0);
        const gpu = gpuModels.length > 0
          ? { total: gpuSum("cardCnt"), used: gpuSum("usedCardCnt"), left: gpuSum("leftCardCnt") }
          : { total: Number(q.gpuCardCnt) || 0, used: Number(q.usedGpuCardCnt) || 0, left: Number(q.leftGpuCardCnt) || 0 };
        return (
          <div key={sp.id} className="mb-6">
            <div className="flex items-center gap-2 mb-3"><b className="text-sm">空间 {sp.name}</b><span className="text-xs text-ink-faint">{sp.id}</span></div>
            <div className="grid grid-cols-2 lg:grid-cols-4 gap-3 mb-3">
              <QuotaCard label="GPU 卡" limitType={q.gpuLimitType} total={gpu.total} used={gpu.used} left={gpu.left} fmt={(n) => String(n)} unit=" 卡" />
              <QuotaCard label="CPU" limitType={q.cpuLimitType} total={q.cpu} used={q.usedCpu} left={q.leftCpu} fmt={cores} unit=" 核" />
              <QuotaCard label="内存" limitType={q.memoryLimitType} total={q.memory} used={q.usedMemory} left={q.leftMemory} fmt={fmtMiB} unit="" />
              <QuotaCard label="存储" limitType={q.storageLimitType} total={q.storage} used={q.usedStorage} left={q.leftStorage} fmt={fmtMiB} unit="" />
            </div>
            {gpuModels.length > 0 && (
              <Table head={<><Th>GPU 型号</Th><Th num>配额（卡）</Th><Th num>已用</Th><Th num>剩余</Th></>}>
                {gpuModels.map((g: any, i: number) => (
                  <tr key={i} className="hover:bg-canvas/60">
                    <Td><b>{g.graphicsCard}</b></Td>
                    <Td num>{g.cardCnt}</Td>
                    <Td num>{g.usedCardCnt}</Td>
                    <Td num><b className={g.leftCardCnt > 0 ? "text-ok" : ""}>{g.leftCardCnt}</b></Td>
                  </tr>
                ))}
              </Table>
            )}
          </div>
        );
      })}
    </>
  );
}
