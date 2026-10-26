import { useState } from "react";
import { Lock } from "lucide-react";
import { clsx } from "clsx";
import { api } from "../lib/api";
import { useToast } from "../components/ui";

/** 本机账号墙：只在 `qd web --host` 绑定非 127.0.0.1 时出现，拦住还没登录这台
 *  机器控制面板的人。和下面的 GPU 平台登录（Login.tsx）完全无关，过了这一关
 *  才轮到平台登录。 */
export function AuthGate({ hasAccount, onAuthed }: { hasAccount: boolean; onAuthed: () => void }) {
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
    <div className="h-full grid place-items-center px-4">
      <form onSubmit={submit} className="card w-full max-w-sm p-7">
        <div className="flex items-center gap-2.5 mb-1">
          <span className="grid place-items-center w-9 h-9 rounded-xl text-white" style={{ background: "linear-gradient(135deg,#7c5cfc,#b14eff)" }}><Lock size={18} /></span>
          <div className="text-lg font-semibold">qd 控制面板</div>
        </div>
        <p className="text-sm text-ink-faint mb-5">这台机器已对公网开放，需要本机账号才能进入</p>
        <div className="flex gap-1 mb-5 p-1 rounded-ctl bg-canvas">
          <button type="button" onClick={() => setTab("login")}
            className={clsx("flex-1 h-8 rounded-ctl text-sm font-medium transition-colors", tab === "login" ? "bg-white shadow-sm" : "text-ink-soft")}>
            登录
          </button>
          <button type="button" onClick={() => setTab("register")}
            className={clsx("flex-1 h-8 rounded-ctl text-sm font-medium transition-colors", tab === "register" ? "bg-white shadow-sm" : "text-ink-soft")}>
            注册
          </button>
        </div>
        <label className="block text-xs font-medium text-ink-soft mb-1">用户名</label>
        <input className="field mb-3" value={u} onChange={(e) => setU(e.target.value)} placeholder="用户名" autoFocus />
        <label className="block text-xs font-medium text-ink-soft mb-1">密码</label>
        <input className="field mb-3" type="password" value={p} onChange={(e) => setP(e.target.value)} placeholder={tab === "register" ? "至少 8 位" : "密码"} />
        {tab === "register" && (
          <>
            <label className="block text-xs font-medium text-ink-soft mb-1">邀请码</label>
            <input className="field mb-4" value={invite} onChange={(e) => setInvite(e.target.value)} placeholder="找启动这台机器的人要" />
          </>
        )}
        <button className={clsx("btn btn-pri w-full justify-center", tab === "login" && "mt-1")} disabled={busy}>
          {busy ? "处理中…" : tab === "register" ? "注册并登录" : "登录"}
        </button>
      </form>
    </div>
  );
}
