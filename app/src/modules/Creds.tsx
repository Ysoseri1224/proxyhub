import { useEffect, useMemo, useState } from "react";
import {
  credList,
  credDetail,
  credSet,
  credDelete,
  ghAccounts,
  ghSwitch,
  toolLogins,
  runInTerminal,
  importPasswordBook,
  chromeProfiles,
  launchApp,
  type CredMeta,
  type CredDetail,
  type GhAccount,
  type ToolLogin,
  type ChromeProfile,
} from "../api";
import { getErrorMessage, tryParse } from "../util";

const BASE_CATEGORIES = ["github", "cloudflare", "aws", "npm", "pypi", "ai", "other"];

export function Creds() {
  const [tools, setTools] = useState<ToolLogin[]>([]);
  const [accounts, setAccounts] = useState<GhAccount[]>([]);
  const [chromes, setChromes] = useState<ChromeProfile[]>([]);
  const [creds, setCreds] = useState<CredMeta[]>([]);
  const [detail, setDetail] = useState<CredDetail | null>(null);
  const [detailLoading, setDetailLoading] = useState(false);
  const [switching, setSwitching] = useState("");
  const [draft, setDraft] = useState({ name: "", value: "", category: "github", note: "" });
  const [toast, setToast] = useState("");

  async function load(): Promise<void> {
    try {
      setTools(await toolLogins());
      setAccounts(await ghAccounts());
      setChromes(await chromeProfiles());
      setCreds(await credList());
    } catch (error: unknown) {
      setToast(getErrorMessage(error));
    }
  }
  useEffect(() => {
    void load();
  }, []);

  const categories = useMemo(() => {
    const s = new Set(BASE_CATEGORIES);
    creds.forEach((c) => s.add(c.category));
    return [...s];
  }, [creds]);

  async function doSwitch(user: string): Promise<void> {
    setSwitching(user);
    try {
      setToast(await ghSwitch(user));
      setAccounts(await ghAccounts());
    } catch (error: unknown) {
      setToast(getErrorMessage(error));
    } finally {
      setSwitching("");
    }
  }
  async function openDetail(name: string): Promise<void> {
    setDetailLoading(true);
    setDetail(null);
    try {
      setDetail(tryParse<CredDetail>(await credDetail(name)));
    } catch (error: unknown) {
      setToast(getErrorMessage(error));
    } finally {
      setDetailLoading(false);
    }
  }
  async function inject(name: string): Promise<void> {
    try {
      const d = tryParse<CredDetail>(await credDetail(name));
      if (d) {
        await navigator.clipboard.writeText(`export ${name}='${d.value}'`);
        setToast(`已复制 export ${name}=… 到剪贴板`);
      }
    } catch (error: unknown) {
      setToast(getErrorMessage(error));
    }
  }
  async function add(): Promise<void> {
    if (!draft.name.trim() || !draft.value.trim()) return;
    try {
      await credSet(draft.name.trim(), draft.value, draft.category.trim() || "other", draft.note);
      setDraft({ name: "", value: "", category: draft.category, note: "" });
      setToast("已存入 Keychain");
      await load();
    } catch (error: unknown) {
      setToast(getErrorMessage(error));
    }
  }
  async function remove(name: string): Promise<void> {
    try {
      await credDelete(name);
      await load();
    } catch (error: unknown) {
      setToast(getErrorMessage(error));
    }
  }
  async function doImport(): Promise<void> {
    try {
      setToast(await importPasswordBook());
      await load();
    } catch (error: unknown) {
      setToast(getErrorMessage(error));
    }
  }
  async function login(cmd: string): Promise<void> {
    try {
      await runInTerminal(cmd);
      setToast(`已在终端打开:${cmd} — 登录完点刷新`);
    } catch (error: unknown) {
      setToast(getErrorMessage(error));
    }
  }

  return (
    <div>
      <div className="row">
        <div>
          <h1 className="page-title">凭据账号</h1>
          <p className="page-sub" style={{ margin: 0 }}>登录态切换 + Keychain 加密凭据库 · ProxyHub 即权威,仓库零明文</p>
        </div>
        <span className="spacer" />
        <button onClick={() => void load()}>刷新</button>
      </div>

      <div className="card" style={{ marginTop: 16 }}>
        <h3>登录态总览</h3>
        {tools.map((t) => (
          <div key={t.tool} className="row" style={{ padding: "6px 0", borderBottom: "0.5px solid var(--border)" }}>
            <span style={{ width: 170, fontWeight: 600 }}>{t.tool}</span>
            <span className={`pill ${t.logged_in ? "ok" : "dim"}`}>{t.detail}</span>
            <span className="spacer" />
            <button onClick={() => void login(t.login_cmd)}>{t.logged_in ? "重新登录" : "登录"}</button>
          </div>
        ))}
      </div>

      <div className="card">
        <h3>GitHub 身份(终端当前账号)</h3>
        {accounts.map((a) => (
          <div key={a.user} className="row" style={{ padding: "6px 10px", marginBottom: 5, borderRadius: 8,
            background: a.active ? "var(--sel)" : "transparent", border: "0.5px solid var(--border)" }}>
            <span style={{ fontWeight: 600 }}>{a.user}</span>
            <span className={`pill ${a.active ? "ok" : "dim"}`}>{a.active ? "当前登录" : "未激活"}</span>
            <span className="spacer" />
            {!a.active && (
              <button className="primary" disabled={switching === a.user} onClick={() => void doSwitch(a.user)}>
                {switching === a.user ? "切换中…" : "切到此账号"}
              </button>
            )}
          </div>
        ))}
        {accounts.length === 0 && <p className="toast">未检测到 gh 登录账号。</p>}
      </div>

      {chromes.length > 0 && (
        <div className="card">
          <h3>Chrome 身份</h3>
          <div className="row">
            {chromes.map((c) => (
              <button key={c.path} onClick={() => void launchApp(c.path).then(setToast)}>{c.name}</button>
            ))}
          </div>
        </div>
      )}

      <div className="card">
        <div className="row">
          <h3 style={{ margin: 0 }}>凭据库({creds.length})</h3>
          <span className="spacer" />
          <button onClick={() => void doImport()}>一键导入密码本</button>
        </div>
        <div className="row" style={{ marginTop: 10 }}>
          <input placeholder="名称" value={draft.name} onChange={(e) => setDraft({ ...draft, name: e.target.value })} style={{ width: 170 }} />
          <input list="cats" placeholder="分类(可新建)" value={draft.category}
            onChange={(e) => setDraft({ ...draft, category: e.target.value })} style={{ width: 120 }} />
          <datalist id="cats">{categories.map((c) => <option key={c} value={c} />)}</datalist>
          <input placeholder="值" type="password" value={draft.value} onChange={(e) => setDraft({ ...draft, value: e.target.value })} style={{ flex: 1, minWidth: 130 }} />
          <input placeholder="备注" value={draft.note} onChange={(e) => setDraft({ ...draft, note: e.target.value })} style={{ width: 90 }} />
          <button className="primary" onClick={() => void add()}>存入</button>
        </div>
        <table style={{ marginTop: 12 }}>
          <thead><tr><th>名称</th><th>分类</th><th>备注</th><th>管理</th></tr></thead>
          <tbody>
            {creds.map((c) => (
              <tr key={c.name}>
                <td style={{ fontWeight: 600, cursor: "default" }} onClick={() => void openDetail(c.name)}>
                  <span style={{ textDecoration: "underline dotted" }}>{c.name}</span>
                </td>
                <td><span className="pill dim">{c.category}</span></td>
                <td style={{ opacity: 0.6, maxWidth: 200, overflow: "hidden", textOverflow: "ellipsis" }}>{c.note}</td>
                <td style={{ whiteSpace: "nowrap" }}>
                  <button onClick={() => void openDetail(c.name)}>详情</button>{" "}
                  <button onClick={() => void inject(c.name)}>注入</button>{" "}
                  <button className="danger" onClick={() => void remove(c.name)}>删</button>
                </td>
              </tr>
            ))}
            {creds.length === 0 && <tr><td colSpan={4} className="toast">还没有凭据,点「一键导入密码本」。</td></tr>}
          </tbody>
        </table>
      </div>

      {(detail || detailLoading) && (
        <div className="modal-backdrop" onClick={() => setDetail(null)}>
          <div className="modal" onClick={(e) => e.stopPropagation()}>
            {detailLoading && <p>在线查询中…</p>}
            {detail && (
              <>
                <div className="row">
                  <h3 style={{ margin: 0 }}>{detail.name}</h3>
                  <span className="pill dim">{detail.type}</span>
                  <span className="spacer" />
                  <button onClick={() => setDetail(null)}>关闭</button>
                </div>
                <table style={{ marginTop: 12 }}>
                  <tbody>
                    <tr><td style={{ color: "var(--text-dim)" }}>归属账号</td><td className="mono">{detail.account || "—"}</td></tr>
                    <tr><td style={{ color: "var(--text-dim)" }}>有效性</td><td><span className={`pill ${detail.status.includes("有效") || detail.status === "active" ? "ok" : "err"}`}>{detail.status || "—"}</span></td></tr>
                    <tr><td style={{ color: "var(--text-dim)" }}>过期</td><td className="mono">{detail.expiration || "—"}</td></tr>
                    <tr><td style={{ color: "var(--text-dim)" }}>权限 scope</td><td className="mono" style={{ wordBreak: "break-all" }}>{detail.scopes || "—"}</td></tr>
                    <tr><td style={{ color: "var(--text-dim)" }}>值</td><td className="mono" style={{ wordBreak: "break-all" }}>{detail.value}</td></tr>
                  </tbody>
                </table>
              </>
            )}
          </div>
        </div>
      )}

      <div className="toast">{toast}</div>
    </div>
  );
}
