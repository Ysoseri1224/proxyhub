// 模块二:网络分流图形化管理。ProxyHub 自己持有结构化规则(routing.json),
// 据此生成 Clash Verge 的全局 Merge 配置(Merge.yaml)。写回前备份,可回滚。不复刻 Clash。
use crate::common::{home, sh, write_file};
use serde::{Deserialize, Serialize};
use std::fs;

#[derive(Serialize, Deserialize, Clone)]
pub struct Rule {
    pub category: String, // app | domain | geosite | ipcidr | geoip | port
    pub value: String,
    pub target: String, // 🌍 国外流量 | DIRECT | ⚡ 自动选择 | ...
}

#[derive(Serialize, Deserialize, Default)]
pub struct RoutingModel {
    pub rules: Vec<Rule>,
}

/// 写入 Merge.yaml 时的首行标记:据此判断该文件是否为 ProxyHub 下发(未被 Clash 重置)。
const MARKER: &str = "由 ProxyHub 生成";

fn clash_app_dir() -> String {
    format!(
        "{}/Library/Application Support/io.github.clash-verge-rev.clash-verge-rev",
        home()
    )
}

fn routing_json() -> String {
    format!("{}/.config/proxyhub/routing.json", home())
}
fn merge_yaml() -> String {
    format!("{}/profiles/Merge.yaml", clash_app_dir())
}
fn backup_dir() -> String {
    format!("{}/.config/proxyhub/routing-backups", home())
}

/// 从 Clash Verge 配置里解析出 mihomo 控制接口的 unix socket 路径。
fn mihomo_sock() -> Option<String> {
    for f in ["config.yaml", "clash-verge.yaml"] {
        if let Ok(s) = fs::read_to_string(format!("{}/{}", clash_app_dir(), f)) {
            for line in s.lines() {
                if let Some(v) = line.trim().strip_prefix("external-controller-unix:") {
                    let v = v.trim().trim_matches('"').trim_matches('\'');
                    if !v.is_empty() {
                        return Some(v.to_string());
                    }
                }
            }
        }
    }
    None
}

/// 从 Clash Verge 配置里解析出 mihomo 控制接口的 TCP 地址(socket 不可用时回退)。
fn mihomo_tcp() -> Option<String> {
    for f in ["clash-verge.yaml", "config.yaml"] {
        if let Ok(s) = fs::read_to_string(format!("{}/{}", clash_app_dir(), f)) {
            for line in s.lines() {
                if let Some(v) = line.trim().strip_prefix("external-controller:") {
                    let v = v.trim().trim_matches('"').trim_matches('\'');
                    if !v.is_empty() {
                        return Some(v.to_string());
                    }
                }
            }
        }
    }
    None
}

/// 向 mihomo 控制接口发 GET(优先 unix socket,其次 TCP)。
fn mihomo_get(path: &str) -> Result<String, String> {
    if let Some(sock) = mihomo_sock() {
        return sh(
            "curl",
            &["-s", "--max-time", "4", "--unix-socket", &sock, &format!("http://localhost{path}")],
        );
    }
    if let Some(tcp) = mihomo_tcp() {
        return sh("curl", &["-s", "--max-time", "4", &format!("http://{tcp}{path}")]);
    }
    Err("未找到 mihomo 控制接口(Clash Verge 未运行?)".into())
}

fn clash_running() -> bool {
    ["verge-mihomo", "clash-verge", "mihomo"].iter().any(|p| {
        sh("pgrep", &["-x", p]).map(|s| !s.trim().is_empty()).unwrap_or(false)
    })
}

fn cat_to_type(cat: &str) -> &'static str {
    match cat {
        "app" => "PROCESS-NAME",
        "domain" => "DOMAIN-SUFFIX",
        "geosite" => "GEOSITE",
        "ipcidr" => "IP-CIDR",
        "geoip" => "GEOIP",
        "port" => "DST-PORT",
        _ => "DOMAIN-SUFFIX",
    }
}
fn type_to_cat(t: &str) -> &'static str {
    match t {
        "PROCESS-NAME" | "PROCESS-PATH" => "app",
        "DOMAIN" | "DOMAIN-SUFFIX" | "DOMAIN-KEYWORD" => "domain",
        "GEOSITE" => "geosite",
        "IP-CIDR" | "IP-CIDR6" => "ipcidr",
        "GEOIP" => "geoip",
        "DST-PORT" | "SRC-PORT" => "port",
        _ => "domain",
    }
}

/// 由结构化模型生成完整 Merge.yaml。
fn gen_merge(model: &RoutingModel) -> String {
    let mut s = String::new();
    s.push_str("# 由 ProxyHub 生成 —— 请勿手改,改动请在 ProxyHub 界面操作。\n");
    s.push_str("profile:\n  store-selected: true\n  store-fake-ip: true\n\n");
    s.push_str("prepend-proxy-groups:\n");
    s.push_str("  - name: \"⚡ 自动选择\"\n    type: url-test\n    include-all-proxies: true\n    url: https://www.gstatic.com/generate_204\n    interval: 300\n    tolerance: 50\n");
    s.push_str("  - name: \"🌍 国外流量\"\n    type: select\n    include-all-proxies: true\n    proxies:\n      - \"⚡ 自动选择\"\n\n");
    s.push_str("prepend-rules:\n");
    s.push_str("  - GEOIP,private,DIRECT,no-resolve\n");
    // 用户规则(最高优先)
    for r in &model.rules {
        let ty = cat_to_type(&r.category);
        let tail = if ty == "GEOIP" || ty == "IP-CIDR" || ty == "IP-CIDR6" {
            ",no-resolve"
        } else {
            ""
        };
        s.push_str(&format!("  - {},{},{}{}\n", ty, r.value, r.target, tail));
    }
    // 兜底:中国直连
    s.push_str("  - GEOSITE,cn,DIRECT\n");
    s.push_str("  - GEOSITE,apple-cn,DIRECT\n");
    s.push_str("  - GEOIP,CN,DIRECT,no-resolve\n");
    s
}

#[tauri::command]
pub fn routing_get() -> Result<String, String> {
    match fs::read_to_string(routing_json()) {
        Ok(s) => Ok(s),
        Err(_) => Ok(serde_json::to_string(&RoutingModel::default()).unwrap()),
    }
}

#[tauri::command]
pub fn routing_save(json: String) -> Result<String, String> {
    let model: RoutingModel = serde_json::from_str(&json).map_err(|e| format!("非法模型: {e}"))?;
    // 1) 备份当前 Merge.yaml
    let mut backed = String::from("(无旧文件)");
    if let Ok(cur) = fs::read_to_string(merge_yaml()) {
        let ts = sh("date", &["+%Y%m%d-%H%M%S"]).unwrap_or_default();
        let ts = ts.trim();
        let bp = format!("{}/Merge-{}.yaml", backup_dir(), ts);
        write_file(&bp, &cur)?;
        backed = bp;
    }
    // 2) 保存模型(ProxyHub 自身才是权威)
    write_file(&routing_json(), &json)?;
    // 3) 生成并写 Merge.yaml
    let expect = gen_merge(&model);
    write_file(&merge_yaml(), &expect)?;
    // 4) 立即读回校验,确认真的落盘(而非静默失败/被瞬间重置)
    let landed = fs::read_to_string(merge_yaml()).map_err(|e| format!("写入后读回失败: {e}"))?;
    if !landed.contains(MARKER) {
        return Err("Merge.yaml 写入后未检测到 ProxyHub 标记——可能被其他程序即时覆盖,请关闭 Clash Verge 后重试。".into());
    }
    // 5) 正在运行时,规则需 Clash Verge「重新激活」才会并入运行配置——如实告知,不假装已生效
    let tail = if clash_running() {
        " · Clash Verge 运行中:需在其界面「重新激活/Reactivate」当前配置(或重启 Clash Verge)后规则才生效"
    } else {
        ""
    };
    Ok(format!("已写回 {} 条规则,备份: {}{}", model.rules.len(), backed, tail))
}

/// 一键从 routing.json 重新下发 Merge.yaml(用于被 Clash Verge 重置后重新应用)。
#[tauri::command]
pub fn routing_reapply() -> Result<String, String> {
    let json = fs::read_to_string(routing_json()).map_err(|e| format!("无 routing.json: {e}"))?;
    routing_save(json)
}

#[tauri::command]
pub fn routing_backups() -> Result<Vec<String>, String> {
    let dir = backup_dir();
    let mut out = vec![];
    if let Ok(rd) = fs::read_dir(&dir) {
        for e in rd.flatten() {
            if let Some(n) = e.file_name().to_str() {
                out.push(n.to_string());
            }
        }
    }
    out.sort();
    out.reverse();
    Ok(out)
}

#[tauri::command]
pub fn routing_rollback(name: String) -> Result<String, String> {
    let src = format!("{}/{}", backup_dir(), name);
    let cur = fs::read_to_string(&src).map_err(|e| e.to_string())?;
    write_file(&merge_yaml(), &cur)?;
    Ok(format!("已回滚到 {name}"))
}

#[derive(Serialize)]
pub struct RoutingStatus {
    clash_running: bool,
    merge_exists: bool,
    merge_managed: bool,        // Merge.yaml 当前是否为 ProxyHub 下发(含标记)
    subscription_present: bool, // 是否已导入订阅(否则策略组为空,规则无出口)
    model_rules: usize,         // ProxyHub 规则数(权威)
    runtime_rules: i64,         // mihomo 运行时规则数(-1=未知)
    runtime_groups: Vec<String>, // mihomo 运行时真实策略组
    notes: Vec<String>,         // 面向用户的如实提示
}

/// 分流真实状态:ProxyHub 规则 vs 磁盘 Merge vs mihomo 运行时,把"是否真生效"讲清楚。
#[tauri::command]
pub fn routing_status() -> Result<RoutingStatus, String> {
    let running = clash_running();
    let merge = fs::read_to_string(merge_yaml());
    let merge_exists = merge.is_ok();
    let merge_managed = merge.as_deref().map(|s| s.contains(MARKER)).unwrap_or(false);

    // 订阅:profiles 目录下有非 Merge.yaml 的 .yaml 即视为已导入订阅
    let mut subscription_present = false;
    if let Ok(rd) = fs::read_dir(format!("{}/profiles", clash_app_dir())) {
        for e in rd.flatten() {
            let n = e.file_name().to_string_lossy().to_string();
            if n.ends_with(".yaml") && n != "Merge.yaml" {
                subscription_present = true;
            }
        }
    }

    let model_rules = fs::read_to_string(routing_json())
        .ok()
        .and_then(|s| serde_json::from_str::<RoutingModel>(&s).ok())
        .map(|m| m.rules.len())
        .unwrap_or(0);

    let mut runtime_rules = -1i64;
    let mut runtime_groups = vec![];
    if running {
        if let Ok(j) = mihomo_get("/rules") {
            if let Ok(v) = serde_json::from_str::<serde_json::Value>(&j) {
                if let Some(a) = v.get("rules").and_then(|x| x.as_array()) {
                    runtime_rules = a.len() as i64;
                }
            }
        }
        if let Ok(j) = mihomo_get("/proxies") {
            if let Ok(v) = serde_json::from_str::<serde_json::Value>(&j) {
                if let Some(m) = v.get("proxies").and_then(|x| x.as_object()) {
                    for (k, val) in m {
                        let is_group = val
                            .get("type")
                            .and_then(|t| t.as_str())
                            .map(|t| matches!(t, "Selector" | "URLTest" | "Fallback" | "LoadBalance" | "Relay"))
                            .unwrap_or(false);
                        if is_group {
                            runtime_groups.push(k.clone());
                        }
                    }
                }
            }
        }
        runtime_groups.sort();
    }

    let mut notes = vec![];
    if !subscription_present {
        notes.push("未检测到订阅:策略组为空,去「🌍 国外流量」的规则当前无可用出口。请先在 Clash Verge 导入机场订阅。".into());
    }
    if running && merge_exists && !merge_managed {
        notes.push("Merge.yaml 已被 Clash Verge 重置为默认,ProxyHub 规则当前未下发——点「重新下发」。".into());
    }
    if running && merge_managed && runtime_rules >= 0 && (runtime_rules as usize) < model_rules {
        notes.push("规则已写入 Merge.yaml,但 mihomo 运行时规则数偏少——请在 Clash Verge 中「重新激活」配置使其并入。".into());
    }
    if !running {
        notes.push("Clash Verge 未运行:规则不会生效,这里只做静态管理。".into());
    }

    Ok(RoutingStatus {
        clash_running: running,
        merge_exists,
        merge_managed,
        subscription_present,
        model_rules,
        runtime_rules,
        runtime_groups,
        notes,
    })
}

#[derive(Serialize)]
pub struct DiagHit {
    rule_type: String,
    value: String,
    target: String,
    certainty: String, // 命中 | 候选 | 兜底
}

#[derive(Serialize)]
pub struct DiagResult {
    host: String,
    hits: Vec<DiagHit>,
    conclusion: String,
}

/// 域名/IP 分流诊断:按规则顺序模拟,显式域名/IP 规则可确定命中,
/// GEOSITE/GEOIP 这类按地理库判定的只标为"候选"(最终以 mihomo 为准)。
#[tauri::command]
pub fn routing_diagnose(host: String) -> Result<DiagResult, String> {
    let host = host.trim().trim_start_matches("http://").trim_start_matches("https://");
    let host = host.split('/').next().unwrap_or(host).to_lowercase();
    if host.is_empty() {
        return Err("请输入域名或 IP".into());
    }
    let is_ip = host.split('.').all(|p| p.parse::<u8>().is_ok()) && host.matches('.').count() == 3;

    let model: RoutingModel = fs::read_to_string(routing_json())
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default();

    let mut hits = vec![];
    let mut decided: Option<String> = None;

    for r in &model.rules {
        let ty = cat_to_type(&r.category);
        match r.category.as_str() {
            "domain" => {
                let v = r.value.to_lowercase();
                if host == v || host.ends_with(&format!(".{v}")) {
                    hits.push(DiagHit { rule_type: ty.into(), value: r.value.clone(), target: r.target.clone(), certainty: "命中".into() });
                    if decided.is_none() { decided = Some(r.target.clone()); }
                }
            }
            "ipcidr" | "geoip" => {
                if is_ip {
                    // 真正的 CIDR/GEOIP 归属需运行时判定,这里只作候选
                    hits.push(DiagHit { rule_type: ty.into(), value: r.value.clone(), target: r.target.clone(), certainty: "候选".into() });
                }
            }
            "geosite" => {
                if !is_ip {
                    hits.push(DiagHit { rule_type: ty.into(), value: r.value.clone(), target: r.target.clone(), certainty: "候选".into() });
                }
            }
            "app" | "port" => { /* 与单个域名无关 */ }
            _ => {}
        }
    }

    // 兜底(gen_merge 固定追加):中国站点直连
    if !is_ip {
        for (v, _t) in [("cn", "DIRECT"), ("apple-cn", "DIRECT")] {
            hits.push(DiagHit { rule_type: "GEOSITE".into(), value: v.into(), target: "DIRECT".into(), certainty: "兜底".into() });
        }
    }
    hits.push(DiagHit { rule_type: "GEOIP".into(), value: "CN".into(), target: "DIRECT".into(), certainty: "兜底".into() });

    let conclusion = if let Some(t) = &decided {
        format!("显式规则命中 → 走「{t}」。")
    } else if hits.iter().any(|h| h.certainty == "候选") {
        "无显式域名规则直接命中;最终取决于 GEOSITE/GEOIP 地理库归属(以上为按顺序的候选规则),由 mihomo 在运行时判定。".into()
    } else {
        "未命中任何上层规则,将落到兜底:国内站点直连,其余由订阅默认策略处理。".into()
    };

    Ok(DiagResult { host: host.to_string(), hits, conclusion })
}

/// 从现有 Merge.yaml 的 prepend-rules 解析出结构化模型。
#[tauri::command]
pub fn routing_import_clash() -> Result<String, String> {
    let content = fs::read_to_string(merge_yaml()).map_err(|e| e.to_string())?;
    let mut rules = vec![];
    let mut in_rules = false;
    for line in content.lines() {
        let t = line.trim();
        if t.starts_with("prepend-rules:") || t.starts_with("rules:") {
            in_rules = true;
            continue;
        }
        if in_rules {
            if !t.starts_with("- ") {
                if !t.is_empty() && !line.starts_with(' ') {
                    break;
                }
                continue;
            }
            let body = t.trim_start_matches("- ");
            let parts: Vec<&str> = body.split(',').collect();
            if parts.len() >= 3 {
                let ty = parts[0].trim();
                if ty == "GEOIP" && parts[1].trim() == "private" {
                    continue;
                }
                rules.push(Rule {
                    category: type_to_cat(ty).to_string(),
                    value: parts[1].trim().to_string(),
                    target: parts[2].trim().to_string(),
                });
            }
        }
    }
    serde_json::to_string(&RoutingModel { rules }).map_err(|e| e.to_string())
}

#[derive(Serialize)]
pub struct AppItem {
    name: String,
    process: String, // 真实可执行/进程名(mihomo PROCESS-NAME 匹配用)
    path: String,
}

// 平铺扫描单个目录(不递归、跳过符号链接别名,避免回环与重复开销)。
fn scan_apps_flat(dir: &str, out: &mut Vec<AppItem>) {
    let rd = match fs::read_dir(dir) {
        Ok(r) => r,
        Err(_) => return,
    };
    for e in rd.flatten() {
        // 跳过符号链接(如 ~/Applications/分类 下的别名),防回环空转
        if e.path()
            .symlink_metadata()
            .map(|m| m.file_type().is_symlink())
            .unwrap_or(false)
        {
            continue;
        }
        let fname = e.file_name().to_string_lossy().to_string();
        if !fname.ends_with(".app") {
            continue;
        }
        let path = e.path();
        // 进程名 = Contents/MacOS/ 里的主可执行文件名(mihomo PROCESS-NAME 匹配用)
        let mut process = fname.trim_end_matches(".app").to_string();
        if let Ok(macos) = fs::read_dir(path.join("Contents/MacOS")) {
            if let Some(f) = macos.flatten().next() {
                process = f.file_name().to_string_lossy().to_string();
            }
        }
        out.push(AppItem {
            name: fname.trim_end_matches(".app").to_string(),
            process,
            path: path.to_string_lossy().to_string(),
        });
    }
}

/// 枚举本机全部 App 及其真实进程名(固定目录、平铺、不跟符号链接——快且不卡)。
#[tauri::command]
pub fn list_apps() -> Result<Vec<AppItem>, String> {
    let h = home();
    let dirs = [
        "/Applications".to_string(),
        "/Applications/Utilities".to_string(),
        "/System/Applications".to_string(),
        "/System/Applications/Utilities".to_string(),
        format!("{h}/Applications"),
    ];
    let mut out = vec![];
    for d in dirs {
        scan_apps_flat(&d, &mut out);
    }
    out.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    out.dedup_by(|a, b| a.name == b.name);
    Ok(out)
}

/// 读取所有浏览器(Chrome/Edge/Brave 各身份 + Safari)的书签/收藏,提取去重域名。
#[tauri::command]
pub fn import_bookmarks() -> Result<Vec<String>, String> {
    let base = format!("{}/Library/Application Support", home());
    let mut domains = std::collections::BTreeSet::new();

    // Chromium 系:Chrome / Edge / Brave,各含多 profile,书签是 JSON
    let chromium_roots = [
        format!("{base}/Google/Chrome"),
        format!("{base}/Microsoft Edge"),
        format!("{base}/BraveSoftware/Brave-Browser"),
    ];
    for root in chromium_roots {
        let mut profiles = vec!["Default".to_string()];
        if let Ok(rd) = fs::read_dir(&root) {
            for e in rd.flatten() {
                let n = e.file_name().to_string_lossy().to_string();
                if n.starts_with("Profile ") {
                    profiles.push(n);
                }
            }
        }
        for p in profiles {
            if let Ok(s) = fs::read_to_string(format!("{root}/{p}/Bookmarks")) {
                if let Ok(v) = serde_json::from_str::<serde_json::Value>(&s) {
                    collect_urls(&v, &mut domains);
                }
            }
        }
    }

    // Safari 书签是 plist(需 Full Disk Access,拿不到会被跳过)
    let safari = format!("{}/Library/Safari/Bookmarks.plist", home());
    if std::path::Path::new(&safari).exists() {
        if let Ok(out) = sh("plutil", &["-convert", "json", "-o", "-", &safari]) {
            if let Ok(v) = serde_json::from_str::<serde_json::Value>(&out) {
                collect_urls(&v, &mut domains);
            }
        }
    }

    // 书签 HTML 导出(浏览器未同步到本地时的真实来源,如迁移导出目录)
    let html_dirs = [format!("{}/Workspace/C-Users-aaron/浏览器书签", home())];
    for dir in html_dirs {
        if let Ok(rd) = fs::read_dir(&dir) {
            for e in rd.flatten() {
                let p = e.path();
                if p.extension().and_then(|x| x.to_str()) == Some("html") {
                    if let Ok(s) = fs::read_to_string(&p) {
                        collect_html_hrefs(&s, &mut domains);
                    }
                }
            }
        }
    }

    Ok(domains.into_iter().collect())
}

/// 从书签 HTML 导出里提取 href 域名(手写扫描,不引入正则依赖)。
fn collect_html_hrefs(html: &str, out: &mut std::collections::BTreeSet<String>) {
    let lower = html.to_lowercase();
    let bytes = html.as_bytes();
    let mut idx = 0;
    while let Some(pos) = lower[idx..].find("href=") {
        let start = idx + pos + 5;
        idx = start;
        if start >= bytes.len() {
            break;
        }
        let quote = bytes[start];
        if quote != b'"' && quote != b'\'' {
            continue;
        }
        let content_start = start + 1;
        if let Some(end) = html[content_start..].find(quote as char) {
            let url = &html[content_start..content_start + end];
            if let Some(host) = url
                .split("//")
                .nth(1)
                .and_then(|r| r.split('/').next())
                .map(|h| h.trim_start_matches("www."))
            {
                if host.contains('.') {
                    out.insert(host.to_string());
                }
            }
            idx = content_start + end;
        }
    }
}

fn collect_urls(v: &serde_json::Value, out: &mut std::collections::BTreeSet<String>) {
    match v {
        serde_json::Value::Object(m) => {
            // Chromium 用 "url";Safari 用 "URLString"
            for key in ["url", "URLString"] {
                if let Some(serde_json::Value::String(url)) = m.get(key) {
                    if let Some(host) = url
                        .split("//")
                        .nth(1)
                        .and_then(|r| r.split('/').next())
                        .map(|h| h.trim_start_matches("www."))
                    {
                        if host.contains('.') {
                            out.insert(host.to_string());
                        }
                    }
                }
            }
            for (_, val) in m {
                collect_urls(val, out);
            }
        }
        serde_json::Value::Array(a) => {
            for val in a {
                collect_urls(val, out);
            }
        }
        _ => {}
    }
}

/// 读取当前订阅里的真实策略组名(作为"去向"选项),外加 DIRECT/REJECT 与本工具自建组。
#[tauri::command]
pub fn clash_groups() -> Result<Vec<String>, String> {
    let dir = format!(
        "{}/Library/Application Support/io.github.clash-verge-rev.clash-verge-rev/profiles",
        home()
    );
    let mut groups: Vec<String> = vec![
        "🌍 国外流量".into(),
        "⚡ 自动选择".into(),
        "DIRECT".into(),
        "REJECT".into(),
    ];
    if let Ok(rd) = fs::read_dir(&dir) {
        for e in rd.flatten() {
            let n = e.file_name().to_string_lossy().to_string();
            if !n.ends_with(".yaml") || n == "Merge.yaml" {
                continue;
            }
            if let Ok(content) = fs::read_to_string(e.path()) {
                parse_group_names(&content, &mut groups);
            }
        }
    }
    Ok(groups)
}

fn parse_group_names(content: &str, out: &mut Vec<String>) {
    let mut in_groups = false;
    for line in content.lines() {
        if line.starts_with("proxy-groups:") {
            in_groups = true;
            continue;
        }
        if in_groups {
            // 遇到下一个顶层键则结束
            if !line.starts_with(' ') && !line.starts_with('-') && !line.trim().is_empty() {
                break;
            }
            let t = line.trim_start();
            let name = t
                .strip_prefix("- name:")
                .or_else(|| t.strip_prefix("name:"))
                .map(|r| r.trim().trim_matches('"').trim_matches('\'').to_string());
            if let Some(name) = name {
                if !name.is_empty() && !out.contains(&name) {
                    out.push(name);
                }
            }
        }
    }
}

#[tauri::command]
pub fn launch_clash() -> Result<String, String> {
    sh("open", &["-a", "Clash Verge"])?;
    Ok("已唤起 Clash Verge".into())
}

/// 出口 IP 纯净度检测:地区 + 是否被识别为代理/机房 IP。
#[tauri::command]
pub fn exit_ip_check() -> Result<String, String> {
    sh(
        "curl",
        &[
            "-s",
            "--max-time",
            "8",
            "http://ip-api.com/json/?fields=query,country,city,isp,org,as,proxy,hosting,mobile",
        ],
    )
}
