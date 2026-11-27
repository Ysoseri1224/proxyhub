import { useEffect, useMemo, useRef, useState, type ReactNode } from "react";
import {
  verifyProfiles,
  verifyRun,
  verifyProgress,
  verifyLatest,
  verifyCurrentExit,
  verifyTrace,
  verifySampleNow,
  verifyScheduleGet,
  verifyScheduleSet,
  verifyPeakStats,
  type VProfile,
  type VReport,
  type VProgress,
  type NodeResult,
  type Verdict,
  type ExitView,
  type Hop,
  type VSchedule,
  type PeakStat,
  type IpIntel,
  type Purity,
  type AiAccess,
} from "../api";
import { getErrorMessage } from "../util";

const VERDICT: Record<Verdict, { label: string; cls: string; hint: string }> = {
  consistent: { label: "宣传与观测一致", cls: "ok", hint: "能核对的宣传全部对得上(不代表线路等级已被证明)" },
  mismatch: { label: "不一致", cls: "err", hint: "至少一项宣传与观测相反" },
  unverified: { label: "未独立核实", cls: "warn", hint: "有宣传无法用公开观测核对,例如专线、运营商优化、原生 IP" },
  unreachable: { label: "不通", cls: "dim", hint: "经节点访问外网全部失败" },
};
type Filter = "all" | Verdict;
type SortKey = "verdict" | "latency" | "name";
const ORDER: Verdict[] = ["mismatch", "unverified", "consistent", "unreachable"];

function median(v: number[]): number | null {
  if (v.length === 0) return null;
  const s = [...v].sort((a, b) => a - b);
  return s[Math.floor(s.length / 2)];
}
function lossPct(n: NodeResult): number | null {
  return n.http_tries ? Math.round((n.http_fail / n.http_tries) * 100) : null;
}
function purityCls(p: Purity | null): string {
  if (!p) return "dim";
  return p.level === "clean" ? "ok" : p.level === "warn" ? "warn" : p.level === "risky" ? "err" : "dim";
}
function ipLine(i: IpIntel | null): string {
  if (!i) return "—";
  return `${i.cc} · AS${i.asn} ${i.as_name || i.isp}`;
}
function fmtTime(sec: number): string {
  return new Date(sec * 1000).toLocaleString("zh-CN", { hour12: false });
}

interface AiPillsProps {
  ai: AiAccess[];
}
function AiPills({ ai }: AiPillsProps) {
  return (
    <>
      {ai.map((a) => (
        <span key={a.service} className={`pill ${a.ok ? "ok" : "err"}`} title={a.basis}>
          {a.ok ? "✓" : "✕"} {a.service}
        </span>
      ))}
    </>
  );
}

interface IntelGridProps {
  title: string;
  i: IpIntel | null;
  extra?: ReactNode;
}
function IntelGrid({ title, i, extra }: IntelGridProps) {
  return (
    <div style={{ flex: 1, minWidth: 300 }}>
      <div className="sec">{title}</div>
      {i ? (
        <div className="kv">
          <span>IP</span><span className="mono">{i.ip}</span>
          <span>地区</span><span>{i.country} {i.city} ({i.cc})</span>
          <span>ASN</span><span className="mono">AS{i.asn} {i.as_name}</span>
          <span>ISP / 组织</span><span>{i.isp}{i.org && i.org !== i.isp ? ` / ${i.org}` : ""}</span>
          <span>反向解析</span><span className="mono" style={{ wordBreak: "break-all" }}>{i.rdns || "—"}</span>
          <span>机房判定</span><span>{i.datacenter ? `是 · ${i.dc_reason}` : "否"}</span>
          <span>标记</span>
          <span>
            {i.proxy && <span className="pill err">代理</span>} {i.mobile && <span className="pill info">移动网络</span>}{" "}
            {!i.proxy && !i.mobile && <span className="toast">无</span>}
          </span>
          {extra}
        </div>
      ) : (
        <p className="toast">没有拿到数据</p>
      )}
    </div>
  );
}

interface NodeModalProps {
  n: NodeResult;
  peak?: PeakStat;
  onClose: () => void;
}
function NodeModal({ n, peak, onClose }: NodeModalProps) {
  const [hops, setHops] = useState<Hop[] | null>(null);
  const [tracing, setTracing] = useState(false);
  const [err, setErr] = useState("");
  const v = VERDICT[n.verdict];
  const tcp = median(n.entry_tcp_ms);
  const http = median(n.http_ms);

  async function trace(): Promise<void> {
    setTracing(true);
    setErr("");
    try {
      setHops(await verifyTrace(n.entry_ip));
    } catch (e: unknown) {
      setErr(getErrorMessage(e));
    } finally {
      setTracing(false);
    }
  }

  return (
    <div className="modal-backdrop" onClick={onClose}>
      <div className="modal wide" onClick={(e) => e.stopPropagation()}>
        <div className="row">
          <h3 style={{ margin: 0 }}>{n.name}</h3>
          <span className={`pill ${v.cls}`} title={v.hint}>{v.label}</span>
          <span className="spacer" />
          <button onClick={onClose}>关闭</button>
        </div>
        <div className="row" style={{ marginTop: 8, gap: 6 }}>
          <span className="pill dim">{n.ptype}</span>
          <span className="mono">{n.server}:{n.port}</span>
          {n.claims.map((c) => <span key={c} className="pill info">宣传:{c}</span>)}
        </div>
        {n.error && <p className="toast" style={{ color: "var(--err)" }}>{n.error}</p>}

        <div className="sec">逐项核对</div>
        <table>
          <thead><tr><th>项目</th><th>宣传</th><th>观测</th><th>结论</th></tr></thead>
          <tbody>
            {n.checks.map((c, i) => (
              <tr key={i}>
                <td>{c.item}</td>
                <td>{c.claim}</td>
                <td>{c.observed}{c.note && <div className="toast">{c.note}</div>}</td>
                <td>
                  <span className={`pill ${c.result === "pass" ? "ok" : c.result === "fail" ? "err" : "warn"}`}>
                    {c.result === "pass" ? "符合" : c.result === "fail" ? "不符" : "未核实"}
                  </span>
                </td>
              </tr>
            ))}
            {n.checks.length === 0 && <tr><td colSpan={4} className="toast">节点名里没有可核对的宣传</td></tr>}
          </tbody>
        </table>
        {n.missing.length > 0 && (
          <>
            <div className="sec">还缺的证据 / 提示</div>
            <ul style={{ margin: 0, paddingLeft: 18 }}>
              {n.missing.map((m) => <li key={m} className="toast" style={{ marginBottom: 3 }}>{m}</li>)}
            </ul>
          </>
        )}

        <div className="row" style={{ alignItems: "flex-start", gap: 20 }}>
          <IntelGrid
            title={`入口 · ${n.dns_source}`}
            i={n.entry}
            extra={<><span>本机→入口</span><span>{tcp != null ? `${tcp} ms(TCP 建连中位)` : "连不上"}</span></>}
          />
          <IntelGrid
            title="出口(经节点)"
            i={n.exit}
            extra={
              <>
                <span>ChatGPT 识别</span><span>{n.ai_loc || "—"}</span>
                <span>经节点延迟</span>
                <span>{http != null ? `${http} ms` : "—"} · 丢包 {lossPct(n) ?? "—"}%({n.http_tries} 次)</span>
              </>
            }
          />
        </div>

        <div className="sec">出口纯净度</div>
        <div className="row" style={{ gap: 6 }}>
          <span className={`pill ${purityCls(n.purity)}`}>{n.purity.label}</span>
          <AiPills ai={n.ai} />
        </div>
        {n.purity.reasons.map((r) => <div key={r} className="toast">· {r}</div>)}
        {n.shared_exit_with.length > 0 && (
          <p className="toast">与 {n.shared_exit_with.length} 个节点共用同一出口:{n.shared_exit_with.join("、")}。它们只是入口不同,切换它们分散不了风控。</p>
        )}

        <div className="sec">晚高峰对比</div>
        {peak ? (
          <div className="kv">
            <span>闲时</span><span>{peak.off_ms ?? "—"} ms · 丢包 {peak.off_loss ?? "—"}% · {peak.off_n} 次采样</span>
            <span>晚高峰</span><span>{peak.peak_ms ?? "—"} ms · 丢包 {peak.peak_loss ?? "—"}% · {peak.peak_n} 次采样</span>
            <span>本地运营商</span><span>{peak.local_isp || "—"}</span>
          </div>
        ) : (
          <p className="toast">还没有采样。开启下方「晚高峰监测」后自动积累。</p>
        )}

        <div className="row" style={{ marginTop: 16 }}>
          <div className="sec" style={{ margin: 0 }}>本机 → 入口 路由追踪</div>
          <span className="spacer" />
          <button onClick={() => void trace()} disabled={tracing || !n.entry_ip}>
            {tracing ? "追踪中(最长约 20 秒)…" : hops ? "重新追踪" : "开始追踪"}
          </button>
        </div>
        {err && <p className="toast" style={{ color: "var(--err)" }}>{err}</p>}
        {hops && (
          <>
            <table style={{ marginTop: 8 }}>
              <thead><tr><th>#</th><th>IP</th><th>延迟</th><th>归属</th></tr></thead>
              <tbody>
                {hops.map((h) => (
                  <tr key={h.n}>
                    <td>{h.n}</td>
                    <td className="mono">{h.ip || "*"}</td>
                    <td>{h.ms != null ? `${h.ms.toFixed(1)} ms` : "—"}</td>
                    <td>{h.private ? <span className="toast">{h.ip ? "内网" : "无响应"}</span> : `${h.cc} · AS${h.asn} ${h.org}`}</td>
                  </tr>
                ))}
              </tbody>
            </table>
            <p className="toast">可见跳点只是响应了探测的设备;跳数少或中间无响应不能证明是专线(MPLS 可隐藏核心跳点)。</p>
          </>
        )}
      </div>
    </div>
  );
}

function ExitCard() {
  const [views, setViews] = useState<ExitView[] | null>(null);
  const [loading, setLoading] = useState(false);
  const [err, setErr] = useState("");
  async function check(): Promise<void> {
    setLoading(true);
    setErr("");
    try {
      setViews(await verifyCurrentExit());
    } catch (e: unknown) {
      setErr(getErrorMessage(e));
    } finally {
      setLoading(false);
    }
  }
  return (
    <div className="card">
      <div className="row">
        <h3 style={{ margin: 0 }}>当前出口纯净度</h3>
        <span className="toast">本机直连 vs 经 Clash 的出口</span>
        <span className="spacer" />
        <button className="primary" onClick={() => void check()} disabled={loading}>{loading ? "检测中…" : "检测"}</button>
      </div>
      {loading && <div className="bar" style={{ marginTop: 12 }}><div style={{ width: "45%" }} /></div>}
      {err && <p className="toast" style={{ color: "var(--err)" }}>{err}</p>}
      {views && !loading && (
        <table style={{ marginTop: 10 }}>
          <tbody>
            {views.map((v) => (
              <tr key={v.label}>
                <td style={{ width: 170 }}><strong>{v.label}</strong></td>
                <td>
                  {v.ip ? (
                    <div className="row" style={{ gap: 6 }}>
                      <span className="mono">{v.ip.ip}</span>
                      <span>{v.ip.country} {v.ip.city}</span>
                      <span className="pill dim">{v.ip.isp}</span>
                      <span className={`pill ${purityCls(v.purity)}`} title={v.purity?.reasons.join("\n")}>{v.purity?.label}</span>
                      {v.ai_loc && <span className="pill dim">ChatGPT 识别 {v.ai_loc}</span>}
                      <AiPills ai={v.ai} />
                    </div>
                  ) : (
                    <span className="toast">—</span>
                  )}
                  <div className="toast">{v.note}</div>
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      )}
    </div>
  );
}

interface PeakCardProps {
  stats: PeakStat[];
  onChanged: () => void;
}
function PeakCard({ stats, onChanged }: PeakCardProps) {
  const [sch, setSch] = useState<VSchedule | null>(null);
  const [busy, setBusy] = useState(false);
  const [msg, setMsg] = useState("");
  useEffect(() => {
    void verifyScheduleGet().then(setSch);
  }, []);
  async function toggle(): Promise<void> {
    if (!sch) return;
    setBusy(true);
    try {
      setSch(await verifyScheduleSet(!sch.loaded));
      setMsg(sch.loaded ? "已关闭定时采样" : "已开启:到点由系统自动采样,ProxyHub 不用开着");
    } catch (e: unknown) {
      setMsg(getErrorMessage(e));
    } finally {
      setBusy(false);
    }
  }
  async function sampleNow(): Promise<void> {
    setBusy(true);
    setMsg("采样中(约 1–2 分钟)…");
    try {
      setMsg(await verifySampleNow());
      setSch(await verifyScheduleGet());
      onChanged();
    } catch (e: unknown) {
      setMsg(getErrorMessage(e));
    } finally {
      setBusy(false);
    }
  }
  const rows = [...stats].sort((a, b) => (b.peak_loss ?? -1) - (a.peak_loss ?? -1));
  return (
    <div className="card">
      <div className="row">
        <h3 style={{ margin: 0 }}>晚高峰监测</h3>
        {sch && <span className={`pill ${sch.loaded ? "ok" : "dim"}`}>{sch.loaded ? "定时采样已开启" : "未开启"}</span>}
        <span className="spacer" />
        <button onClick={() => void sampleNow()} disabled={busy}>立即采样一次</button>
        <button className="primary" onClick={() => void toggle()} disabled={busy || !sch}>{sch?.loaded ? "关闭定时" : "开启定时"}</button>
      </div>
      {sch && (
        <p className="toast" style={{ marginTop: 6 }}>
          采样时刻 {sch.slots.join(" · ")} · 每次每节点 5 次请求 + 3 次入口建连 · 保留 14 天 · 已有 {sch.samples} 条记录
        </p>
      )}
      <div className="toast">{msg}</div>
      {rows.length > 0 ? (
        <table style={{ marginTop: 8 }}>
          <thead>
            <tr><th>节点</th><th>闲时 延迟 / 丢包</th><th>晚高峰 延迟 / 丢包</th><th>变化</th></tr>
          </thead>
          <tbody>
            {rows.map((s) => {
              const d = s.off_ms != null && s.peak_ms != null ? Math.round(s.peak_ms - s.off_ms) : null;
              return (
                <tr key={s.node}>
                  <td>{s.node}</td>
                  <td>{s.off_ms ?? "—"} ms / {s.off_loss ?? "—"}% <span className="toast">({s.off_n})</span></td>
                  <td>{s.peak_ms ?? "—"} ms / {s.peak_loss ?? "—"}% <span className="toast">({s.peak_n})</span></td>
                  <td>{d == null ? "—" : <span className={`pill ${d > 150 || (s.peak_loss ?? 0) > 10 ? "err" : d > 60 ? "warn" : "ok"}`}>{d > 0 ? "+" : ""}{d} ms</span>}</td>
                </tr>
              );
            })}
          </tbody>
        </table>
      ) : (
        <p className="toast">还没有数据。这些数值只描述你的地点、运营商和时段,不存在「低于多少就是专线」的阈值。</p>
      )}
    </div>
  );
}

export function Verify(): ReactNode {
  const [profiles, setProfiles] = useState<VProfile[]>([]);
  const [profile, setProfile] = useState("");
  const [report, setReport] = useState<VReport | null>(null);
  const [prog, setProg] = useState<VProgress | null>(null);
  const [filter, setFilter] = useState<Filter>("all");
  const [sort, setSort] = useState<SortKey>("verdict");
  const [q, setQ] = useState("");
  const [open, setOpen] = useState<NodeResult | null>(null);
  const [stats, setStats] = useState<PeakStat[]>([]);
  const [toast, setToast] = useState("");
  const timer = useRef<number | null>(null);

  const loadStats = (): void => void verifyPeakStats().then(setStats);

  useEffect(() => {
    verifyProfiles()
      .then((ps) => {
        setProfiles(ps);
        setProfile((ps.find((p) => p.current) ?? ps[0])?.uid ?? "");
      })
      .catch((e: unknown) => setToast(getErrorMessage(e)));
    void verifyLatest().then((r) => r && setReport(r));
    loadStats();
    return () => {
      if (timer.current) window.clearInterval(timer.current);
    };
  }, []);

  async function run(): Promise<void> {
    setToast("");
    timer.current = window.setInterval(() => void verifyProgress().then(setProg), 500);
    try {
      setReport(await verifyRun(profile));
      setFilter("all");
    } catch (e: unknown) {
      setToast(getErrorMessage(e));
    } finally {
      if (timer.current) window.clearInterval(timer.current);
      setProg(null);
    }
  }

  const counts = useMemo(() => {
    const c: Record<Filter, number> = { all: 0, consistent: 0, mismatch: 0, unverified: 0, unreachable: 0 };
    report?.nodes.forEach((n) => {
      c.all += 1;
      c[n.verdict] += 1;
    });
    return c;
  }, [report]);

  const rows = useMemo(() => {
    if (!report) return [];
    const kw = q.trim().toLowerCase();
    const list = report.nodes.filter(
      (n) => (filter === "all" || n.verdict === filter) && (!kw || `${n.name} ${n.server} ${n.exit?.ip ?? ""}`.toLowerCase().includes(kw)),
    );
    return list.sort((a, b) => {
      if (sort === "name") return a.name.localeCompare(b.name, "zh");
      if (sort === "latency") return (median(a.http_ms) ?? 1e9) - (median(b.http_ms) ?? 1e9);
      return ORDER.indexOf(a.verdict) - ORDER.indexOf(b.verdict);
    });
  }, [report, filter, sort, q]);

  const running = prog?.running ?? false;
  const pct = prog && prog.total ? Math.round((prog.done / prog.total) * 100) : 8;

  return (
    <div>
      <div className="row">
        <div>
          <h1 className="page-title">节点核验</h1>
          <p className="page-sub" style={{ margin: 0 }}>对照节点名里的宣传与实际观测 · 观测只是线索,证据不足一律记为「未独立核实」</p>
        </div>
        <span className="spacer" />
        <select value={profile} onChange={(e) => setProfile(e.target.value)} disabled={running}>
          {profiles.map((p) => (
            <option key={p.uid} value={p.uid}>{p.name}{p.current ? "(当前)" : ""} · {p.nodes} 个节点</option>
          ))}
        </select>
        <button className="primary" onClick={() => void run()} disabled={running || !profile}>{running ? "核验中…" : "开始核验"}</button>
      </div>

      {running && (
        <div className="card" style={{ marginTop: 16 }}>
          <div className="row"><strong>{prog?.stage}</strong><span className="spacer" />{prog?.total ? `${prog.done} / ${prog.total}` : ""}</div>
          <div className="bar" style={{ marginTop: 8 }}><div style={{ width: `${pct}%` }} /></div>
          <p className="toast">用临时内核逐节点测试,不会切换你 Clash 里正在用的节点。</p>
        </div>
      )}
      <div className="toast" style={{ margin: "8px 0", color: toast ? "var(--err)" : undefined }}>{toast}</div>

      <ExitCard />

      <div className="card">
        {report ? (
          <>
            <div className="row">
              <div className="seg">
                {(["all", ...ORDER] as Filter[]).map((f) => (
                  <button key={f} className={filter === f ? "on" : ""} onClick={() => setFilter(f)} title={f === "all" ? "" : VERDICT[f].hint}>
                    {f === "all" ? "全部" : VERDICT[f].label} {counts[f]}
                  </button>
                ))}
              </div>
              <span className="spacer" />
              <input placeholder="搜索节点 / 服务器 / 出口 IP" value={q} onChange={(e) => setQ(e.target.value)} style={{ width: 200 }} />
              <select value={sort} onChange={(e) => setSort(e.target.value as SortKey)}>
                <option value="verdict">按判定</option>
                <option value="latency">按延迟</option>
                <option value="name">按名称</option>
              </select>
            </div>
            <table style={{ marginTop: 10 }}>
              <thead>
                <tr><th>节点</th><th>入口</th><th>出口</th><th>纯净度</th><th>延迟 / 丢包</th><th>判定</th></tr>
              </thead>
              <tbody>
                {rows.map((n) => {
                  const ms = median(n.http_ms);
                  const v = VERDICT[n.verdict];
                  return (
                    <tr key={n.name} className="click" onClick={() => setOpen(n)}>
                      <td>
                        {n.name}
                        <div className="toast">{n.ptype}{n.shared_exit_with.length > 0 ? ` · 与 ${n.shared_exit_with.length} 个节点共用出口` : ""}</div>
                      </td>
                      <td className="mono">{ipLine(n.entry)}</td>
                      <td className="mono">
                        {n.exit ? `${n.exit.ip} · ${n.exit.cc}` : "—"}
                        {n.exit && n.ai_loc && n.ai_loc !== n.exit.cc && <div className="toast">ChatGPT 识别为 {n.ai_loc}</div>}
                      </td>
                      <td><span className={`pill ${purityCls(n.purity)}`}>{n.purity.label}</span></td>
                      <td>{ms != null ? `${ms} ms` : "—"} <span className="toast">/ {lossPct(n) ?? "—"}%</span></td>
                      <td><span className={`pill ${v.cls}`} title={v.hint}>{v.label}</span></td>
                    </tr>
                  );
                })}
                {rows.length === 0 && <tr><td colSpan={6} className="toast">没有符合条件的节点</td></tr>}
              </tbody>
            </table>
            <p className="toast" style={{ marginTop: 10 }}>
              {report.profile_name} · {fmtTime(report.at)} · 本地出口 {report.local_ip || "未知"} {report.local_isp}
              {report.skipped_info.length > 0 && ` · 跳过 ${report.skipped_info.length} 条信息类条目(${report.skipped_info.join("、")})`}
            </p>
            {report.notes.map((m) => <p key={m} className="toast" style={{ color: "var(--warn)" }}>⚠ {m}</p>)}
          </>
        ) : (
          <p className="toast">还没有核验记录。选好订阅后点「开始核验」,约 2 分钟。</p>
        )}
      </div>

      <PeakCard stats={stats} onChanged={loadStats} />

      {open && <NodeModal n={open} peak={stats.find((s) => s.node === open.name)} onClose={() => setOpen(null)} />}
    </div>
  );
}
