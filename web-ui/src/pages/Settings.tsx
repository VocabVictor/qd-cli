import { useState } from "react";
import { api, AppState, getMe, isAdmin } from "../lib/api";
import { PageBar, Card, useToast } from "../components/ui";
import { Meter } from "../components/viz";

function Row({ k, v }: { k: string; v: any }) {
  return <div className="flex gap-4 py-2 text-[13px] border-b border-line last:border-0"><span className="w-24 shrink-0 text-ink-faint">{k}</span><span className="min-w-0 break-all">{v}</span></div>;
}

function MyAccount() {
  const toast = useToast();
  const me = getMe();
  const [oldPw, setOldPw] = useState("");
  const [newPw, setNewPw] = useState("");
  const [busy, setBusy] = useState(false);
  const change = async (e: React.FormEvent) => {
    e.preventDefault();
    if (newPw.length < 8) return toast("新密码至少 8 位", true);
    setBusy(true);
    try { await api.localAuth.changePassword(oldPw, newPw); setOldPw(""); setNewPw(""); toast("密码已修改，其他设备上的登录已失效"); }
    catch (err: any) { toast(err.message, true); } finally { setBusy(false); }
  };
  const total = me.quota?.total ?? 0, used = me.usage?.total ?? 0;
  return (
    <div className="grid md:grid-cols-2 gap-4 mb-4">
      <Card className="p-5">
        <h2 className="text-h2 mb-3">我的账号</h2>
        <Row k="用户名" v={me.username} />
        <Row k="角色" v={isAdmin() ? "管理员" : "成员"} />
        {!isAdmin() && (
          <div className="mt-3">
            <div className="flex justify-between text-aux mb-1"><span className="text-ink-soft">GPU 总配额</span><span className="tabular-nums">{used} / {total} 卡</span></div>
            <Meter value={used} total={total || 1} tone={total && used >= total ? "danger" : "brand"} />
            {total === 0 && <p className="text-[11px] text-ink-faint mt-2">你的配额为 0，只能提交 CPU 作业。需要 GPU 请联系管理员分配。</p>}
          </div>
        )}
      </Card>
      <Card className="p-5">
        <h2 className="text-h2 mb-3">修改密码</h2>
        <form onSubmit={change} className="space-y-2.5">
          <input className="field" type="password" placeholder="当前密码" value={oldPw} onChange={(e) => setOldPw(e.target.value)} autoComplete="current-password" aria-label="当前密码" />
          <input className="field" type="password" placeholder="新密码（至少 8 位）" value={newPw} onChange={(e) => setNewPw(e.target.value)} autoComplete="new-password" aria-label="新密码" />
          <button type="submit" className="btn btn-pri" disabled={busy || !oldPw || !newPw}>修改密码</button>
        </form>
      </Card>
    </div>
  );
}

export function SettingsPage({ state }: { state: AppState }) {
  const toast = useToast();
  const [key, setKey] = useState("base-url");
  const [val, setVal] = useState("");
  const [ka, setKa] = useState("");
  const publicMode = getMe().required;

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
      {publicMode && <MyAccount />}
      <Card className="p-5 mb-4">
        <h2 className="text-h2 mb-3">平台连接</h2>
        <Row k="版本" v={`qd ${state.version}`} />
        <Row k="平台地址" v={<span className="text-ink-soft">{state.baseUrl}</span>} />
        <Row k="空间 ID" v={<span className="text-ink-soft">{state.spaceId || "-"}</span>} />
        <Row k="平台账号" v={state.username || "未登录"} />
        <Row k="并发数" v={state.concurrency} />
        <Row k="TLS 校验" v={state.insecureTls ? "已关闭（内网默认）" : "开启"} />
      </Card>
      {isAdmin() && (
        <>
          <Card className="p-5 mb-4">
            <h2 className="text-h2 mb-3">修改配置<span className="font-normal text-ink-faint ml-2 text-aux">重启 qd web 生效</span></h2>
            <div className="flex items-center gap-2 flex-wrap">
              <select className="field w-44" value={key} onChange={(e) => setKey(e.target.value)} aria-label="配置项">
                {["base-url", "space-id", "concurrency", "connect-timeout", "request-timeout", "insecure-tls"].map((k) => <option key={k}>{k}</option>)}
              </select>
              <input className="field flex-1 max-w-sm" placeholder="值" value={val} onChange={(e) => setVal(e.target.value)} aria-label="配置值" />
              <button className="btn btn-pri" onClick={save}>保存</button>
            </div>
          </Card>
          <Card className="p-5">
            <h2 className="text-h2 mb-3">登录保活</h2>
            <div className="flex items-center gap-2 flex-wrap">
              <button className="btn" onClick={() => keepalive("status")}>查看状态</button>
              <button className="btn" onClick={() => keepalive("once")}>立即保活一次</button>
              <button className="btn" onClick={() => keepalive("install")}>安装计划任务（30 分钟）</button>
              <button className="btn btn-danger" onClick={() => keepalive("remove")}>移除计划任务</button>
              <span className="text-aux text-ink-faint">{ka}</span>
            </div>
          </Card>
        </>
      )}
    </>
  );
}
