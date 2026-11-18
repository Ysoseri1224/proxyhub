import { useEffect, useState } from "react";
import {
  routingGet,
  routingSave,
  routingBackups,
  routingRollback,
  routingImportClash,
  listApps,
  importBookmarks,
  launchClash,
  exitIpCheck,
  type Rule,
  type AppItem,
} from "../api";
import { getErrorMessage, tryParse } from "../util";

type Category = "app" | "domain" | "geosite" | "ipcidr" | "geoip" | "port";
const CATS: { id: Category; label: string }[] = [
  { id: "app", label: "应用 App" },
  { id: "domain", label: "域名" },
  { id: "geosite", label: "预置服务集" },
  { id: "ipcidr", label: "IP/网段" },
  { id: "geoip", label: "地区" },
  { id: "port", label: "端口" },
];
const TARGETS = ["🌍 国外流量", "⚡ 自动选择", "DIRECT"];

interface IpInfo {
  query?: string;
  country?: string;
  city?: string;
  isp?: string;
  org?: string;
  proxy?: boolean;
  hosting?: boolean;
}

export function Routing() {
  const [rules, setRules] = useState<Rule[]>([]);
  const [apps, setApps] = useState<AppItem[]>([]);
  const [domains, setDomains] = useState<string[]>([]);
  const [backups, setBackups] = useState<string[]>([]);
  const [ip, setIp] = useState<IpInfo | null>(null);
  const [draft, setDraft] = useState<Rule>({ category: "domain", value: "", target: TARGETS[0] });
  const [toast, setToast] = useState("");

  async function load(): Promise<void> {
    try {
      const model = tryParse<{ rules: Rule[] }>(await routingGet());
      setRules(model?.rules ?? []);
      setBackups(await routingBackups());
    } catch (error: unknown) {
      setToast(getErrorMessage(error));
    }
  }
  useEffect(() => {
    void load();
    void listApps().then(setApps).catch(() => undefined);
  }, []);

  function addRule(): void {
    if (!draft.value.trim()) return;
    setRules((prev) => [...prev, draft]);
    setDraft({ category: draft.category, value: "", target: draft.target });
  }
  function removeRule(i: number): void {
    setRules((prev) => prev.filter((_, idx) => idx !== i));
  }

  async function save(): Promise<void> {
    try {
      setToast(await routingSave(JSON.stringify({ rules })));
      await load();
    } catch (error: unknown) {
      setToast(getErrorMessage(error));
    }
  }
  async function importClash(): Promise<void> {
    try {
      const model = tryParse<{ rules: Rule[] }>(await routingImportClash());
      setRules(model?.rules ?? []);
      setToast("已从 Clash Merge 导入规则(未写回,确认后点保存)");
    } catch (error: unknown) {
      setToast(getErrorMessage(error));
    }
  }
  async function loadBookmarks(): Promise<void> {
    try {
      setDomains(await importBookmarks());
      setToast(`从书签提取 ${domains.length} 个域名(在值输入框下拉可选)`);
    } catch (error: unknown) {
      setToast(getErrorMessage(error));
    }
  }
  async function checkIp(): Promise<void> {
    try {
      setIp(tryParse<IpInfo>(await exitIpCheck()));
    } catch (error: unknown) {
      setToast(getErrorMessage(error));
    }
  }

  return (
    <div>
      <h1 className="page-title">网络分流</h1>
      <p className="page-sub">图形化编排"谁走哪条分流" · 写回 Clash · 不复刻 Clash</p>

      <div className="card">
        <div className="row">
          <h3 style={{ margin: 0 }}>规则</h3>
          <span className="spacer" />
          <button onClick={() => void importClash()}>从 Clash 导入</button>
          <button onClick={() => void loadBookmarks()}>读书签域名</button>
          <button onClick={() => void launchClash().then(setToast)}>打开 Clash Verge</button>
          <button className="primary" onClick={() => void save()}>保存并写回</button>
        </div>

        <div className="row" style={{ marginTop: 12 }}>
          <select value={draft.category} onChange={(e) => setDraft({ ...draft, category: e.target.value })}>
            {CATS.map((c) => <option key={c.id} value={c.id}>{c.label}</option>)}
          </select>
          <input
            list={draft.category === "app" ? "apps" : "domains"}
            placeholder={draft.category === "app" ? "应用名(进程)" : "值,如 openai.com / 1.1.1.1/32 / CN"}
            value={draft.value}
            style={{ flex: 1, minWidth: 200 }}
            onChange={(e) => setDraft({ ...draft, value: e.target.value })}
          />
          <datalist id="apps">{apps.map((a) => <option key={a.path} value={a.name} />)}</datalist>
          <datalist id="domains">{domains.map((d) => <option key={d} value={d} />)}</datalist>
          <select value={draft.target} onChange={(e) => setDraft({ ...draft, target: e.target.value })}>
            {TARGETS.map((t) => <option key={t} value={t}>{t}</option>)}
          </select>
          <button onClick={addRule}>添加</button>
        </div>

        <table style={{ marginTop: 12 }}>
          <thead><tr><th>分类</th><th>值</th><th>去向</th><th></th></tr></thead>
          <tbody>
            {rules.map((r, i) => (
              <tr key={i}>
                <td>{CATS.find((c) => c.id === r.category)?.label ?? r.category}</td>
                <td className="mono">{r.value}</td>
                <td>{r.target}</td>
                <td><button className="danger" onClick={() => removeRule(i)}>删</button></td>
              </tr>
            ))}
            {rules.length === 0 && <tr><td colSpan={4} className="toast">暂无规则。中国直连兜底会自动附加。</td></tr>}
          </tbody>
        </table>
      </div>

      <div className="card">
        <div className="row">
          <h3 style={{ margin: 0 }}>出口 IP 纯净度</h3>
          <span className="spacer" />
          <button onClick={() => void checkIp()}>检测</button>
        </div>
        {ip && (
          <div className="row" style={{ marginTop: 10, gap: 14 }}>
            <span className="mono">{ip.query}</span>
            <span>{ip.country} {ip.city}</span>
            <span style={{ opacity: 0.6 }}>{ip.isp}</span>
            <span className={`pill ${ip.proxy ? "err" : "ok"}`}>{ip.proxy ? "被标记为代理" : "非代理"}</span>
            <span className={`pill ${ip.hosting ? "err" : "ok"}`}>{ip.hosting ? "机房IP" : "住宅/非机房"}</span>
          </div>
        )}
      </div>

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
