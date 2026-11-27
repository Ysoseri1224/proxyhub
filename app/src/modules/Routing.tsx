import { useEffect, useMemo, useState } from "react";
import {
  routingGet,
  routingSave,
  routingReapply,
  routingStatus,
  routingDiagnose,
  routingBackups,
  routingRollback,
  routingImportClash,
  listApps,
  importBookmarks,
  clashGroups,
  launchClash,
  type Rule,
  type AppItem,
  type RoutingStatus,
  type DiagResult,
} from "../api";
import { getErrorMessage, tryParse } from "../util";

type Category = "app" | "domain" | "geosite" | "ipcidr" | "geoip" | "port";
interface CatDef {
  id: Category;
  label: string;
  hint: string;
}
const CATS: CatDef[] = [
  { id: "app", label: "应用 App", hint: "按进程名,如 Google Chrome(需 Tun 模式)" },
  { id: "domain", label: "域名", hint: "如 openai.com(可从书签导入)" },
  { id: "geosite", label: "预置服务集", hint: "如 google / github / netflix" },
  { id: "ipcidr", label: "IP / 网段", hint: "如 1.1.1.1/32" },
  { id: "geoip", label: "地区", hint: "如 CN / US" },
  { id: "port", label: "端口", hint: "如 443" },
];


interface SectionProps {
  cat: CatDef;
  items: { rule: Rule; index: number }[];
  groups: string[];
  apps: AppItem[];
  domains: string[];
  collapsed: boolean;
  onToggle: () => void;
  onAdd: (value: string, target: string) => void;
  onRemove: (index: number) => void;
}

function CategorySection({ cat, items, groups, apps, domains, collapsed, onToggle, onAdd, onRemove }: SectionProps) {
  const [value, setValue] = useState("");
  const [target, setTarget] = useState(groups[0] ?? "DIRECT");
  useEffect(() => {
    if (!groups.includes(target)) setTarget(groups[0] ?? "DIRECT");
  }, [groups]);

  function submit(): void {
    if (!value.trim()) return;
    onAdd(value.trim(), target);
    setValue("");
  }

  return (
    <div className="card" style={{ padding: 0, overflow: "hidden" }}>
      <div
        className="row"
        style={{ padding: "12px 16px", cursor: "default", userSelect: "none" }}
        onClick={onToggle}
      >
        <span style={{ width: 16, color: "var(--text-dim)" }}>{collapsed ? "▸" : "▾"}</span>
        <strong>{cat.label}</strong>
        <span className="pill dim">{items.length}</span>
        <span className="spacer" />
        <span className="toast" style={{ minHeight: 0 }}>{cat.hint}</span>
      </div>
      {!collapsed && (
        <div style={{ padding: "4px 16px 14px", borderTop: "0.5px solid var(--border)" }}>
          {items.map(({ rule, index }) => (
            <div key={index} className="row" style={{ padding: "5px 0", borderBottom: "0.5px solid var(--border)" }}>
              <span className="mono" style={{ flex: 1 }}>{rule.value}</span>
              <span style={{ color: "var(--text-dim)" }}>→</span>
              <span className="pill dim">{rule.target}</span>
              <button className="danger" onClick={() => onRemove(index)}>删</button>
            </div>
          ))}
          <div className="row" style={{ marginTop: 10 }}>
            <input
              list={cat.id === "app" ? "apps" : cat.id === "domain" ? "domains" : undefined}
              placeholder={`添加${cat.label}…`}
              value={value}
              onChange={(e) => setValue(e.target.value)}
              onKeyDown={(e) => e.key === "Enter" && submit()}
              style={{ flex: 1 }}
            />
            <span style={{ color: "var(--text-dim)" }}>→</span>
            <select value={target} onChange={(e) => setTarget(e.target.value)}>
              {groups.map((g) => <option key={g} value={g}>{g}</option>)}
            </select>
            <button className="primary" onClick={submit}>添加</button>
          </div>
          {cat.id === "app" && <datalist id="apps">{apps.map((a) => <option key={a.path} value={a.process}>{a.name}</option>)}</datalist>}
          {cat.id === "domain" && <datalist id="domains">{domains.map((d) => <option key={d} value={d} />)}</datalist>}
        </div>
      )}
    </div>
  );
}

export function Routing() {
  const [rules, setRules] = useState<Rule[]>([]);
  const [groups, setGroups] = useState<string[]>(["🌍 国外流量", "⚡ 自动选择", "DIRECT"]);
  const [apps, setApps] = useState<AppItem[]>([]);
  const [domains, setDomains] = useState<string[]>([]);
  const [showBm, setShowBm] = useState(false);
  const [bmFilter, setBmFilter] = useState("");
  const [backups, setBackups] = useState<string[]>([]);
  const [collapsed, setCollapsed] = useState<Record<string, boolean>>({});
  const [dirty, setDirty] = useState(false);
  const [status, setStatus] = useState<RoutingStatus | null>(null);
  const [diagHost, setDiagHost] = useState("");
  const [diag, setDiag] = useState<DiagResult | null>(null);
  const [toast, setToast] = useState("");

  async function refreshStatus(): Promise<void> {
    try {
      setStatus(await routingStatus());
    } catch {
      // 状态读取失败不致命:面板不显示即可
    }
  }

  async function load(): Promise<void> {
    try {
      const model = tryParse<{ rules: Rule[] }>(await routingGet());
      const rs = model?.rules ?? [];
      setRules(rs);
      setBackups(await routingBackups());
      // 默认:有规则的分类展开,空的收起
      const init: Record<string, boolean> = {};
      CATS.forEach((c) => (init[c.id] = !rs.some((r) => r.category === c.id)));
      setCollapsed(init);
    } catch (error: unknown) {
      setToast(getErrorMessage(error));
    }
  }
  useEffect(() => {
    void load();
    void refreshStatus();
    void clashGroups().then(setGroups).catch(() => undefined);
    // 注意:app 列表懒加载(展开「应用」分类时才扫),避免进页面就卡
  }, []);

  async function reapply(): Promise<void> {
    try {
      setToast(await routingReapply());
      await refreshStatus();
    } catch (error: unknown) {
      setToast(getErrorMessage(error));
    }
  }
  async function runDiagnose(): Promise<void> {
    if (!diagHost.trim()) return;
    try {
      setDiag(await routingDiagnose(diagHost.trim()));
    } catch (error: unknown) {
      setToast(getErrorMessage(error));
    }
  }

  function toggleCat(id: string): void {
    const opening = collapsed[id] !== false;
    setCollapsed((p) => ({ ...p, [id]: !p[id] }));
    if (opening && id === "app" && apps.length === 0) {
      void listApps().then(setApps).catch(() => undefined);
    }
  }

  const byCat = useMemo(() => {
    const m: Record<string, { rule: Rule; index: number }[]> = {};
    CATS.forEach((c) => (m[c.id] = []));
    rules.forEach((rule, index) => m[rule.category]?.push({ rule, index }));
    return m;
  }, [rules]);

  function addRule(category: Category, value: string, target: string): void {
    setRules((prev) => [...prev, { category, value, target }]);
    setDirty(true);
  }
  function removeRule(index: number): void {
    setRules((prev) => prev.filter((_, i) => i !== index));
    setDirty(true);
  }

  async function save(): Promise<void> {
    try {
      const msg = await routingSave(JSON.stringify({ rules }));
      setToast(msg);
      setDirty(false);
      await load();
      await refreshStatus();
    } catch (error: unknown) {
      setToast(getErrorMessage(error));
    }
  }
  async function importClash(): Promise<void> {
    try {
      const model = tryParse<{ rules: Rule[] }>(await routingImportClash());
      setRules(model?.rules ?? []);
      setDirty(true);
      setToast("已从 Clash 导入(确认后点保存写回)");
    } catch (error: unknown) {
      setToast(getErrorMessage(error));
    }
  }
  async function loadBookmarks(): Promise<void> {
    try {
      const d = await importBookmarks();
      setDomains(d);
      setShowBm(true);
      setToast(d.length ? `从浏览器书签提取 ${d.length} 个域名,点下方域名即可加为规则` : "未读到书签(Chrome/Edge/Brave/Safari)");
    } catch (error: unknown) {
      setToast(getErrorMessage(error));
    }
  }

  return (
    <div>
      <div className="row">
        <div>
          <h1 className="page-title">网络分流</h1>
          <p className="page-sub" style={{ margin: 0 }}>按分类编排"谁走哪条分流" · 去向取自当前订阅策略组</p>
        </div>
        <span className="spacer" />
        <button onClick={() => void importClash()}>从 Clash 导入</button>
        <button onClick={() => void loadBookmarks()}>读书签</button>
        <button onClick={() => void launchClash().then(setToast)}>打开 Clash Verge</button>
        <button className="primary" onClick={() => void save()} disabled={!dirty}>
          {dirty ? "保存并写回 ●" : "已保存"}
        </button>
      </div>

      {status && (
        <div className="card" style={{ marginTop: 16 }}>
          <div className="row">
            <h3 style={{ margin: 0 }}>生效状态</h3>
            <span className={`pill ${status.clash_running ? "ok" : "dim"}`}>
              Clash {status.clash_running ? "运行中" : "未运行"}
            </span>
            <span className={`pill ${status.merge_managed ? "ok" : "err"}`}>
              {status.merge_managed ? "规则已下发" : "规则未下发/被重置"}
            </span>
            <span className={`pill ${status.subscription_present ? "ok" : "err"}`}>
              {status.subscription_present ? "已有订阅" : "无订阅"}
            </span>
            <span className="spacer" />
            <span className="mono" style={{ opacity: 0.6 }}>
              ProxyHub {status.model_rules} 条 / 运行时{" "}
              {status.runtime_rules < 0 ? "?" : status.runtime_rules} 条
            </span>
            <button onClick={() => void reapply()}>重新下发</button>
          </div>
          {status.runtime_groups.length > 0 && (
            <div className="row" style={{ gap: 6, marginTop: 10 }}>
              <span className="toast" style={{ minHeight: 0 }}>运行时策略组:</span>
              {status.runtime_groups.map((g) => (
                <span key={g} className="pill dim">{g}</span>
              ))}
            </div>
          )}
          {status.notes.map((n, i) => (
            <p key={i} className="toast" style={{ marginTop: 8, color: "var(--warn)" }}>⚠ {n}</p>
          ))}
        </div>
      )}

      <div className="card">
        <div className="row">
          <h3 style={{ margin: 0 }}>分流诊断</h3>
          <span className="toast" style={{ minHeight: 0 }}>输入域名/IP,看它会撞到哪条规则</span>
          <span className="spacer" />
          <input
            placeholder="如 chatgpt.com"
            value={diagHost}
            onChange={(e) => setDiagHost(e.target.value)}
            onKeyDown={(e) => e.key === "Enter" && void runDiagnose()}
            style={{ width: 200 }}
          />
          <button className="primary" onClick={() => void runDiagnose()}>诊断</button>
        </div>
        {diag && (
          <div style={{ marginTop: 12 }}>
            <table>
              <thead>
                <tr><th>规则类型</th><th>匹配值</th><th>去向</th><th>判定</th></tr>
              </thead>
              <tbody>
                {diag.hits.map((h, i) => (
                  <tr key={i}>
                    <td className="mono">{h.rule_type}</td>
                    <td className="mono">{h.value}</td>
                    <td><span className="pill dim">{h.target}</span></td>
                    <td>
                      <span className={`pill ${h.certainty === "命中" ? "ok" : h.certainty === "兜底" ? "dim" : "err"}`}>
                        {h.certainty}
                      </span>
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
            <p className="toast" style={{ marginTop: 10 }}>{diag.conclusion}</p>
          </div>
        )}
      </div>

      <div style={{ marginTop: 18 }}>
        {CATS.map((cat) => (
          <CategorySection
            key={cat.id}
            cat={cat}
            items={byCat[cat.id] ?? []}
            groups={groups}
            apps={apps}
            domains={domains}
            collapsed={collapsed[cat.id] ?? false}
            onToggle={() => toggleCat(cat.id)}
            onAdd={(v, t) => addRule(cat.id, v, t)}
            onRemove={removeRule}
          />
        ))}
      </div>

      {showBm && (
        <div className="card">
          <div className="row">
            <h3 style={{ margin: 0 }}>浏览器书签域名({domains.length})</h3>
            <input placeholder="过滤…" value={bmFilter} onChange={(e) => setBmFilter(e.target.value)} style={{ width: 140 }} />
            <span className="spacer" />
            <button onClick={() => setShowBm(false)}>收起</button>
          </div>
          <div className="row" style={{ gap: 6, marginTop: 10, maxHeight: 180, overflowY: "auto" }}>
            {domains
              .filter((d) => d.includes(bmFilter))
              .map((d) => (
                <button key={d} className="mono" style={{ fontSize: 11.5, padding: "3px 9px" }}
                  onClick={() => { addRule("domain", d, groups[0] ?? "DIRECT"); setToast(`已加规则:${d} → ${groups[0]}`); }}>
                  + {d}
                </button>
              ))}
            {domains.length === 0 && <span className="toast">没有可用书签域名。</span>}
          </div>
        </div>
      )}

      <div className="card">
        <h3>配置备份 / 回滚</h3>
        {backups.length === 0 ? (
          <p className="toast">保存一次后会在此生成带时间戳的备份,可一键回滚。</p>
        ) : (
          <table>
            <tbody>
              {backups.map((b) => (
                <tr key={b}>
                  <td className="mono">{b}</td>
                  <td style={{ textAlign: "right" }}>
                    <button onClick={() => void routingRollback(b).then(setToast)}>回滚到此</button>
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        )}
      </div>

      <div className="toast">{toast}</div>
    </div>
  );
}
