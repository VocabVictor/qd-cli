import { useState } from "react";
import { api, AppState } from "../lib/api";
import { PageBar, Card, useToast } from "../components/ui";

function Row({ k, v }: { k: string; v: any }) {
  return <div className="flex gap-4 py-1.5 text-sm"><span className="w-24 shrink-0 text-ink-faint">{k}</span><span className="min-w-0 break-all">{v}</span></div>;
}

export function SettingsPage({ state }: { state: AppState }) {
  const toast = useToast();
  const [key, setKey] = useState("base-url");
  const [val, setVal] = useState("");
  const [ka, setKa] = useState("");

  const save = async () => {
    try { const r = await api.config(key, val.trim()); toast(`已保存 ${r.key}（${r.note}）`); }
    catch (e: any) { toast(e.message, true); }
  };
  const keepalive = async (action: string) => {
    setKa("…");
    try { const r = await api.keepalive(action); setKa(action === "status" ? (r.installed ? "已安装计划任务" : "未安装计划任务") : "完成"); if (action !== "status") toast("保活操作完成"); }
    catch (e: any) { setKa(""); toast(e.message, true); }
  };

  return (
    <>
      <PageBar title="设置" />
      <Card className="p-5 mb-4">
        <h2 className="text-sm font-semibold mb-3">当前状态</h2>
        <Row k="版本" v={`qd ${state.version}`} />
        <Row k="平台地址" v={<span className="text-ink-faint">{state.baseUrl}</span>} />
        <Row k="空间 ID" v={<span className="text-ink-faint">{state.spaceId || "-"}</span>} />
        <Row k="登录用户" v={state.username || "未登录"} />
        <Row k="并发数" v={state.concurrency} />
        <Row k="TLS 校验" v={state.insecureTls ? "已关闭（内网默认）" : "开启"} />
      </Card>
      <Card className="p-5 mb-4">
        <h2 className="text-sm font-semibold mb-3">修改配置<span className="font-normal text-ink-faint ml-2 text-xs">重启 qd web 生效</span></h2>
        <div className="flex items-center gap-2 flex-wrap">
          <select className="field w-44" value={key} onChange={(e) => setKey(e.target.value)}>
            {["base-url", "space-id", "concurrency", "connect-timeout", "request-timeout", "insecure-tls"].map((k) => <option key={k}>{k}</option>)}
          </select>
          <input className="field flex-1 max-w-sm" placeholder="值" value={val} onChange={(e) => setVal(e.target.value)} />
          <button className="btn btn-pri" onClick={save}>保存</button>
        </div>
      </Card>
      <Card className="p-5">
        <h2 className="text-sm font-semibold mb-3">登录保活</h2>
        <div className="flex items-center gap-2 flex-wrap">
          <button className="btn" onClick={() => keepalive("status")}>查看状态</button>
          <button className="btn" onClick={() => keepalive("once")}>立即保活一次</button>
          <button className="btn" onClick={() => keepalive("install")}>安装计划任务（30 分钟）</button>
          <button className="btn btn-danger" onClick={() => keepalive("remove")}>移除计划任务</button>
          <span className="text-sm text-ink-faint">{ka}</span>
        </div>
      </Card>
    </>
  );
}
