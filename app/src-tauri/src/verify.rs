// 模块五 · 节点核验
//
// 口径(依二毛《如何判断一个机场使用的线路类型》):跳数、ASN、延迟、IP 归属都只是"观测线索",
// 不能证明服务商采购了 IPLC/IEPL 专线。证据不足一律记为「未独立核实」,绝不替服务商背书。
//
// 实现要点:
// - 不碰用户正在运行的 Clash:起一个临时 mihomo,为每个节点开独立 listener(listeners[].proxy),并行测。
// - 节点凭据只进 0700 临时目录,结束时(含出错/panic)由 Drop 守卫杀进程 + 删目录。
// - 结果与凭据隔离:落盘的报告只含 名称/协议/服务器/端口 与观测数据。
use crate::common::{home, sh, write_file};
use serde::{Deserialize, Serialize};
use serde_yaml::{Mapping, Value as Yaml};
use std::collections::{HashMap, HashSet};
use std::net::{IpAddr, SocketAddr, TcpListener, TcpStream, ToSocketAddrs};
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

const CV_DIR: &str = "Library/Application Support/io.github.clash-verge-rev.clash-verge-rev";
const WORKERS: usize = 6;

// ───────────────────────────── 数据结构 ─────────────────────────────

#[derive(Serialize, Clone)]
pub struct ProfileInfo {
    uid: String,
    name: String,
    current: bool,
    nodes: usize,
}

#[derive(Clone)]
struct Node {
    name: String,
    ptype: String,
    server: String,
    port: u16,
    raw: Yaml, // 含凭据,只用于临时内核配置,绝不序列化进报告
}

#[derive(Serialize, Deserialize, Clone, Default)]
pub struct IpIntel {
    ip: String,
    cc: String,
    country: String,
    city: String,
    isp: String,
    org: String,
    asn: u32,
    as_name: String,
    rdns: String,
    hosting: bool,
    proxy: bool,
    mobile: bool,
    datacenter: bool,
    dc_reason: String,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct Check {
    item: String,
    claim: String,
    observed: String,
    result: String, // pass | fail | unverified
    note: String,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct AiAccess {
    service: String,
    ok: bool,
    basis: String,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct Purity {
    level: String, // clean | warn | risky | unknown
    label: String,
    reasons: Vec<String>,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct NodeResult {
    name: String,
    ptype: String,
    server: String,
    port: u16,
    entry_ip: String,
    dns_source: String,
    entry: Option<IpIntel>,
    entry_tcp_ms: Vec<f64>,
    reachable: bool,
    error: String,
    exit: Option<IpIntel>,
    ai_loc: String,
    http_ms: Vec<f64>,
    http_fail: u32,
    http_tries: u32,
    claims: Vec<String>,
    checks: Vec<Check>,
    verdict: String, // consistent | mismatch | unverified | unreachable
    missing: Vec<String>,
    purity: Purity,
    ai: Vec<AiAccess>,
    #[serde(default)]
    shared_exit_with: Vec<String>,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct Report {
    at: u64,
    profile: String,
    profile_name: String,
    local_isp: String,
    local_ip: String,
    skipped_info: Vec<String>,
    #[serde(default)]
    notes: Vec<String>,
    nodes: Vec<NodeResult>,
}

#[derive(Serialize, Clone, Default)]
pub struct Progress {
    running: bool,
    stage: String,
    done: usize,
    total: usize,
}

static PROGRESS: Mutex<Progress> = Mutex::new(Progress {
    running: false,
    stage: String::new(),
    done: 0,
    total: 0,
});

fn set_stage(stage: &str, total: Option<usize>) {
    if let Ok(mut p) = PROGRESS.lock() {
        p.stage = stage.to_string();
        if let Some(t) = total {
            p.total = t;
            p.done = 0;
        }
    }
}
fn bump() {
    if let Ok(mut p) = PROGRESS.lock() {
        p.done += 1;
    }
}

fn now() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}
fn data_dir() -> String {
    format!("{}/.config/proxyhub/verify", home())
}

// ───────────────────────────── 订阅读取 ─────────────────────────────

fn cv_path(rel: &str) -> String {
    format!("{}/{CV_DIR}/{rel}", home())
}

fn load_profiles_yaml() -> Result<Yaml, String> {
    let p = cv_path("profiles.yaml");
    let text = std::fs::read_to_string(&p).map_err(|e| format!("读不到 Clash Verge 的 profiles.yaml({p}):{e}"))?;
    serde_yaml::from_str(&text).map_err(|e| format!("profiles.yaml 解析失败:{e}"))
}

fn ystr(v: &Yaml, k: &str) -> String {
    match v.get(k) {
        Some(Yaml::String(s)) => s.clone(),
        Some(Yaml::Number(n)) => n.to_string(),
        _ => String::new(),
    }
}

/// 信息类"节点"(剩余流量、到期时间、官网地址)不是真实线路,核验时跳过并单独列出。
fn is_info_node(name: &str, server: &str) -> bool {
    const KW: [&str; 14] = ["剩余", "流量:", "流量：", "到期", "过期", "官网", "套餐", "重置", "公告", "TG群", "TG频道", "群组", "客服", "频道"];
    KW.iter().any(|k| name.contains(k)) || server.is_empty() || server == "127.0.0.1" || server.starts_with("ntp.")
}

fn load_nodes(uid: &str) -> Result<(Vec<Node>, Vec<String>, String), String> {
    let prof = load_profiles_yaml()?;
    let items = prof.get("items").and_then(|v| v.as_sequence()).cloned().unwrap_or_default();
    let item = items
        .iter()
        .find(|it| ystr(it, "uid") == uid)
        .ok_or_else(|| format!("Clash Verge 里找不到订阅 {uid}"))?;
    let file = ystr(item, "file");
    let pname = {
        let n = ystr(item, "name");
        if n.is_empty() { uid.to_string() } else { n }
    };
    let text = std::fs::read_to_string(cv_path(&format!("profiles/{file}")))
        .map_err(|e| format!("读不到订阅文件 {file}:{e}"))?;
    let doc: Yaml = serde_yaml::from_str(&text).map_err(|e| format!("订阅文件解析失败:{e}"))?;
    let proxies = doc
        .get("proxies")
        .and_then(|v| v.as_sequence())
        .ok_or("这个订阅里没有 proxies 段(可能是规则/分组类配置,不是节点订阅)")?;
    let mut nodes = Vec::new();
    let mut skipped = Vec::new();
    for p in proxies {
        let name = ystr(p, "name");
        let server = ystr(p, "server");
        let port: u16 = ystr(p, "port").parse().unwrap_or(0);
        if is_info_node(&name, &server) || port == 0 {
            skipped.push(name);
            continue;
        }
        nodes.push(Node { name, ptype: ystr(p, "type"), server, port, raw: p.clone() });
    }
    Ok((nodes, skipped, pname))
}

#[tauri::command]
pub fn verify_profiles() -> Result<Vec<ProfileInfo>, String> {
    let prof = load_profiles_yaml()?;
    let current = ystr(&prof, "current");
    let items = prof.get("items").and_then(|v| v.as_sequence()).cloned().unwrap_or_default();
    let mut out = Vec::new();
    for it in &items {
        if ystr(it, "type") != "remote" && ystr(it, "type") != "local" {
            continue;
        }
        let uid = ystr(it, "uid");
        let nodes = load_nodes(&uid).map(|(n, _, _)| n.len()).unwrap_or(0);
        let name = {
            let n = ystr(it, "name");
            if n.is_empty() { uid.clone() } else { n }
        };
        out.push(ProfileInfo { current: uid == current, uid, name, nodes });
    }
    if out.is_empty() {
        return Err("Clash Verge 里还没有节点订阅。先在 Clash Verge 导入订阅。".into());
    }
    Ok(out)
}

// ───────────────────────────── 临时内核 ─────────────────────────────

struct TempCore {
    child: Child,
    dir: String,
}
impl Drop for TempCore {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

fn mihomo_bin() -> Result<String, String> {
    let cands = [
        "/Applications/Clash Verge.app/Contents/MacOS/verge-mihomo".to_string(),
        "/opt/homebrew/bin/mihomo".to_string(),
    ];
    cands
        .into_iter()
        .find(|p| std::path::Path::new(p).exists())
        .ok_or_else(|| "找不到 mihomo 内核(Clash Verge 未安装?),无法逐节点测出口".into())
}

fn free_port() -> Result<u16, String> {
    let l = TcpListener::bind("127.0.0.1:0").map_err(|e| e.to_string())?;
    Ok(l.local_addr().map_err(|e| e.to_string())?.port())
}

/// 起临时 mihomo,每个节点一个 mixed listener。返回守卫与各节点端口。
fn start_core(nodes: &[Node]) -> Result<(TempCore, Vec<u16>), String> {
    let bin = mihomo_bin()?;
    let dir = format!("{}/proxyhub-verify-{}-{}", std::env::temp_dir().display(), std::process::id(), now());
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let _ = sh("chmod", &["700", &dir]);

    let mut ports = Vec::new();
    let mut listeners = Vec::new();
    let mut proxies = Vec::new();
    for (i, n) in nodes.iter().enumerate() {
        let port = free_port()?;
        ports.push(port);
        let mut m = Mapping::new();
        m.insert("name".into(), format!("ph{i}").into());
        m.insert("type".into(), "mixed".into());
        m.insert("listen".into(), "127.0.0.1".into());
        m.insert("port".into(), Yaml::Number(port.into()));
        m.insert("proxy".into(), n.name.clone().into());
        listeners.push(Yaml::Mapping(m));
        proxies.push(n.raw.clone());
    }
    let mut cfg = Mapping::new();
    cfg.insert("log-level".into(), "warning".into());
    cfg.insert("ipv6".into(), Yaml::Bool(false));
    cfg.insert("mode".into(), "rule".into());
    cfg.insert("proxies".into(), Yaml::Sequence(proxies));
    cfg.insert("listeners".into(), Yaml::Sequence(listeners));
    cfg.insert("rules".into(), Yaml::Sequence(vec!["MATCH,DIRECT".into()]));
    let text = serde_yaml::to_string(&Yaml::Mapping(cfg)).map_err(|e| e.to_string())?;
    let cfg_path = format!("{dir}/config.yaml");
    write_file(&cfg_path, &text)?;
    let _ = sh("chmod", &["600", &cfg_path]);

    let child = Command::new(&bin)
        .args(["-d", &dir, "-f", &cfg_path])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| format!("临时内核启动失败:{e}"))?;
    let core = TempCore { child, dir };

    // 等第一个 listener 起来(最多 8 秒),起不来明确报错,不静默
    let first = ports.first().copied().unwrap_or(0);
    let deadline = Instant::now() + Duration::from_secs(8);
    loop {
        if TcpStream::connect_timeout(&SocketAddr::from(([127, 0, 0, 1], first)), Duration::from_millis(300)).is_ok() {
            break;
        }
        if Instant::now() > deadline {
            return Err("临时内核 8 秒内没有开始监听(可能订阅里有内核不认识的协议字段)".into());
        }
        std::thread::sleep(Duration::from_millis(200));
    }
    Ok((core, ports))
}

// ───────────────────────────── 探测原语 ─────────────────────────────

fn curl_via(port: Option<u16>, url: &str, timeout: u32) -> Result<String, String> {
    let t = timeout.to_string();
    let mut args: Vec<String> = vec!["-s".into(), "-m".into(), t];
    match port {
        Some(p) => {
            args.push("-x".into());
            args.push(format!("http://127.0.0.1:{p}"));
        }
        None => {
            args.push("--noproxy".into());
            args.push("*".into());
        }
    }
    args.push(url.into());
    let refs: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
    sh("curl", &refs)
}

/// Cloudflare trace:返回 (ip, loc)。
fn cf_trace(port: Option<u16>, host: &str) -> Option<(String, String)> {
    let body = curl_via(port, &format!("https://{host}/cdn-cgi/trace"), 10).ok()?;
    let get = |k: &str| {
        body.lines()
            .find_map(|l| l.strip_prefix(&format!("{k}=")).map(|v| v.trim().to_string()))
            .unwrap_or_default()
    };
    let ip = get("ip");
    if ip.is_empty() { None } else { Some((ip, get("loc"))) }
}

/// 取出口 IPv4:依次尝试多个探测站(各自可能被墙或只回 IPv6),全部失败返回空串。
fn exit_ip_via(port: Option<u16>) -> String {
    for url in ["https://api.ipify.org", "https://ipv4.icanhazip.com"] {
        if let Ok(b) = curl_via(port, url, 8) {
            let ip = b.trim().to_string();
            if ip.parse::<std::net::Ipv4Addr>().is_ok() {
                return ip;
            }
        }
    }
    for host in ["1.1.1.1", "www.cloudflare.com"] {
        if let Some((ip, _)) = cf_trace(port, host) {
            if ip.parse::<std::net::Ipv4Addr>().is_ok() {
                return ip;
            }
        }
    }
    String::new()
}

/// 经节点访问 generate_204,返回 (成功耗时毫秒列表, 失败次数)。
fn http_probe(port: u16, tries: u32) -> (Vec<f64>, u32) {
    let mut ms = Vec::new();
    let mut fail = 0;
    for _ in 0..tries {
        let p = format!("http://127.0.0.1:{port}");
        // 主探测站不通时换备用站(某些落地屏蔽了其中一个),两个都失败才记一次丢包
        let hit = ["http://cp.cloudflare.com/generate_204", "http://www.gstatic.com/generate_204"].iter().find_map(|u| {
            let out = sh("curl", &["-s", "-o", "/dev/null", "-m", "6", "-x", &p, "-w", "%{http_code} %{time_total}", u]).ok()?;
            let mut it = out.split_whitespace();
            let code = it.next().unwrap_or("0");
            let t: f64 = it.next().and_then(|v| v.parse().ok()).unwrap_or(0.0);
            (code == "204").then_some((t * 1000.0).round())
        });
        match hit {
            Some(t) => ms.push(t),
            None => fail += 1,
        }
    }
    (ms, fail)
}

/// 本机 → 入口 的 TCP 建连耗时(只测这一段,不经代理)。
fn tcp_probe(ip: &str, port: u16, tries: u32) -> Vec<f64> {
    let Ok(addr) = format!("{ip}:{port}").parse::<SocketAddr>() else { return vec![] };
    (0..tries)
        .filter_map(|_| {
            let t = Instant::now();
            TcpStream::connect_timeout(&addr, Duration::from_secs(3))
                .ok()
                .map(|_| (t.elapsed().as_secs_f64() * 1000.0).round())
        })
        .collect()
}

fn is_fake_ip(ip: &IpAddr) -> bool {
    matches!(ip, IpAddr::V4(v) if v.octets()[0] == 198 && (v.octets()[1] == 18 || v.octets()[1] == 19))
}

/// 解析入口:系统 DNS;若拿到 Clash fake-ip(198.18/15)则改用 DoH 取真实地址。
fn resolve_entry(server: &str, port: u16) -> (String, String) {
    if let Ok(ip) = server.parse::<IpAddr>() {
        return (ip.to_string(), "字面 IP".into());
    }
    let sys = (server, port)
        .to_socket_addrs()
        .ok()
        .and_then(|mut it| it.find(|a| a.is_ipv4()))
        .map(|a| a.ip());
    if let Some(ip) = sys {
        if !is_fake_ip(&ip) {
            return (ip.to_string(), "系统 DNS".into());
        }
    }
    let url = format!("https://dns.alidns.com/resolve?name={server}&type=1");
    if let Ok(body) = curl_via(None, &url, 6) {
        if let Ok(v) = serde_json::from_str::<serde_json::Value>(&body) {
            if let Some(ip) = v["Answer"].as_array().and_then(|a| {
                a.iter().filter(|x| x["type"] == 1).find_map(|x| x["data"].as_str().map(String::from))
            }) {
                return (ip, "DoH(系统 DNS 返回 fake-ip)".into());
            }
        }
    }
    (String::new(), "解析失败".into())
}

// ───────────────────────────── IP 情报 ─────────────────────────────

const DC_ASNS: &[u32] = &[
    16509, 14618, 8987, // Amazon
    15169, 396982, 19527, // Google
    8075, 8068, // Microsoft
    31898, // Oracle
    45102, 37963, 24429, // Alibaba
    132203, 45090, // Tencent
    55990, 136907, // Huawei Cloud
    14061, 20473, 63949, 16276, 24940, 13335, 9009, 212238, 51167, 62240, 35916, 25820, 40065, 906, 54600,
    199524, 60068, 3223, 8100, 46562, 20278, 53667, 36352, 47583, 197540, 12876, 38136, 64050, 9312, 3258,
    21859, 133199, 142002, 137409, 149440, 41378, 59711, 216071, 200651, 401696,
];
const DC_WORDS: &[&str] = &[
    "cloud", "hosting", "datacenter", "data center", "server", "vps", "colo", "compute", "dedicated",
    "amazon", "aws", "google", "microsoft", "azure", "oracle", "alibaba", "aliyun", "tencent", "digitalocean",
    "vultr", "choopa", "linode", "akamai", "ovh", "hetzner", "m247", "leaseweb", "contabo", "zenlayer",
    "dmit", "cogent", "light node", "lightnode", "kaopu", "akile", "isif", "gcore", "g-core", "datacamp", "kirino", "bandwagon", "it7", "racknerd", "colocrossing",
];

static LOOKUP_NOTES: Mutex<Vec<String>> = Mutex::new(Vec::new());

fn note(msg: String) {
    if let Ok(mut v) = LOOKUP_NOTES.lock() {
        if !v.contains(&msg) {
            v.push(msg);
        }
    }
}

fn parse_ip_api(v: &serde_json::Value) -> Option<IpIntel> {
    if v["status"] != "success" {
        return None;
    }
    let as_field = v["as"].as_str().unwrap_or("");
    let asn = as_field.strip_prefix("AS").and_then(|r| r.split_whitespace().next()).and_then(|n| n.parse().ok()).unwrap_or(0);
    let s = |k: &str| v[k].as_str().unwrap_or("").to_string();
    Some(IpIntel {
        ip: s("query"), cc: s("countryCode"), country: s("country"), city: s("city"), isp: s("isp"), org: s("org"), asn,
        as_name: if s("asname").is_empty() { as_field.to_string() } else { s("asname") },
        hosting: v["hosting"].as_bool().unwrap_or(false),
        proxy: v["proxy"].as_bool().unwrap_or(false),
        mobile: v["mobile"].as_bool().unwrap_or(false),
        ..Default::default()
    })
}

const IPAPI_FIELDS: &str = "status,message,query,country,countryCode,city,isp,org,as,asname,mobile,proxy,hosting";

/// ip-api 查询:批量接口(15 次/分钟)失败则退避重试,仍失败改走单 IP 接口(45 次/分钟);
/// 查不到的 IP 记入 LOOKUP_NOTES,在报告里明示,不静默吞掉。
fn ip_api_batch(ips: &[String]) -> HashMap<String, IpIntel> {
    let mut out = HashMap::new();
    for chunk in ips.chunks(90) {
        let body: Vec<serde_json::Value> = chunk.iter().map(|ip| serde_json::json!({ "query": ip })).collect();
        let payload = serde_json::to_string(&body).unwrap_or_default();
        let url = format!("http://ip-api.com/batch?fields={IPAPI_FIELDS}");
        for attempt in 0..3 {
            let r = sh("curl", &["-s", "-m", "10", "--noproxy", "*", "-X", "POST", &url, "-d", &payload]);
            if let Some(arr) = r.ok().and_then(|t| serde_json::from_str::<Vec<serde_json::Value>>(&t).ok()) {
                for v in arr {
                    if let Some(i) = parse_ip_api(&v) {
                        out.insert(i.ip.clone(), i);
                    }
                }
                break;
            }
            std::thread::sleep(Duration::from_secs(4 * (attempt + 1)));
        }
    }
    let missing: Vec<String> = ips.iter().filter(|ip| !out.contains_key(*ip)).cloned().collect();
    for ip in &missing {
        let url = format!("http://ip-api.com/json/{ip}?fields={IPAPI_FIELDS}");
        let got = sh("curl", &["-s", "-m", "8", "--noproxy", "*", &url])
            .ok()
            .and_then(|t| serde_json::from_str::<serde_json::Value>(&t).ok())
            .and_then(|v| parse_ip_api(&v));
        match got {
            Some(i) => {
                out.insert(ip.clone(), i);
            }
            None => note(format!("{ip} 的归属查询失败(ip-api 限流或不可达),相关判定会显示为未核实")),
        }
        std::thread::sleep(Duration::from_millis(1400));
    }
    out
}

fn rdns(ip: &str) -> String {
    curl_via(None, &format!("https://ipinfo.io/{ip}/json"), 6)
        .ok()
        .and_then(|b| serde_json::from_str::<serde_json::Value>(&b).ok())
        .and_then(|v| v["hostname"].as_str().map(String::from))
        .unwrap_or_default()
}

/// 机房判定:三路交叉(ip-api hosting 标记 / 云厂商 ASN 表 / ISP·组织·反向 DNS 关键词)。
fn classify_dc(i: &mut IpIntel) {
    let mut why = Vec::new();
    if i.hosting {
        why.push("ip-api 标记 hosting".to_string());
    }
    if DC_ASNS.contains(&i.asn) {
        why.push(format!("AS{} 属云/机房 ASN", i.asn));
    }
    let hay = format!("{} {} {} {}", i.isp, i.org, i.as_name, i.rdns).to_lowercase();
    if let Some(w) = DC_WORDS.iter().find(|w| hay.contains(*w)) {
        why.push(format!("归属/反解含「{w}」"));
    }
    i.datacenter = !why.is_empty();
    i.dc_reason = why.join(";");
}

fn enrich(ips: &[String]) -> HashMap<String, IpIntel> {
    let mut map = ip_api_batch(ips);
    let keys: Vec<String> = map.keys().cloned().collect();
    let rd: Arc<Mutex<HashMap<String, String>>> = Arc::new(Mutex::new(HashMap::new()));
    let queue = Arc::new(Mutex::new(keys));
    let hs: Vec<_> = (0..WORKERS)
        .map(|_| {
            let (q, rd) = (queue.clone(), rd.clone());
            std::thread::spawn(move || loop {
                let next = q.lock().ok().and_then(|mut v| v.pop());
                let Some(ip) = next else { break };
                let h = rdns(&ip);
                if let Ok(mut m) = rd.lock() {
                    m.insert(ip, h);
                }
            })
        })
        .collect();
    for h in hs {
        let _ = h.join();
    }
    let rd = rd.lock().map(|m| m.clone()).unwrap_or_default();
    for (ip, i) in map.iter_mut() {
        i.rdns = rd.get(ip).cloned().unwrap_or_default();
        classify_dc(i);
    }
    map
}

// ───────────────────────────── 宣传解析与判定 ─────────────────────────────

const REGIONS: &[(&str, &str)] = &[
    ("香港", "HK"), ("台湾", "TW"), ("澳门", "MO"), ("日本", "JP"), ("新加坡", "SG"), ("美国", "US"), ("韩国", "KR"),
    ("法国", "FR"), ("德国", "DE"), ("英国", "GB"), ("泰国", "TH"), ("马来西亚", "MY"), ("印度尼西亚", "ID"),
    ("印尼", "ID"), ("印度", "IN"), ("澳大利亚", "AU"), ("澳洲", "AU"), ("加拿大", "CA"), ("俄罗斯", "RU"),
    ("荷兰", "NL"), ("土耳其", "TR"), ("越南", "VN"), ("菲律宾", "PH"), ("阿根廷", "AR"), ("巴西", "BR"),
    ("新西兰", "NZ"), ("意大利", "IT"), ("西班牙", "ES"), ("瑞士", "CH"), ("瑞典", "SE"), ("爱尔兰", "IE"),
    ("波兰", "PL"), ("乌克兰", "UA"), ("以色列", "IL"), ("阿联酋", "AE"), ("迪拜", "AE"), ("南非", "ZA"),
    ("墨西哥", "MX"), ("悉尼", "AU"), ("墨尔本", "AU"), ("东京", "JP"), ("大阪", "JP"), ("首尔", "KR"), ("春川", "KR"), ("洛杉矶", "US"), ("圣何塞", "US"), ("硅谷", "US"), ("伦敦", "GB"), ("法兰克福", "DE"), ("巴黎", "FR"), ("雅典", "GR"), ("希腊", "GR"), ("智利", "CL"), ("尼日利亚", "NG"), ("埃及", "EG"), ("哈萨克", "KZ"), ("芬兰", "FI"),
];

/// 节点名的地区宣传:国旗 emoji 与中文地名都解析;两者矛盾时如实返回两个,由判定逐一核对。
fn region_claims(name: &str) -> Vec<(String, String)> {
    let mut out: Vec<(String, String)> = Vec::new();
    let chars: Vec<char> = name.chars().collect();
    for w in chars.windows(2) {
        let ri = |c: char| (0x1F1E6..=0x1F1FF).contains(&(c as u32)).then(|| char::from(b'A' + (c as u32 - 0x1F1E6) as u8));
        if let (Some(a), Some(b)) = (ri(w[0]), ri(w[1])) {
            out.push((format!("国旗 {a}{b}"), format!("{a}{b}")));
            break;
        }
    }
    if let Some((zh, cc)) = REGIONS.iter().find(|(zh, _)| name.contains(zh)) {
        if !out.iter().any(|(_, c)| c == cc) {
            out.push(((*zh).to_string(), (*cc).to_string()));
        }
    }
    out
}

fn has_any(name: &str, kws: &[&str]) -> bool {
    let up = name.to_uppercase();
    kws.iter().any(|k| up.contains(&k.to_uppercase()))
}

const AI_UNSUPPORTED: &[(&str, &[&str])] = &[
    ("ChatGPT", &["CN", "HK", "MO", "RU", "BY", "IR", "KP", "SY", "CU"]),
    ("Claude", &["CN", "HK", "MO", "RU", "BY", "IR", "KP", "SY", "CU"]),
    ("Gemini", &["CN", "HK", "MO", "RU", "BY", "IR", "KP", "SY", "CU"]),
];

fn ai_access(exit_cc: &str, ai_loc: &str) -> Vec<AiAccess> {
    AI_UNSUPPORTED
        .iter()
        .map(|(svc, bad)| {
            // ChatGPT 用它自己 CDN 看到的地区(更贴近 OpenAI 的判定);其它用出口 IP 归属地
            let (cc, basis) = if *svc == "ChatGPT" && !ai_loc.is_empty() {
                (ai_loc, format!("chatgpt.com 识别为 {ai_loc}"))
            } else {
                (exit_cc, format!("出口归属 {exit_cc}"))
            };
            AiAccess { service: svc.to_string(), ok: !cc.is_empty() && !bad.contains(&cc), basis }
        })
        .collect()
}

fn purity_of(exit: &IpIntel) -> Purity {
    let mut reasons = Vec::new();
    let (level, label) = if exit.proxy {
        reasons.push("ip-api 将其标记为代理/VPN 出口".into());
        ("risky", "已被标记代理")
    } else if exit.datacenter {
        reasons.push(format!("机房 IP:{}", exit.dc_reason));
        reasons.push("GPT/Google 等对机房 IP 更容易弹验证或风控".into());
        ("warn", "机房 IP")
    } else if exit.mobile {
        reasons.push("移动蜂窝网络 IP".into());
        ("clean", "移动网络")
    } else {
        reasons.push("未命中机房 ASN/关键词,也未被标记代理(免费数据源的启发式判断)".into());
        ("clean", "疑似住宅/原生")
    };
    Purity { level: level.into(), label: label.into(), reasons }
}

fn judge(n: &mut NodeResult, peak_samples: usize) {
    let name = n.name.clone();
    let entry = n.entry.clone();
    let exit = n.exit.clone();
    let ecc = entry.as_ref().map(|e| e.cc.clone()).unwrap_or_default();
    let entry_desc = entry
        .as_ref()
        .map(|e| format!("{} · {} · AS{} {}", e.ip, e.cc, e.asn, e.as_name))
        .unwrap_or_else(|| "入口未知".into());
    let mut checks = Vec::new();
    let mut claims = Vec::new();
    let mut missing = Vec::new();

    // 地区(国旗与地名都核对;互相矛盾时两条都列出,必有一条不符)
    for (label, cc) in region_claims(&name) {
        claims.push(format!("落地 {label}"));
        let (result, observed) = match &exit {
            Some(x) if x.cc == cc => ("pass", format!("出口 {} 归属 {}", x.ip, x.cc)),
            Some(x) => ("fail", format!("出口 {} 归属 {}({}),与宣传 {cc} 不符", x.ip, x.cc, x.country)),
            None => ("unverified", "没拿到出口 IP".into()),
        };
        checks.push(Check { item: "落地地区".into(), claim: format!("{label}({cc})"), observed, result: result.into(), note: "按出口 IP 的公开归属库判断".into() });
    }

    // 线路类型
    if has_any(&name, &["IPLC", "IEPL", "专线"]) {
        claims.push("专线(IPLC/IEPL)".into());
        let (result, note) = if ecc.is_empty() {
            ("unverified", "入口未解析".to_string())
        } else if ecc == "CN" {
            ("unverified", "入口在国内、出口在境外,符合中转特征;但这不能证明运营商采购了 IPLC/IEPL".to_string())
        } else {
            ("fail", "入口直接位于境外,本机到入口走的是公网,与专线宣传不一致".to_string())
        };
        checks.push(Check { item: "线路类型".into(), claim: "专线".into(), observed: entry_desc.clone(), result: result.into(), note });
        missing.push("服务商可验证的线路说明(合同/资源证明),或 Looking Glass 路由".into());
    } else if has_any(&name, &["中转"]) {
        claims.push("中转".into());
        let (result, note) = match ecc.as_str() {
            "" => ("unverified", "入口未解析"),
            "CN" => ("pass", "入口在国内,符合中转特征(不说明是否专线)"),
            _ => ("fail", "入口在境外,不是国内中转"),
        };
        checks.push(Check { item: "线路类型".into(), claim: "中转".into(), observed: entry_desc.clone(), result: result.into(), note: note.into() });
    } else if has_any(&name, &["直连"]) {
        claims.push("直连".into());
        let (result, note) = match ecc.as_str() {
            "" => ("unverified", "入口未解析"),
            "CN" => ("fail", "入口在国内,实际是中转而非直连"),
            _ => ("pass", "入口在境外,符合直连"),
        };
        checks.push(Check { item: "线路类型".into(), claim: "直连".into(), observed: entry_desc.clone(), result: result.into(), note: note.into() });
    }

    // IP 类型
    if has_any(&name, &["家宽", "住宅", "RESIDENTIAL"]) {
        claims.push("住宅/家宽 IP".into());
        let (result, observed) = match &exit {
            Some(x) if x.datacenter => ("fail", format!("出口判为机房:{}", x.dc_reason)),
            Some(x) => ("pass", format!("出口 {} · {},未命中机房特征", x.ip, x.isp)),
            None => ("unverified", "没拿到出口 IP".into()),
        };
        checks.push(Check { item: "IP 类型".into(), claim: "住宅/家宽".into(), observed, result: result.into(), note: "免费数据源的启发式判断,不等于商业 IP 库结论".into() });
    }
    if has_any(&name, &["原生"]) {
        claims.push("原生 IP".into());
        checks.push(Check {
            item: "IP 类型".into(), claim: "原生".into(),
            observed: exit.as_ref().map(|x| format!("{} 归属 {}", x.ip, x.cc)).unwrap_or_default(),
            result: "unverified".into(),
            note: "原生需对比 IP 注册地与广播地,免费数据源不足以判断".into(),
        });
        missing.push("IP 注册地(RIR)与广播地对比".into());
    }
    // 机房厂商:「AWSJP / JPAWS」= 经 AWS 日本中转入口;单写「AWS」= 入口或出口任一在 AWS 即符合
    let up = name.to_uppercase();
    for (kw, vendor) in [("AWS", "amazon"), ("AZURE", "microsoft"), ("GCP", "google"), ("甲骨文", "oracle"), ("ORACLE", "oracle"), ("阿里云", "alibaba"), ("腾讯云", "tencent")] {
        if !up.contains(kw) {
            continue;
        }
        let suffix_cc = ["JP", "SG", "HK", "US", "KR", "TW", "DE", "GB"]
            .into_iter()
            .find(|cc| up.contains(&format!("{kw}{cc}")) || up.contains(&format!("{cc}{kw}")));
        let belongs = |i: &IpIntel| format!("{} {} {} {}", i.isp, i.org, i.as_name, i.rdns).to_lowercase().contains(vendor);
        let (result, observed, claim) = match suffix_cc {
            Some(cc) => {
                let claim = format!("经 {kw} {cc} 中转入口");
                match &entry {
                    Some(e) if belongs(e) && e.cc == cc => ("pass", format!("入口 {} · AS{} {} · {}", e.ip, e.asn, e.as_name, e.cc), claim),
                    Some(e) => ("fail", format!("入口 {} · {} · {},与宣传不符", e.ip, e.as_name, e.cc), claim),
                    None => ("unverified", "入口归属未知".into(), claim),
                }
            }
            None => {
                let claim = format!("{kw} 机房");
                let hit = [("入口", &entry), ("出口", &exit)].into_iter().find_map(|(lbl, i)| i.as_ref().filter(|i| belongs(i)).map(|i| format!("{lbl} {} 属于 {}", i.ip, i.as_name)));
                match (hit, &entry, &exit) {
                    (Some(h), _, _) => ("pass", h, claim),
                    (None, None, None) => ("unverified", "入口/出口归属均未知".into(), claim),
                    (None, _, _) => ("fail", format!("入口与出口都不属于 {kw}"), claim),
                }
            }
        };
        claims.push(claim.clone());
        checks.push(Check { item: "机房厂商".into(), claim, observed, result: result.into(), note: String::new() });
        break;
    }
    for (kw, isp) in [("移联", "移动/联通"), ("移动", "移动"), ("联通", "联通"), ("电信", "电信"), ("CMI", "移动"), ("CN2", "电信"), ("9929", "联通")] {
        if has_any(&name, &[kw]) {
            claims.push(format!("面向{isp}优化"));
            checks.push(Check {
                item: "运营商优化".into(), claim: kw.into(), observed: entry_desc.clone(), result: "unverified".into(),
                note: format!("需要用{isp}宽带在晚高峰对比测试,单次观测不能确认"),
            });
            break;
        }
    }

    if entry.as_ref().map(|e| e.asn == 13335).unwrap_or(false) {
        checks.push(Check {
            item: "入口形态".into(), claim: "—".into(), observed: entry_desc.clone(), result: "unverified".into(),
            note: "入口是 Cloudflare CDN(anycast),入口地区/ASN 没有参考意义,线路宣传无法从入口核对".into(),
        });
    }
    if let Some(x) = &exit {
        if !n.ai_loc.is_empty() && !x.cc.is_empty() && n.ai_loc != x.cc {
            missing.push(format!("chatgpt.com 识别地区为 {},与普通出口 {} 不同:服务端对 OpenAI 单独分流(落地会话可能不稳定)", n.ai_loc, x.cc));
        }
    }
    if peak_samples == 0 {
        missing.push("晚高峰(20:00–22:00)测试样本——开启下方「晚高峰监测」后自动积累".into());
    }

    n.verdict = if !n.reachable {
        "unreachable".into()
    } else if checks.iter().any(|c| c.result == "fail") {
        "mismatch".into()
    } else if checks.is_empty() || checks.iter().any(|c| c.result == "unverified") {
        "unverified".into()
    } else {
        "consistent".into()
    };
    if checks.is_empty() {
        missing.push("节点名里没有可核对的宣传(地区/线路/IP 类型)".into());
    }
    n.claims = claims;
    n.checks = checks;
    n.missing = missing;
}

// ───────────────────────────── 主流程 ─────────────────────────────

struct Probe {
    reachable: bool,
    error: String,
    exit_ip: String,
    ai_loc: String,
    http_ms: Vec<f64>,
    http_fail: u32,
    entry_tcp_ms: Vec<f64>,
}

fn probe_all(nodes: &[Node], ports: &[u16], entries: &[(String, String)], http_tries: u32) -> Vec<Probe> {
    let results: Arc<Mutex<Vec<Option<Probe>>>> = Arc::new(Mutex::new((0..nodes.len()).map(|_| None).collect()));
    let queue = Arc::new(Mutex::new((0..nodes.len()).rev().collect::<Vec<usize>>()));
    let jobs: Arc<Vec<(u16, String, u16)>> = Arc::new(
        nodes.iter().zip(ports).zip(entries).map(|((n, p), (ip, _))| (*p, ip.clone(), n.port)).collect(),
    );
    let hs: Vec<_> = (0..WORKERS)
        .map(|_| {
            let (q, r, j) = (queue.clone(), results.clone(), jobs.clone());
            std::thread::spawn(move || loop {
                let next = q.lock().ok().and_then(|mut v| v.pop());
                let Some(i) = next else { break };
                let (port, entry_ip, entry_port) = j[i].clone();
                let entry_tcp_ms = if entry_ip.is_empty() { vec![] } else { tcp_probe(&entry_ip, entry_port, 3) };
                // 先测 HTTP 连通(以它为准),再取出口 IP;任一成功即视为可用,避免因单一探测站被墙误判
                let (http_ms, http_fail) = http_probe(port, http_tries);
                let exit_ip = exit_ip_via(Some(port));
                let reachable = !http_ms.is_empty() || !exit_ip.is_empty();
                let p = if reachable {
                    let ai_loc = cf_trace(Some(port), "chatgpt.com").map(|(_, l)| l).unwrap_or_default();
                    Probe { reachable, error: String::new(), exit_ip, ai_loc, http_ms, http_fail, entry_tcp_ms }
                } else {
                    Probe {
                        reachable,
                        error: if entry_tcp_ms.is_empty() { "本机连不上入口".into() } else { "入口可连,但经节点访问外网全部失败(节点失效/协议握手失败)".into() },
                        exit_ip, ai_loc: String::new(), http_ms, http_fail, entry_tcp_ms,
                    }
                };
                if let Ok(mut v) = r.lock() {
                    v[i] = Some(p);
                }
                bump();
            })
        })
        .collect();
    for h in hs {
        let _ = h.join();
    }
    let v = results.lock().map(|mut v| std::mem::take(&mut *v)).unwrap_or_default();
    v.into_iter()
        .map(|o| o.unwrap_or(Probe { reachable: false, error: "探测线程异常".into(), exit_ip: String::new(), ai_loc: String::new(), http_ms: vec![], http_fail: 0, entry_tcp_ms: vec![] }))
        .collect()
}

fn local_info() -> (String, String) {
    let ip = exit_ip_via(None);
    if ip.is_empty() {
        return (String::new(), String::new());
    }
    let isp = ip_api_batch(std::slice::from_ref(&ip)).get(&ip).map(|i| format!("{} · {} {}", i.isp, i.country, i.city)).unwrap_or_default();
    (ip, isp)
}

fn run_verify(uid: &str) -> Result<Report, String> {
    if let Ok(mut v) = LOOKUP_NOTES.lock() {
        v.clear();
    }
    set_stage("读取订阅", None);
    let (nodes, skipped, pname) = load_nodes(uid)?;
    if nodes.is_empty() {
        return Err("这个订阅里没有可测的节点".into());
    }
    set_stage("解析入口地址", None);
    let entries: Vec<(String, String)> = nodes.iter().map(|n| resolve_entry(&n.server, n.port)).collect();
    set_stage("启动临时内核", None);
    let (core, ports) = start_core(&nodes)?;
    set_stage("逐节点测出口/延迟", Some(nodes.len()));
    let probes = probe_all(&nodes, &ports, &entries, 4);
    drop(core); // 立即清掉临时内核与含凭据的配置
    set_stage("查询 IP 归属与纯净度", None);
    let mut ips: Vec<String> = entries.iter().map(|(ip, _)| ip.clone()).chain(probes.iter().map(|p| p.exit_ip.clone())).filter(|s| !s.is_empty()).collect();
    ips.sort();
    ips.dedup();
    let intel = enrich(&ips);
    let (local_ip, local_isp) = local_info();
    let peak = peak_counts(&pname);

    let mut out = Vec::new();
    for ((n, (eip, src)), p) in nodes.iter().zip(&entries).zip(probes) {
        let exit = intel.get(&p.exit_ip).cloned();
        let purity = exit.as_ref().map(purity_of).unwrap_or(Purity { level: "unknown".into(), label: "未知".into(), reasons: vec![] });
        let ai = exit.as_ref().map(|x| ai_access(&x.cc, &p.ai_loc)).unwrap_or_default();
        let mut r = NodeResult {
            name: n.name.clone(), ptype: n.ptype.clone(), server: n.server.clone(), port: n.port,
            entry_ip: eip.clone(), dns_source: src.clone(), entry: intel.get(eip).cloned(), entry_tcp_ms: p.entry_tcp_ms,
            reachable: p.reachable, error: p.error, exit, ai_loc: p.ai_loc,
            http_tries: p.http_ms.len() as u32 + p.http_fail, http_ms: p.http_ms, http_fail: p.http_fail,
            claims: vec![], checks: vec![], verdict: String::new(), missing: vec![], purity, ai, shared_exit_with: vec![],
        };
        judge(&mut r, peak.get(&n.name).copied().unwrap_or(0));
        out.push(r);
    }
    // 共用出口:同一出口 IP 的节点只是入口不同,对风控没有分散作用
    let mut by_exit: HashMap<String, Vec<String>> = HashMap::new();
    for r in &out {
        if let Some(x) = &r.exit {
            by_exit.entry(x.ip.clone()).or_default().push(r.name.clone());
        }
    }
    for r in out.iter_mut() {
        if let Some(x) = &r.exit {
            r.shared_exit_with = by_exit[&x.ip].iter().filter(|n| **n != r.name).cloned().collect();
        }
    }
    let notes = LOOKUP_NOTES.lock().map(|mut v| std::mem::take(&mut *v)).unwrap_or_default();
    let report = Report { at: now(), profile: uid.into(), profile_name: pname, local_isp, local_ip, skipped_info: skipped, notes, nodes: out };
    let text = serde_json::to_string_pretty(&report).map_err(|e| e.to_string())?;
    write_file(&format!("{}/latest.json", data_dir()), &text)?;
    write_file(&format!("{}/settings.json", data_dir()), &serde_json::json!({ "profile": uid }).to_string())?;
    Ok(report)
}

#[tauri::command]
pub async fn verify_run(profile: String) -> Result<Report, String> {
    {
        let mut p = PROGRESS.lock().map_err(|e| e.to_string())?;
        if p.running {
            return Err("已经有一轮核验在跑了".into());
        }
        *p = Progress { running: true, stage: "准备".into(), done: 0, total: 0 };
    }
    let r = tauri::async_runtime::spawn_blocking(move || run_verify(&profile)).await.map_err(|e| e.to_string())?;
    if let Ok(mut p) = PROGRESS.lock() {
        p.running = false;
        p.stage = if r.is_ok() { "完成".into() } else { "失败".into() };
    }
    r
}

#[tauri::command]
pub fn verify_progress() -> Progress {
    PROGRESS.lock().map(|p| p.clone()).unwrap_or_default()
}

#[tauri::command]
pub fn verify_latest() -> Option<Report> {
    let text = std::fs::read_to_string(format!("{}/latest.json", data_dir())).ok()?;
    serde_json::from_str(&text).ok()
}

// ───────────────────────────── 当前出口(原「出口 IP 纯净度」) ─────────────────────────────

#[derive(Serialize)]
pub struct ExitView {
    label: String,
    ip: Option<IpIntel>,
    ai_loc: String,
    purity: Option<Purity>,
    ai: Vec<AiAccess>,
    note: String,
}

fn clash_mixed_port() -> Option<u16> {
    let text = std::fs::read_to_string(cv_path("clash-verge.yaml")).ok()?;
    text.lines().find_map(|l| l.strip_prefix("mixed-port:").and_then(|v| v.trim().parse().ok()))
}

fn clash_tun_on() -> bool {
    std::fs::read_to_string(cv_path("clash-verge.yaml"))
        .ok()
        .and_then(|t| serde_yaml::from_str::<Yaml>(&t).ok())
        .and_then(|y| y.get("tun").and_then(|t| t.get("enable")).and_then(|e| e.as_bool()))
        .unwrap_or(false)
}

fn system_proxy_on() -> bool {
    sh("scutil", &["--proxy"]).map(|o| o.contains("HTTPSEnable : 1") || o.contains("HTTPEnable : 1")).unwrap_or(false)
}

fn exit_view(label: &str, port: Option<u16>, note: String) -> ExitView {
    let ip = exit_ip_via(port);
    match (!ip.is_empty()).then_some(ip) {
        Some(ip) => {
            let ai_loc = cf_trace(port, "chatgpt.com").map(|(_, l)| l).unwrap_or_default();
            let intel = enrich(std::slice::from_ref(&ip)).remove(&ip);
            let purity = intel.as_ref().map(purity_of);
            let ai = intel.as_ref().map(|i| ai_access(&i.cc, &ai_loc)).unwrap_or_default();
            ExitView { label: label.into(), ip: intel, ai_loc, purity, ai, note }
        }
        None => ExitView { label: label.into(), ip: None, ai_loc: String::new(), purity: None, ai: vec![], note: format!("{note}(所有出口探测站都访问失败)") },
    }
}

#[tauri::command]
pub async fn verify_current_exit() -> Result<Vec<ExitView>, String> {
    tauri::async_runtime::spawn_blocking(|| {
        let mut out = vec![exit_view("本机直连出口", None, "不经任何代理,即你宽带的公网 IP".into())];
        let running = sh("pgrep", &["-x", "verge-mihomo"]).is_ok();
        match (running, clash_mixed_port()) {
            (true, Some(port)) => {
                let routed = if clash_tun_on() {
                    "TUN 已开:全部应用都按 Clash 规则分流".to_string()
                } else if system_proxy_on() {
                    "系统代理已开:遵守系统代理的应用会经过 Clash;按进程(App)分流的规则需要 TUN 才生效".to_string()
                } else {
                    "⚠ 系统代理与 TUN 都没开:这只是 Clash 端口的出口,应用实际并不经过它".to_string()
                };
                out.push(exit_view(&format!("Clash 当前出口(端口 {port})"), Some(port), routed));
            }
            _ => out.push(ExitView { label: "Clash 当前出口".into(), ip: None, ai_loc: String::new(), purity: None, ai: vec![], note: "Clash Verge 未运行".into() }),
        }
        Ok(out)
    })
    .await
    .map_err(|e| e.to_string())?
}

// ───────────────────────────── 路由追踪 ─────────────────────────────

#[derive(Serialize)]
pub struct Hop {
    n: u32,
    ip: String,
    ms: Option<f64>,
    cc: String,
    asn: u32,
    org: String,
    private: bool,
}

fn is_private(ip: &str) -> bool {
    match ip.parse::<IpAddr>() {
        Ok(IpAddr::V4(v)) => v.is_private() || v.is_loopback() || v.is_link_local() || (v.octets()[0] == 100 && (64..128).contains(&v.octets()[1])),
        _ => true,
    }
}

#[tauri::command]
pub async fn verify_trace(ip: String) -> Result<Vec<Hop>, String> {
    if ip.parse::<IpAddr>().is_err() {
        return Err("入口 IP 无效,无法追踪".into());
    }
    tauri::async_runtime::spawn_blocking(move || {
        let out = Command::new("/usr/sbin/traceroute")
            .args(["-n", "-q", "1", "-w", "1", "-m", "20", &ip])
            .output()
            .map_err(|e| format!("traceroute 启动失败:{e}"))?;
        let text = String::from_utf8_lossy(&out.stdout).to_string();
        let mut hops = Vec::new();
        for line in text.lines() {
            let mut it = line.split_whitespace();
            let Some(n) = it.next().and_then(|v| v.parse::<u32>().ok()) else { continue };
            let host = it.next().unwrap_or("*").to_string();
            let ms = it.next().and_then(|v| v.parse::<f64>().ok());
            let hip = if host == "*" { String::new() } else { host };
            hops.push(Hop { n, private: hip.is_empty() || is_private(&hip), ip: hip, ms, cc: String::new(), asn: 0, org: String::new() });
        }
        let public: Vec<String> = hops.iter().filter(|h| !h.private).map(|h| h.ip.clone()).collect::<HashSet<_>>().into_iter().collect();
        let intel = ip_api_batch(&public);
        for h in hops.iter_mut() {
            if let Some(i) = intel.get(&h.ip) {
                h.cc = i.cc.clone();
                h.asn = i.asn;
                h.org = i.as_name.clone();
            }
        }
        if hops.is_empty() {
            return Err(format!("traceroute 没有输出:{}", String::from_utf8_lossy(&out.stderr)));
        }
        Ok(hops)
    })
    .await
    .map_err(|e| e.to_string())?
}

// ───────────────────────────── 晚高峰监测 ─────────────────────────────

#[derive(Serialize, Deserialize)]
struct Sample {
    #[serde(default)]
    profile: String,
    at: u64,
    hour: u32,
    peak: bool,
    local_isp: String,
    node: String,
    http_ms: Vec<f64>,
    http_fail: u32,
    entry_tcp_ms: Vec<f64>,
}

fn local_hour() -> u32 {
    sh("date", &["+%H"]).ok().and_then(|s| s.trim().parse().ok()).unwrap_or(0)
}

fn samples_path() -> String {
    format!("{}/samples.jsonl", data_dir())
}

fn read_samples() -> Vec<Sample> {
    std::fs::read_to_string(samples_path())
        .unwrap_or_default()
        .lines()
        .filter_map(|l| serde_json::from_str(l).ok())
        .collect()
}

fn peak_counts(profile: &str) -> HashMap<String, usize> {
    let mut m = HashMap::new();
    for s in read_samples().into_iter().filter(|s| s.peak && s.profile == profile) {
        *m.entry(s.node).or_insert(0) += 1;
    }
    m
}

fn saved_profile() -> Option<String> {
    let t = std::fs::read_to_string(format!("{}/settings.json", data_dir())).ok()?;
    serde_json::from_str::<serde_json::Value>(&t).ok()?["profile"].as_str().map(String::from)
}

/// 晚高峰采样:对 Clash Verge 里所有节点订阅各测一轮,便于横向对比。
fn sample_once() -> Result<usize, String> {
    let profiles = verify_profiles()?;
    let (_, local_isp) = local_info();
    let hour = local_hour();
    let minute: u32 = sh("date", &["+%M"]).ok().and_then(|m| m.trim().parse().ok()).unwrap_or(0);
    let peak = (20..22).contains(&hour) || (hour == 22 && minute == 0);
    let cutoff = now().saturating_sub(14 * 86400);
    let mut keep: Vec<String> = read_samples().into_iter().filter(|s| s.at >= cutoff).filter_map(|s| serde_json::to_string(&s).ok()).collect();
    let mut total = 0;
    let mut errs = Vec::new();
    for pf in profiles.iter().filter(|p| p.nodes > 0) {
        let (nodes, _, _) = match load_nodes(&pf.uid) {
            Ok(v) => v,
            Err(e) => {
                errs.push(format!("{}:{e}", pf.name));
                continue;
            }
        };
        let entries: Vec<(String, String)> = nodes.iter().map(|n| resolve_entry(&n.server, n.port)).collect();
        let (core, ports) = match start_core(&nodes) {
            Ok(v) => v,
            Err(e) => {
                errs.push(format!("{}:{e}", pf.name));
                continue;
            }
        };
        let probes = probe_all(&nodes, &ports, &entries, 5);
        drop(core);
        for (n, p) in nodes.iter().zip(probes) {
            let s = Sample { profile: pf.name.clone(), at: now(), hour, peak, local_isp: local_isp.clone(), node: n.name.clone(), http_ms: p.http_ms, http_fail: p.http_fail, entry_tcp_ms: p.entry_tcp_ms };
            if let Ok(line) = serde_json::to_string(&s) {
                keep.push(line);
            }
        }
        total += nodes.len();
    }
    write_file(&samples_path(), &(keep.join("\n") + "\n"))?;
    if total == 0 {
        return Err(format!("没有采到任何节点:{}", errs.join(";")));
    }
    if !errs.is_empty() {
        eprintln!("部分订阅采样失败:{}", errs.join(";"));
    }
    Ok(total)
}

/// 命令行入口:`app --verify-report [订阅uid]`,输出 JSON 报告(不含凭据)。
pub fn headless_report(uid: Option<String>) {
    crate::common::ensure_path();
    let uid = uid.or_else(saved_profile).or_else(|| load_profiles_yaml().ok().map(|p| ystr(&p, "current")));
    match uid.ok_or_else(|| "没有可用订阅".to_string()).and_then(|u| run_verify(&u)) {
        Ok(r) => println!("{}", serde_json::to_string_pretty(&r).unwrap_or_default()),
        Err(e) => eprintln!("核验失败:{e}"),
    }
}

/// launchd 无界面入口:`ProxyHub.app/Contents/MacOS/app --verify-sample`
pub fn headless_sample() {
    crate::common::ensure_path();
    match sample_once() {
        Ok(n) => println!("[{}] sampled {n} nodes", now()),
        Err(e) => eprintln!("[{}] sample failed: {e}", now()),
    }
}

#[tauri::command]
pub async fn verify_sample_now() -> Result<String, String> {
    tauri::async_runtime::spawn_blocking(|| sample_once().map(|n| format!("已采样 {n} 个节点"))).await.map_err(|e| e.to_string())?
}

const AGENT: &str = "com.proxyhub.verify";
const SLOTS: [(u32, u32); 5] = [(15, 0), (20, 0), (20, 40), (21, 20), (22, 0)];

fn agent_path() -> String {
    format!("{}/Library/LaunchAgents/{AGENT}.plist", home())
}

#[derive(Serialize)]
pub struct Schedule {
    installed: bool,
    loaded: bool,
    slots: Vec<String>,
    binary: String,
    samples: usize,
}

fn app_binary() -> String {
    std::env::current_exe().map(|p| p.display().to_string()).unwrap_or_default()
}

#[tauri::command]
pub fn verify_schedule_get() -> Schedule {
    let uid = sh("id", &["-u"]).unwrap_or_default().trim().to_string();
    Schedule {
        installed: std::path::Path::new(&agent_path()).exists(),
        loaded: sh("launchctl", &["print", &format!("gui/{uid}/{AGENT}")]).is_ok(),
        slots: SLOTS.iter().map(|(h, m)| format!("{h:02}:{m:02}{}", if *h == 15 { "(闲时基线)" } else { "" })).collect(),
        binary: app_binary(),
        samples: read_samples().len(),
    }
}

#[tauri::command]
pub fn verify_schedule_set(enable: bool) -> Result<Schedule, String> {
    let uid = sh("id", &["-u"])?.trim().to_string();
    let target = format!("gui/{uid}");
    let _ = sh("launchctl", &["bootout", &format!("{target}/{AGENT}")]);
    if enable {
        let bin = app_binary();
        if !bin.starts_with("/Applications/") {
            return Err(format!("当前运行的不是 /Applications 里的 ProxyHub({bin})。请从 /Applications 启动后再开启,否则定时任务会指向编译目录。"));
        }
        let slots: String = SLOTS
            .iter()
            .map(|(h, m)| format!("    <dict><key>Hour</key><integer>{h}</integer><key>Minute</key><integer>{m}</integer></dict>\n"))
            .collect();
        let log = format!("{}/agent.log", data_dir());
        let plist = format!(
            "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<!DOCTYPE plist PUBLIC \"-//Apple//DTD PLIST 1.0//EN\" \"http://www.apple.com/DTDs/PropertyList-1.0.dtd\">\n<plist version=\"1.0\"><dict>\n  <key>Label</key><string>{AGENT}</string>\n  <key>ProgramArguments</key><array><string>{bin}</string><string>--verify-sample</string></array>\n  <key>StartCalendarInterval</key><array>\n{slots}  </array>\n  <key>StandardOutPath</key><string>{log}</string>\n  <key>StandardErrorPath</key><string>{log}</string>\n</dict></plist>\n"
        );
        write_file(&agent_path(), &plist)?;
        sh("launchctl", &["bootstrap", &target, &agent_path()]).map_err(|e| format!("launchctl 加载失败:{e}"))?;
    } else {
        let _ = std::fs::remove_file(agent_path());
    }
    Ok(verify_schedule_get())
}

#[derive(Serialize)]
pub struct PeakStat {
    profile: String,
    node: String,
    off_n: usize,
    off_ms: Option<f64>,
    off_loss: Option<f64>,
    peak_n: usize,
    peak_ms: Option<f64>,
    peak_loss: Option<f64>,
    local_isp: String,
}

fn median(v: &mut [f64]) -> Option<f64> {
    if v.is_empty() {
        return None;
    }
    v.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    Some(v[v.len() / 2])
}

#[tauri::command]
pub fn verify_peak_stats() -> Vec<PeakStat> {
    let mut by: HashMap<(String, String), Vec<Sample>> = HashMap::new();
    for s in read_samples() {
        by.entry((s.profile.clone(), s.node.clone())).or_default().push(s);
    }
    let mut out: Vec<PeakStat> = by
        .into_iter()
        .map(|((profile, node), ss)| {
            let agg = |peak: bool| {
                let part: Vec<&Sample> = ss.iter().filter(|s| s.peak == peak).collect();
                let mut ms: Vec<f64> = part.iter().flat_map(|s| s.http_ms.clone()).collect();
                let tries: u32 = part.iter().map(|s| s.http_ms.len() as u32 + s.http_fail).sum();
                let fail: u32 = part.iter().map(|s| s.http_fail).sum();
                let loss = if tries == 0 { None } else { Some((fail as f64 / tries as f64 * 1000.0).round() / 10.0) };
                (part.len(), median(&mut ms), loss)
            };
            let (off_n, off_ms, off_loss) = agg(false);
            let (peak_n, peak_ms, peak_loss) = agg(true);
            let local_isp = ss.last().map(|s| s.local_isp.clone()).unwrap_or_default();
            PeakStat { profile, node, off_n, off_ms, off_loss, peak_n, peak_ms, peak_loss, local_isp }
        })
        .collect();
    out.sort_by(|a, b| (&a.profile, &a.node).cmp(&(&b.profile, &b.node)));
    out
}
