import { useState } from "react";
import { Lock } from "lucide-react";
import { api } from "../lib/api";
import { deviceId } from "../lib/device";
import { useToast } from "../components/ui";

/** 本机账号墙：只在 `qd web --host` 绑定非 127.0.0.1 时出现。邀请码决定角色
 *  （管理员码 → 管理员，普通码 → 成员，配额 0），由服务端识别，页面上只有一个输入框。 */
export function AuthGate({ hasAccount, notice, onAuthed }: { hasAccount: boolean; notice?: string; onAuthed: () => void }) {
  const toast = useToast();
  const [tab, setTab] = useState<"login" | "register">(hasAccount ? "login" : "register");
  const [u, setU] = useState("");
  const [p, setP] = useState("");
  const [invite, setInvite] = useState("");
  const [busy, setBusy] = useState(false);

  const submit = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!u || !p) return toast("请输入用户名和密码", true);
    if (tab === "register" && !invite) return toast("请输入邀请码", true);
    setBusy(true);
    try {
      if (tab === "register") await api.localAuth.register(u, p, invite);
      else await api.localAuth.login(u, p);
      onAuthed();
    } catch (err: any) {
      toast(err.message || "操作失败", true);
    } finally {
      setBusy(false);
    }
  };

  return (
    <div className="h-full grid place-items-center px-4 bg-canvas">
      <form onSubmit={submit} className="card w-full max-w-[340px] p-7">
        <div className="flex flex-col items-center text-center mb-5">
          <span className="grid place-items-center w-9 h-9 rounded-[7px] bg-ink text-white mb-3"><Lock size={17} strokeWidth={1.8} /></span>
          <div className="text-[17px] font-semibold">qd 控制台</div>
          <p className="text-aux text-ink-faint mt-1">这台机器已对外开放，需要本机账号才能进入</p>
        </div>
        {notice && <p className="text-aux text-warn bg-warn/10 rounded-ctl px-3 py-2 mb-4" role="alert" data-testid="auth-notice">{notice}</p>}
        <div className="segmented mb-5" role="tablist">
          <button type="button" role="tab" aria-pressed={tab === "login"} onClick={() => setTab("login")}>登录</button>
          <button type="button" role="tab" aria-pressed={tab === "register"} onClick={() => setTab("register")}>注册</button>
        </div>
        <label className="block text-aux font-medium text-ink-soft mb-1" htmlFor="auth-user">用户名</label>
        <input id="auth-user" className="field mb-3" value={u} onChange={(e) => setU(e.target.value)} placeholder="用户名" autoComplete="username" autoFocus />
        <label className="block text-aux font-medium text-ink-soft mb-1" htmlFor="auth-pass">密码</label>
        <input id="auth-pass" className="field mb-3" type="password" value={p} onChange={(e) => setP(e.target.value)}
          placeholder={tab === "register" ? "至少 8 位" : "密码"} autoComplete={tab === "register" ? "new-password" : "current-password"} />
        {tab === "register" && (
          <>
            <label className="block text-aux font-medium text-ink-soft mb-1" htmlFor="auth-invite">邀请码</label>
            <input id="auth-invite" className="field mb-1" value={invite} onChange={(e) => setInvite(e.target.value)} placeholder="找启动这台机器的人要" autoComplete="off" />
            <p className="text-[11px] text-ink-faint mb-3">管理员邀请码注册为管理员；普通邀请码注册为成员，GPU 配额默认为 0，需管理员分配。</p>
          </>
        )}
        <button type="submit" className="btn btn-pri w-full h-9 mt-2" disabled={busy}>
          {busy ? "处理中…" : tab === "register" ? "注册并登录" : "登录"}
        </button>
        <p className="text-[11px] text-ink-faint text-center mt-4">本机设备指纹 <span className="font-mono select-all" data-testid="device-id">{deviceId}</span></p>
      </form>
    </div>
  );
}
