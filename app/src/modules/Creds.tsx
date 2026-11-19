import { useEffect, useState } from "react";
import {
  credList,
  credGet,
  credSet,
  credDelete,
  ghAccounts,
  ghSwitch,
  toolLogins,
  runInTerminal,
  importPasswordBook,
  type CredMeta,
  type GhAccount,
  type ToolLogin,
} from "../api";
import { getErrorMessage } from "../util";

const CATEGORIES = ["github", "cloudflare", "aws", "npm", "pypi", "ai", "other"];

export function Creds() {
  const [tools, setTools] = useState<ToolLogin[]>([]);
  const [accounts, setAccounts] = useState<GhAccount[]>([]);
  const [creds, setCreds] = useState<CredMeta[]>([]);
  const [revealed, setRevealed] = useState<Record<string, string>>({});
  const [switching, setSwitching] = useState("");
  const [draft, setDraft] = useState({ name: "", value: "", category: "github", note: "" });
  const [toast, setToast] = useState("");

  async function load(): Promise<void> {
    try {
      setTools(await toolLogins());
      setAccounts(await ghAccounts());
      setCreds(await credList());
    } catch (error: unknown) {
      setToast(getErrorMessage(error));
    }
  }
  useEffect(() => {
    void load();
  }, []);

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
  async function login(cmd: string): Promise<void> {
    try {
      await runInTerminal(cmd);
      setToast(`已在终端打开:${cmd} — 登录完回来点刷新`);
    } catch (error: unknown) {
      setToast(getErrorMessage(error));
    }
  }
  async function reveal(name: string): Promise<void> {
    try {
      const value = await credGet(name);
      setRevealed((prev) => ({ ...prev, [name]: value }));
    } catch (error: unknown) {
      setToast(getErrorMessage(error));
    }
  }
  async function inject(name: string): Promise<void> {
    try {
      const value = await credGet(name);
      await navigator.clipboard.writeText(`export ${name}='${value}'`);
      setToast(`已复制 export ${name}=… 到剪贴板`);
    } catch (error: unknown) {
      setToast(getErrorMessage(error));
    }
  }
  async function edit(c: CredMeta): Promise<void> {
    const value = window.prompt(`修改 ${c.name} 的值(留空取消)`);
    if (!value) return;
    try {
      await credSet(c.name, value, c.category, c.note);
      setToast(`已更新 ${c.name}`);
      await load();
    } catch (error: unknown) {
      setToast(getErrorMessage(error));
    }
  }
  async function add(): Promise<void> {
    if (!draft.name.trim() || !draft.value.trim()) return;
    try {
      await credSet(draft.name.trim(), draft.value, draft.category, draft.note);
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

  return (
    <div>
      <div className="row">
        <div>
          <h1 className="page-title">凭据账号</h1>
          <p className="page-sub" style={{ margin: 0 }}>各工具登录态 + Keychain 加密凭据库 · 仓库零明文</p>
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
        <h3>GitHub 多账号(终端当前账号)</h3>
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

      <div className="card">
        <div className="row">
          <h3 style={{ margin: 0 }}>凭据库({creds.length})</h3>
          <span className="spacer" />
          <button onClick={() => void doImport()}>一键导入密码本</button>
        </div>
        <div className="row" style={{ marginTop: 10 }}>
          <input placeholder="名称" value={draft.name} onChange={(e) => setDraft({ ...draft, name: e.target.value })} style={{ width: 180 }} />
          <select value={draft.category} onChange={(e) => setDraft({ ...draft, category: e.target.value })}>
            {CATEGORIES.map((c) => <option key={c} value={c}>{c}</option>)}
          </select>
          <input placeholder="值" type="password" value={draft.value} onChange={(e) => setDraft({ ...draft, value: e.target.value })} style={{ flex: 1, minWidth: 140 }} />
          <input placeholder="备注" value={draft.note} onChange={(e) => setDraft({ ...draft, note: e.target.value })} style={{ width: 100 }} />
          <button className="primary" onClick={() => void add()}>存入</button>
        </div>
        <table style={{ marginTop: 12 }}>
          <thead><tr><th>名称</th><th>分类</th><th>备注</th><th>值</th><th>管理</th></tr></thead>
          <tbody>
            {creds.map((c) => (
              <tr key={c.name}>
                <td style={{ fontWeight: 600 }}>{c.name}</td>
                <td><span className="pill dim">{c.category}</span></td>
                <td style={{ opacity: 0.6, maxWidth: 160, overflow: "hidden", textOverflow: "ellipsis" }}>{c.note}</td>
                <td className="mono" style={{ maxWidth: 180, overflow: "hidden", textOverflow: "ellipsis" }}>
                  {revealed[c.name] ?? <button onClick={() => void reveal(c.name)}>显示</button>}
                </td>
                <td style={{ whiteSpace: "nowrap" }}>
                  <button onClick={() => void inject(c.name)}>注入</button>{" "}
                  <button onClick={() => void edit(c)}>改</button>{" "}
                  <button className="danger" onClick={() => void remove(c.name)}>删</button>
                </td>
              </tr>
            ))}
            {creds.length === 0 && <tr><td colSpan={5} className="toast">还没有凭据,点「一键导入密码本」。</td></tr>}
          </tbody>
        </table>
      </div>

      <div className="toast">{toast}</div>
    </div>
  );
}
