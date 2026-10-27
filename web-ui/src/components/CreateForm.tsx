import { useEffect, useState, useMemo, useRef, ReactNode } from "react";
import { Check, Cpu, FileText, Loader2, Save, TerminalSquare, X } from "lucide-react";
import { api, qpost, pcall, eachSpace, spacesOf, AppState, asList, field, getMe, isAdmin } from "../lib/api";
import { Drawer } from "./Drawer";
import { dropCache } from "../lib/cache";
import { useToast } from "./ui";

type Kind = "job";

/** 上方标签、下方控件的一格。 */
function Field({ label, htmlFor, hint, className, children }: { label: string; htmlFor?: string; hint?: ReactNode; className?: string; children: ReactNode }) {
  return (
    <div className={className}>
      <label htmlFor={htmlFor} className="label">{label}</label>
      {children}
      {hint && <div className="hint">{hint}</div>}
    </div>
  );
}

function Section({ icon: Icon, tone, title, desc, children }: { icon: typeof Cpu; tone: string; title: string; desc: string; children: ReactNode }) {
  return (
    <section className="card p-5">
      <div className="flex items-center gap-3 mb-4">
        <span className={`ico ico-${tone}`}><Icon size={15} strokeWidth={1.9} /></span>
        <div><h3 className="text-h2">{title}</h3><p className="text-[11px] text-ink-faint">{desc}</p></div>
      </div>
      <div className="space-y-4">{children}</div>
    </section>
  );
}

/** 数字输入：输入过程中允许暂时为空，离开时再收回到合法范围，避免一删就跳回最小值。 */
function NumField({ value, onChange, min = 1, max, unit, label, id, className }: {
  value: number; onChange: (n: number) => void; min?: number; max?: number; unit?: string; label: string; id?: string; className?: string;
}) {
  const [text, setText] = useState(String(value));
  useEffect(() => { if (Number(text) !== value) setText(String(value)); }, [value]); // eslint-disable-line
  const clamp = (n: number) => Math.min(max ?? Infinity, Math.max(min, Math.round(n)));
  const bad = text !== "" && (Number(text) < min || (max != null && Number(text) > max));
  return (
    <div className={"affix " + (className || "")}>
      <input id={id} className="field text-right tabular-nums" type="number" inputMode="numeric" min={min} max={max} aria-label={label} value={text}
        aria-invalid={bad || undefined}
        onChange={(e) => { setText(e.target.value); const n = Number(e.target.value); if (e.target.value !== "" && Number.isFinite(n) && n >= min) onChange(clamp(n)); }}
        onBlur={() => { const n = clamp(Number(text) || min); setText(String(n)); onChange(n); }} />
      {unit && <span>{unit}</span>}
    </div>
  );
}

export function CreateForm({ kind, state, open, onClose, onDone, initial, groupLabel }: { kind: Kind; state: AppState; open: boolean; onClose: () => void; onDone: () => void; initial?: any; groupLabel?: string }) {
  const toast = useToast();
  const spaces = spacesOf(state);
  const [space, setSpace] = useState(initial?.spaceId || state.spaceId || spaces[0]?.id || "");
  const [name, setName] = useState("qd-job");
  const [projects, setProjects] = useState<any[]>([]);
  const [groups, setGroups] = useState<any[]>([]);
  const [repos, setRepos] = useState<any[]>([]);
  const [tags, setTags] = useState<any[]>([]);
  const [projectId, setProjectId] = useState("");
  const [rsgroupId, setRsgroupId] = useState(initial?.rsgroupId ? String(initial.rsgroupId) : "");
  const [repoId, setRepoId] = useState("");
  const [imageId, setImageId] = useState("");
  const [useGpu, setUseGpu] = useState(true);
  const [gpuType, setGpuType] = useState("");
  const [gpu, setGpu] = useState(1);
  const [cpu, setCpu] = useState(8);
  const [memory, setMemory] = useState(32);
  const [storage, setStorage] = useState(100);
  const [runScript, setRunScript] = useState("python /gemini/code/main.py");
  const [maxRunHour, setHours] = useState(24);
  const [advanced, setAdvanced] = useState(false);
  const [jsonText, setJsonText] = useState("");
  const [busy, setBusy] = useState(false);
  const [loadingOpts, setLoadingOpts] = useState(true);
  const [templates, setTemplates] = useState<any[]>([]);
  const [tplName, setTplName] = useState("");
  const [naming, setNaming] = useState(false);
  const [applied, setApplied] = useState("");

  const reloadTemplates = () => api.templates().then((d) => setTemplates((d.templates || []).filter((t: any) => t.kind === kind))).catch(() => setTemplates([]));
  useEffect(() => { if (open) reloadTemplates(); }, [open, kind]); // eslint-disable-line

  // 用一份既有配置预填表单（复制作业 / 套用模板 / 从节点方块进入都走这里）
  const applyPreset = (v: any) => {
    if (!v) return;
    const role = (v.taskroles && v.taskroles[0]) || {};
    if (v.jobName || v.jobenvName) setName(String(v.jobName || v.jobenvName));
    if (v.spaceId) setSpace(String(v.spaceId));
    if (v.projectId) setProjectId(String(v.projectId));
    if (v.rsgroupId) setRsgroupId(String(v.rsgroupId));
    if (v.imageId !== undefined) setImageId(String(v.imageId));
    if (v.maxRunHour) setHours(Number(v.maxRunHour));
    if (!v.taskroles) return;
    if (role.cpu) setCpu(Number(role.cpu));
    if (role.memory) setMemory(Math.max(1, Math.round(Number(role.memory) / 1024)));
    if (role.storage) setStorage(Math.max(1, Math.round(Number(role.storage) / 1024)));
    const g = Number(role.gpu) || 0;
    setUseGpu(g > 0);
    if (g > 0) setGpu(g);
    if (role.gpuType) setGpuType(String(role.gpuType));
    if (role.runScript) setRunScript(String(role.runScript));
  };
  useEffect(() => { if (open && initial) applyPreset(initial); }, [open, initial]); // eslint-disable-line

  useEffect(() => {
    if (!open) return;
    setLoadingOpts(true);
    (async () => {
      const oneSpace = { ...state, spaces: spaces.filter((s) => s.id === space) } as AppState;
      const [prjPer, idle] = await Promise.all([
        eachSpace(oneSpace, (sp) => pcall("GET", "core", "/project/list", { pageNum: 1, pageSize: 500, accessType: 1 }, sp.id)).catch(() => [] as any[]),
        api.idle().catch(() => ({ groups: [] })),
      ]);
      const prj = (prjPer as any[]).flatMap(({ data }: any) => (data.data?.projectList || data.data?.list || []));
      const grp = (idle.groups || []).filter((g: any) => !g.spaceId || g.spaceId === space);
      setProjects(prj); setGroups(grp);
      // 已选（预设或手选）的项目/资源组仍在列表里就保留，否则才回落到第一个
      setProjectId((cur) => prj.some((p: any) => String(field(p, ["projectId", "id"], "")) === cur) ? cur : (prj[0] ? String(field(prj[0], ["projectId", "id"], "")) : ""));
      setRsgroupId((cur) => grp.some((g: any) => String(g.rsgroupId) === cur) ? cur : (grp[0] ? String(grp[0].rsgroupId) : ""));
      setLoadingOpts(false);
    })();
  }, [open, space]); // eslint-disable-line

  useEffect(() => {
    if (!open) return;
    api.images({ space, size: 500 }).then((d) => setRepos(d.data?.imageList || [])).catch(() => setRepos([]));
  }, [open, space]);

  useEffect(() => {
    if (!repoId) { setTags([]); return; }
    pcall("GET", "core", "/imageRepository/detail/" + repoId, {}, space)
      .then((d) => { const l = d.data?.imageList || []; setTags(l); if (l[0]) setImageId(String(field(l[0], ["imageId"], ""))); })
      .catch(() => setTags([]));
  }, [repoId, space]);

  const group = groups.find((x) => String(x.rsgroupId) === rsgroupId);
  const gpuTypes = useMemo(() => asList(group?.capacity?.gpuTypes), [group]);
  // 纯 CPU 资源组只能提交 CPU 作业；从纯 CPU 组切回 GPU 组时恢复成 GPU
  const hadGpu = useRef(true);
  useEffect(() => {
    if (!group) return;
    if (!gpuTypes.length) setUseGpu(false);
    else if (!hadGpu.current) setUseGpu(true);
    hadGpu.current = gpuTypes.length > 0;
    if (gpuTypes.length && !gpuTypes.includes(gpuType)) setGpuType(gpuTypes[0]);
  }, [gpuTypes, group]); // eslint-disable-line
  const groupFree = Number(group?.idle?.gpuWholeCardsFree) || 0;
  const groupMaxOne = Number(group?.idle?.gpuMaxOnOneNode) || 0;

  const payload = useMemo(() => {
    const spec = { cpu, memory: memory * 1024, storage: storage * 1024, gpu: useGpu ? gpu : 0, gpuType: useGpu ? gpuType : "" };
    return {
      jobName: name, trainType: 1, spaceId: space, projectId, rsgroupId,
      description: "submitted by qd web", imageId: Number(imageId) || 0, maxRunHour, maxRetryCount: 0,
      datasetInData: [], preInData: { dataType: 0, dataPath: "", dataBucket: "" }, services: [],
      taskroles: [{ instance: 1, runScript, customEnv: "", ...spec }],
    };
  }, [name, space, projectId, rsgroupId, imageId, useGpu, gpu, gpuType, cpu, memory, storage, runScript, maxRunHour]);
  useEffect(() => { if (!advanced) setJsonText(JSON.stringify(payload, null, 2)); }, [payload, advanced]);

  // 成员的剩余可用卡数 = 三个维度（总数 / 本空间 / 本资源组）剩余的最小值；服务端会再校验一次。
  const me = getMe();
  const limited = me.required && !isAdmin();
  const left = (lim?: number, used?: number) => Math.max(0, (lim || 0) - (used || 0));
  const remaining = limited ? Math.min(
    left(me.quota?.total, me.usage?.total),
    left(me.quota?.spaces?.[space], me.usage?.spaces?.[space]),
    left(me.quota?.groups?.[rsgroupId], me.usage?.groups?.[rsgroupId]),
  ) : Infinity;
  const needGpu = useGpu ? Math.max(0, gpu) : 0;
  const overQuota = !advanced && limited && needGpu > remaining;
  const missing = !advanced && (!name.trim() ? "填写作业名称" : !projectId ? "选择项目" : !rsgroupId ? "选择资源组" : !Number(imageId) ? "选择镜像" : !runScript.trim() ? "填写运行命令" : "");

  const submit = async () => {
    let body: any;
    try { body = advanced ? JSON.parse(jsonText) : payload; }
    catch (e: any) { return toast("JSON 语法错误：" + e.message, true); }
    if (!body.projectId) return toast("请选择项目", true);
    if (!body.rsgroupId) return toast("请选择资源组", true);
    setBusy(true);
    try {
      const r = await qpost("/api/job/submit", body);
      toast("已提交作业 " + (r.data?.jobId || body.jobName));
      dropCache("jobs");
      dropCache("overview");
      onDone(); onClose();
    } catch (e: any) { toast(e.message, true); }
    finally { setBusy(false); }
  };

  const saveTemplate = async () => {
    const n = tplName.trim();
    if (!n) return toast("先填模板名", true);
    try { await api.saveTemplate(n, kind, payload); setTplName(""); setNaming(false); setApplied(n); await reloadTemplates(); toast("已保存模板 " + n); }
    catch (e: any) { toast(e.message, true); }
  };
  const useTemplate = (t: any) => { applyPreset(t.payload); setApplied(t.name); toast("已套用模板 " + t.name); };
  const dropTemplate = async (n: string) => {
    if (!window.confirm(`删除模板「${n}」？`)) return;
    try { await api.deleteTemplate(n); if (applied === n) setApplied(""); await reloadTemplates(); toast("已删除模板 " + n); }
    catch (e: any) { toast(e.message, true); }
  };

  const canSubmit = !busy && !overQuota && !missing;
  useEffect(() => {
    if (!open) return;
    const h = (e: KeyboardEvent) => { if ((e.ctrlKey || e.metaKey) && e.key === "Enter" && canSubmit) { e.preventDefault(); submit(); } };
    window.addEventListener("keydown", h);
    return () => window.removeEventListener("keydown", h);
  });
  const prefix = state.username ? `${state.username}-` : "";
  const prefixed = !!prefix && !name.startsWith(prefix);
  const quick = [1, 2, 4, 8].filter((n) => !groupMaxOne || n <= Math.max(groupMaxOne, 1));
  const summary = `${useGpu ? `${gpu} × ${gpuType || "GPU"}` : "仅 CPU"} · ${cpu} 核 · ${memory} GiB 内存 · 最长 ${maxRunHour} 小时`;

  return (
    <Drawer open={open} onClose={onClose} title="提交作业" sub={groupLabel ? `资源组 ${groupLabel}` : undefined} width="max-w-2xl"
      footer={!loadingOpts && (
        <div className="flex items-center gap-3">
          <div className="min-w-0 flex-1">
            <div className="text-[13px] font-medium text-ink truncate" data-testid="job-summary">{advanced ? "按 JSON 提交" : summary}</div>
            <div className={"text-[11px] truncate mt-0.5 " + (overQuota || missing ? "text-danger" : "text-ink-faint")}>
              {overQuota ? `GPU 配额不足：还能用 ${remaining} 卡，本次需要 ${needGpu} 卡` : missing ? `还需要${missing}` :
                limited ? `本次占用 ${needGpu} 卡，提交后剩余 ${remaining - needGpu} 卡` : <>提交后在作业列表查看进度 · <span className="font-mono">Ctrl+Enter</span> 提交</>}
            </div>
          </div>
          <button className="btn" onClick={onClose}>取消</button>
          <button className="btn btn-pri min-w-[96px]" onClick={submit} disabled={!canSubmit}>{busy && <Loader2 size={14} className="animate-spin" />}提交作业</button>
        </div>
      )}>
      {loadingOpts ? <div className="flex items-center gap-2 text-ink-faint text-[13px] p-6"><Loader2 size={14} className="animate-spin" />读取项目和资源组…</div> : (
        <div className="min-h-full bg-canvas">
          <div className="sticky top-0 z-10 bg-white/90 backdrop-blur border-b border-line-soft">
            <div className="flex items-center gap-2 h-12 px-6">
              <div className="segmented w-36">
                <button aria-pressed={!advanced} onClick={() => setAdvanced(false)}>表单</button>
                <button aria-pressed={advanced} onClick={() => setAdvanced(true)}>JSON</button>
              </div>
              <div className="flex-1" />
              {naming ? (
                <form className="flex items-center gap-1.5 animate-fade" onSubmit={(e) => { e.preventDefault(); saveTemplate(); }}>
                  <input autoFocus className="field field-sm w-44 text-[12px]" placeholder="给模板起个名字" aria-label="模板名" value={tplName}
                    onChange={(e) => setTplName(e.target.value)} onKeyDown={(e) => { if (e.key === "Escape") { e.stopPropagation(); setNaming(false); } }} />
                  <button type="submit" className="btn btn-pri btn-mini" aria-label="保存模板" disabled={!tplName.trim()}><Check size={12} />保存</button>
                  <button type="button" className="btn btn-ghost btn-mini btn-icon" aria-label="取消保存模板" onClick={() => setNaming(false)}><X size={13} /></button>
                </form>
              ) : isAdmin() && (
                <button className="btn btn-mini" onClick={() => { setTplName(applied); setNaming(true); }}><Save size={12} />存为模板</button>
              )}
            </div>
            {templates.length > 0 && (
              <div className="flex items-center gap-1.5 flex-wrap px-6 pb-2.5" role="list" aria-label="作业模板">
                <span className="text-[11px] text-ink-faint mr-1">模板</span>
                {templates.map((t) => (
                  <span key={t.name} role="listitem" className={"group inline-flex items-center h-7 pl-3 pr-1 gap-0.5 rounded-full border text-[12px] transition-colors " +
                    (applied === t.name ? "bg-brand-50 border-brand/30 text-brand font-medium" : "bg-white border-line text-ink-soft hover:text-ink hover:border-[#CDD3DC]")}>
                    <button className="max-w-[160px] truncate" title={`套用 ${t.name}`} onClick={() => useTemplate(t)}>{t.name}</button>
                    {isAdmin() ? <button aria-label={`删除模板 ${t.name}`} title="删除模板"
                      className="grid place-items-center w-5 h-5 rounded-full text-ink-faint opacity-0 group-hover:opacity-100 focus:opacity-100 hover:bg-danger-50 hover:text-danger"
                      onClick={() => dropTemplate(t.name)}><X size={11} /></button> : <span className="w-2" />}
                  </span>
                ))}
              </div>
            )}
          </div>

          {advanced ? (
            <div className="p-6">
              <textarea className="field bg-white" aria-label="作业 JSON" style={{ height: 480, resize: "vertical" }} value={jsonText} onChange={(e) => setJsonText(e.target.value)} spellCheck={false} />
              <p className="hint">直接编辑平台的作业定义。切回表单会丢弃这里的修改。</p>
            </div>
          ) : (
            <div className="p-6 space-y-4">
              <Section icon={FileText} tone="brand" title="作业" desc="名称、所属空间与项目、运行镜像">
                <Field label="名称" htmlFor="cf-name" hint={prefixed ? <>提交时自动加上平台账号前缀，实际名称 <span className="font-mono text-ink-soft">{prefix}{name || "…"}</span></> : undefined}>
                  <div className="flex items-stretch h-9 rounded-ctl border border-line bg-white shadow-ctl transition-all hover:border-[#CDD3DC] focus-within:border-brand focus-within:ring-[3px] focus-within:ring-brand/15 overflow-hidden">
                    {prefixed && <span className="grid place-items-center px-3 bg-canvas border-r border-line text-[12px] font-mono text-ink-faint select-none">{prefix}</span>}
                    <input id="cf-name" className="flex-1 min-w-0 px-3 bg-transparent outline-none text-[13px]" value={name} onChange={(e) => setName(e.target.value)} placeholder="作业名称" />
                  </div>
                </Field>
                <div className="grid grid-cols-2 gap-4">
                  <Field label="空间" htmlFor="cf-space">
                    <select id="cf-space" className="field" value={space} onChange={(e) => setSpace(e.target.value)}>{spaces.map((s) => <option key={s.id} value={s.id}>{s.name}</option>)}</select>
                  </Field>
                  <Field label="项目" htmlFor="cf-project">
                    <select id="cf-project" className="field" value={projectId} onChange={(e) => setProjectId(e.target.value)} disabled={!projects.length}>
                      {projects.length ? projects.map((p) => { const id = String(field(p, ["projectId", "id"], "")); return <option key={id} value={id}>{field(p, ["projectName", "name"])}</option>; }) : <option value="">这个空间下没有你的项目</option>}
                    </select>
                  </Field>
                </div>
                <Field label="镜像" htmlFor="cf-repo" hint={!repos.length ? "这个空间下没有可用的镜像仓库" : undefined}>
                  <div className="grid grid-cols-[minmax(0,1fr)_180px] gap-2">
                    <select id="cf-repo" className="field" value={repoId} onChange={(e) => setRepoId(e.target.value)}>
                      <option value="">选择镜像仓库</option>
                      {repos.map((r) => <option key={r.imageRepositoryId} value={r.imageRepositoryId}>{field(r, ["originalName", "repositoryName"])}</option>)}
                    </select>
                    <select className="field" aria-label="镜像版本" value={imageId} onChange={(e) => setImageId(e.target.value)} disabled={!tags.length}>
                      {tags.length ? tags.map((t) => <option key={field(t, ["imageId"])} value={String(field(t, ["imageId"]))}>{field(t, ["imageTag"])}</option>) : <option value="">{repoId ? "读取版本…" : "先选仓库"}</option>}
                    </select>
                  </div>
                </Field>
              </Section>

              <Section icon={Cpu} tone="teal" title="资源" desc="在哪个资源组运行，要多少算力">
                <Field label="资源组" htmlFor="cf-group" hint={group ? (gpuTypes.length ? <>空闲 <b className="text-ink-soft font-medium">{groupFree}</b> 卡，单节点最多 <b className="text-ink-soft font-medium">{groupMaxOne}</b> 卡，型号 {gpuTypes.join("、")}</> : "纯 CPU 资源组，只能提交 CPU 作业") : undefined}>
                  <select id="cf-group" className="field" value={rsgroupId} onChange={(e) => setRsgroupId(e.target.value)}>
                    {groups.map((g) => {
                      const gpuN = Number(g.capacity?.gpuCount) || 0;
                      return <option key={g.rsgroupId} value={String(g.rsgroupId)}>{g.rsgroupName || g.rsgroupId}{gpuN ? `（空闲 ${Number(g.idle?.gpuWholeCardsFree) || 0} / ${gpuN} 卡）` : "（仅 CPU）"}</option>;
                    })}
                  </select>
                </Field>
                <Field label="算力">
                  <div className="segmented w-40">
                    <button aria-pressed={!useGpu} onClick={() => setUseGpu(false)}>CPU</button>
                    <button aria-pressed={useGpu} onClick={() => setUseGpu(true)} disabled={!gpuTypes.length} title={gpuTypes.length ? undefined : "这个资源组没有 GPU"}>GPU</button>
                  </div>
                </Field>
                {useGpu && gpuTypes.length > 0 && (
                  <div className="grid grid-cols-[minmax(0,1fr)_auto] gap-4 items-start animate-fade">
                    <Field label="GPU 型号">
                      <select className="field" aria-label="GPU 型号" value={gpuType} onChange={(e) => setGpuType(e.target.value)}>
                        {gpuTypes.map((t) => <option key={t} value={t}>{t}</option>)}
                      </select>
                    </Field>
                    <Field label="卡数" hint={limited ? <span className={overQuota ? "text-danger" : ""}>你的配额在这里还能用 {remaining} 卡</span>
                      : groupMaxOne > 0 && gpu > groupMaxOne ? <span className="text-warn">单节点最多只有 {groupMaxOne} 张空闲卡，作业会排队</span> : undefined}>
                      <div className="flex items-center gap-1.5">
                        <NumField label="GPU 卡数" value={gpu} onChange={setGpu} min={1} max={64} unit="卡" className="w-24" />
                        <div className="flex gap-1">
                          {quick.map((n) => <button key={n} type="button" className={"btn btn-mini btn-icon tabular-nums " + (gpu === n ? "border-brand/40 bg-brand-50 text-brand" : "")} onClick={() => setGpu(n)} aria-label={`${n} 卡`}>{n}</button>)}
                        </div>
                      </div>
                    </Field>
                  </div>
                )}
                <div className="grid grid-cols-3 gap-4">
                  <Field label="CPU"><NumField label="CPU 核数" value={cpu} onChange={setCpu} unit="核" max={512} /></Field>
                  <Field label="内存"><NumField label="内存 GiB" value={memory} onChange={setMemory} unit="GiB" max={4096} /></Field>
                  <Field label="存储"><NumField label="存储 GiB" value={storage} onChange={setStorage} unit="GiB" max={10240} /></Field>
                </div>
              </Section>

              <Section icon={TerminalSquare} tone="amber" title="运行" desc="容器启动后执行的命令与运行时长上限">
                <Field label="运行命令" htmlFor="cf-cmd">
                  <textarea id="cf-cmd" className="field" rows={4} style={{ resize: "vertical" }} value={runScript} onChange={(e) => setRunScript(e.target.value)} spellCheck={false} />
                </Field>
                <Field label="最长运行" hint="超时后平台会自动停止作业">
                  <NumField id="cf-hours" label="最长运行小时" value={maxRunHour} onChange={setHours} unit="小时" max={720} className="w-40" />
                </Field>
              </Section>
            </div>
          )}
        </div>
      )}
    </Drawer>
  );
}
