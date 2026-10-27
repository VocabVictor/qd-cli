import { useEffect, useState, useMemo, ReactNode } from "react";
import { Loader2, Save } from "lucide-react";
import { api, qpost, pcall, eachSpace, spacesOf, AppState, asList, field, getMe, isAdmin } from "../lib/api";
import { Drawer } from "./Drawer";
import { dropCache } from "../lib/cache";
import { useToast } from "./ui";

type Kind = "job";

/** 左标签右控件的一行：标签列固定宽度，密度高、扫读快。 */
function Row({ label, htmlFor, hint, children }: { label: string; htmlFor?: string; hint?: ReactNode; children: ReactNode }) {
  return (
    <div className="grid grid-cols-[96px_minmax(0,1fr)] items-start gap-3 py-1.5">
      <label htmlFor={htmlFor} className="text-[13px] text-ink-soft pt-1.5">{label}</label>
      <div className="min-w-0">{children}{hint && <div className="text-[11px] text-ink-faint mt-1">{hint}</div>}</div>
    </div>
  );
}

function Section({ title, children }: { title: string; children: ReactNode }) {
  return (
    <section className="px-5 py-3 border-b border-line">
      <h3 className="text-[12px] font-medium text-ink-faint mb-1">{title}</h3>
      {children}
    </section>
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
    pcall("GET", "core", "/imageRepository/list", { spaceId: space, source: "current", keyWords: "", pageNum: 1, pageSize: 500 }, space)
      .then((d) => setRepos(d.data?.imageList || [])).catch(() => setRepos([]));
  }, [open, space]);

  useEffect(() => {
    if (!repoId) { setTags([]); return; }
    pcall("GET", "core", "/imageRepository/detail/" + repoId, {}, space)
      .then((d) => { const l = d.data?.imageList || []; setTags(l); if (l[0]) setImageId(String(field(l[0], ["imageId"], ""))); })
      .catch(() => setTags([]));
  }, [repoId, space]);

  const group = groups.find((x) => String(x.rsgroupId) === rsgroupId);
  const gpuTypes = useMemo(() => asList(group?.capacity?.gpuTypes), [group]);
  useEffect(() => { if (gpuTypes.length && !gpuTypes.includes(gpuType)) setGpuType(gpuTypes[0]); }, [gpuTypes]); // eslint-disable-line
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
    try { await api.saveTemplate(n, kind, payload); setTplName(""); await reloadTemplates(); toast("已保存模板 " + n); }
    catch (e: any) { toast(e.message, true); }
  };

  const num = (set: (n: number) => void, min = 0) => (e: React.ChangeEvent<HTMLInputElement>) => set(Math.max(min, Number(e.target.value) || 0));
  const summary = `${useGpu ? `${gpu} × ${gpuType || "GPU"}，` : "仅 CPU，"}${cpu} 核，${memory} GiB 内存，最长 ${maxRunHour} 小时`;

  return (
    <Drawer open={open} onClose={onClose} title="提交作业" sub={groupLabel ? `资源组 ${groupLabel}` : undefined} width="max-w-2xl"
      actions={templates.length > 0 && !loadingOpts ? (
        <select className="field w-40 h-7 text-[12px]" value="" aria-label="套用模板" onChange={(e) => {
          const t = templates.find((x) => x.name === e.target.value);
          if (t) { applyPreset(t.payload); toast("已套用模板 " + t.name); }
        }}>
          <option value="">套用模板…</option>
          {templates.map((t) => <option key={t.name} value={t.name}>{t.name}</option>)}
        </select>
      ) : undefined}
      footer={!loadingOpts && (
        <div className="flex items-center gap-3">
          <div className="min-w-0 flex-1">
            <div className="text-[12px] text-ink truncate" data-testid="job-summary">{advanced ? "按 JSON 提交" : summary}</div>
            <div className={"text-[11px] truncate " + (overQuota || missing ? "text-danger" : "text-ink-faint")}>
              {overQuota ? `GPU 配额不足：还能用 ${remaining} 卡，本次需要 ${needGpu} 卡` : missing ? `还需要${missing}` :
                limited ? `本次占用 ${needGpu} 卡，提交后剩余 ${remaining - needGpu} 卡` : "提交后在作业列表查看进度"}
            </div>
          </div>
          <button className="btn" onClick={onClose}>取消</button>
          <button className="btn btn-pri" onClick={submit} disabled={busy || overQuota || !!missing}>{busy && <Loader2 size={14} className="animate-spin" />}提交作业</button>
        </div>
      )}>
      {loadingOpts ? <div className="flex items-center gap-2 text-ink-faint text-[13px] p-5"><Loader2 size={14} className="animate-spin" />读取项目和资源组…</div> : (
        <>
          <div className="flex items-center gap-1 h-10 px-4 border-b border-line">
            <div className="segmented w-44">
              <button aria-pressed={!advanced} onClick={() => setAdvanced(false)}>表单</button>
              <button aria-pressed={advanced} onClick={() => setAdvanced(true)}>JSON</button>
            </div>
            <div className="flex-1" />
            <input className="field w-36 h-7 text-[12px]" placeholder="另存为模板名" aria-label="模板名" value={tplName} onChange={(e) => setTplName(e.target.value)} />
            <button className="btn btn-ghost btn-mini" onClick={saveTemplate}><Save size={12} />存为模板</button>
            {templates.length > 0 && (
              <select className="field w-28 h-7 text-[12px]" value="" aria-label="删除模板" onChange={async (e) => {
                const n = e.target.value; if (!n) return;
                try { await api.deleteTemplate(n); await reloadTemplates(); toast("已删除模板 " + n); } catch (err: any) { toast(err.message, true); }
              }}>
                <option value="">删除模板…</option>
                {templates.map((t) => <option key={t.name} value={t.name}>{t.name}</option>)}
              </select>
            )}
          </div>

          {advanced ? (
            <div className="p-5">
              <textarea className="field" aria-label="作业 JSON" style={{ height: 460, resize: "vertical" }} value={jsonText} onChange={(e) => setJsonText(e.target.value)} />
              <p className="text-[11px] text-ink-faint mt-2">直接编辑平台的作业定义。切回表单会丢弃这里的修改。</p>
            </div>
          ) : (
            <>
              <Section title="作业">
                <Row label="名称" htmlFor="cf-name"><input id="cf-name" className="field" value={name} onChange={(e) => setName(e.target.value)} /></Row>
                <Row label="空间" htmlFor="cf-space">
                  <select id="cf-space" className="field" value={space} onChange={(e) => setSpace(e.target.value)}>{spaces.map((s) => <option key={s.id} value={s.id}>{s.name}</option>)}</select>
                </Row>
                <Row label="项目" htmlFor="cf-project">
                  <select id="cf-project" className="field" value={projectId} onChange={(e) => setProjectId(e.target.value)}>
                    {projects.length ? projects.map((p) => { const id = String(field(p, ["projectId", "id"], "")); return <option key={id} value={id}>{field(p, ["projectName", "name"])}</option>; }) : <option value="">这个空间下没有你的项目</option>}
                  </select>
                </Row>
                <Row label="镜像" htmlFor="cf-repo">
                  <div className="grid grid-cols-[minmax(0,1fr)_160px] gap-2">
                    <select id="cf-repo" className="field" value={repoId} onChange={(e) => setRepoId(e.target.value)}>
                      <option value="">选择镜像仓库</option>
                      {repos.map((r) => <option key={r.imageRepositoryId} value={r.imageRepositoryId}>{field(r, ["originalName", "repositoryName"])}</option>)}
                    </select>
                    <select className="field" aria-label="镜像版本" value={imageId} onChange={(e) => setImageId(e.target.value)} disabled={!tags.length}>
                      {tags.length ? tags.map((t) => <option key={field(t, ["imageId"])} value={String(field(t, ["imageId"]))}>{field(t, ["imageTag"])}</option>) : <option value="">版本</option>}
                    </select>
                  </div>
                </Row>
              </Section>

              <Section title="资源">
                <Row label="资源组" htmlFor="cf-group" hint={group ? (Number(group.capacity?.gpuCount) ? `空闲 ${groupFree} 卡，单节点最多 ${groupMaxOne} 卡` : "纯 CPU 资源组") : undefined}>
                  <select id="cf-group" className="field" value={rsgroupId} onChange={(e) => setRsgroupId(e.target.value)}>
                    {groups.map((g) => <option key={g.rsgroupId} value={String(g.rsgroupId)}>{g.rsgroupName || g.rsgroupId}</option>)}
                  </select>
                </Row>
                <Row label="算力">
                  <div className="flex items-center gap-2">
                    <div className="segmented w-36">
                      <button aria-pressed={!useGpu} onClick={() => setUseGpu(false)}>CPU</button>
                      <button aria-pressed={useGpu} onClick={() => setUseGpu(true)} disabled={!gpuTypes.length}>GPU</button>
                    </div>
                    {useGpu && <>
                      <select className="field flex-1" aria-label="GPU 型号" value={gpuType} onChange={(e) => setGpuType(e.target.value)}>
                        {gpuTypes.length ? gpuTypes.map((t) => <option key={t} value={t}>{t}</option>) : <option value="">该资源组没有 GPU</option>}
                      </select>
                      <input className="field w-20 text-right" type="number" min={1} aria-label="GPU 卡数" value={gpu} onChange={num(setGpu, 1)} />
                      <span className="text-[13px] text-ink-soft">卡</span>
                    </>}
                  </div>
                  {useGpu && limited && <div className={"text-[11px] mt-1 " + (overQuota ? "text-danger" : "text-ink-faint")}>你的配额在这里还能用 {remaining} 卡</div>}
                  {useGpu && groupMaxOne > 0 && gpu > groupMaxOne && <div className="text-[11px] mt-1 text-warn">单节点最多只有 {groupMaxOne} 张空闲卡，作业会排队等待</div>}
                </Row>
                <Row label="CPU / 内存">
                  <div className="flex items-center gap-2">
                    <input className="field w-20 text-right" type="number" min={1} aria-label="CPU 核数" value={cpu} onChange={num(setCpu, 1)} /><span className="text-[13px] text-ink-soft">核</span>
                    <input className="field w-20 text-right ml-3" type="number" min={1} aria-label="内存 GiB" value={memory} onChange={num(setMemory, 1)} /><span className="text-[13px] text-ink-soft">GiB 内存</span>
                    <input className="field w-20 text-right ml-3" type="number" min={1} aria-label="存储 GiB" value={storage} onChange={num(setStorage, 1)} /><span className="text-[13px] text-ink-soft">GiB 存储</span>
                  </div>
                </Row>
              </Section>

              <Section title="运行">
                <Row label="运行命令" htmlFor="cf-cmd">
                  <textarea id="cf-cmd" className="field" rows={3} style={{ resize: "vertical" }} value={runScript} onChange={(e) => setRunScript(e.target.value)} />
                </Row>
                <Row label="最长运行" htmlFor="cf-hours" hint="超时后平台会自动停止作业">
                  <div className="flex items-center gap-2"><input id="cf-hours" className="field w-20 text-right" type="number" min={1} value={maxRunHour} onChange={num(setHours, 1)} /><span className="text-[13px] text-ink-soft">小时</span></div>
                </Row>
              </Section>
            </>
          )}
        </>
      )}
    </Drawer>
  );
}
