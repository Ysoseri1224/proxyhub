import { useState } from "react";
import "./App.css";
import { Mirror } from "./modules/Mirror";
import { Routing } from "./modules/Routing";
import { Creds } from "./modules/Creds";
import { Cloudflare } from "./modules/Cloudflare";
import { Verify } from "./modules/Verify";

type Tab = "mirror" | "routing" | "verify" | "creds" | "cloudflare";

interface NavDef {
  id: Tab;
  ico: string;
  label: string;
}

const NAV: NavDef[] = [
  { id: "mirror", ico: "📦", label: "镜像源" },
  { id: "routing", ico: "🧭", label: "网络分流" },
  { id: "verify", ico: "🛰️", label: "节点核验" },
  { id: "creds", ico: "🔐", label: "凭据账号" },
  { id: "cloudflare", ico: "☁️", label: "Cloudflare" },
];

function App() {
  const [tab, setTab] = useState<Tab>("mirror");

  return (
    <div className="app">
      <aside className="sidebar">
        <div className="brand">ProxyHub</div>
        {NAV.map((n) => (
          <div
            key={n.id}
            className={`nav-item${tab === n.id ? " active" : ""}`}
            onClick={() => setTab(n.id)}
          >
            <span className="ico">{n.ico}</span>
            <span>{n.label}</span>
          </div>
        ))}
      </aside>
      <main className="content">
        {tab === "mirror" && <Mirror />}
        {tab === "routing" && <Routing />}
        {tab === "verify" && <Verify />}
        {tab === "creds" && <Creds />}
        {tab === "cloudflare" && <Cloudflare />}
      </main>
    </div>
  );
}

export default App;
