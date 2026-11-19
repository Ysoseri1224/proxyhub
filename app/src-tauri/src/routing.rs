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

fn routing_json() -> String {
    format!("{}/.config/proxyhub/routing.json", home())
}
fn merge_yaml() -> String {
    format!(
        "{}/Library/Application Support/io.github.clash-verge-rev.clash-verge-rev/profiles/Merge.yaml",
        home()
    )
}
fn backup_dir() -> String {
    format!("{}/.config/proxyhub/routing-backups", home())
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
    // 2) 保存模型
    write_file(&routing_json(), &json)?;
    // 3) 生成并写 Merge.yaml
    write_file(&merge_yaml(), &gen_merge(&model))?;
    // 4) Clash Verge 正在运行时会管理/可能覆盖该文件,明确提示而非静默
    let clash_running = sh("pgrep", &["-x", "clash-verge"])
        .map(|s| !s.trim().is_empty())
        .unwrap_or(false);
    let warn = if clash_running {
        " ⚠ Clash Verge 正在运行:请在其中「重新激活订阅」以加载;若规则被覆盖,关闭 Clash Verge 后再保存。"
    } else {
        ""
    };
    Ok(format!("已写回({} 条规则),备份: {}{}", model.rules.len(), backed, warn))
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

fn scan_apps(dir: &str, out: &mut Vec<AppItem>) {
    let rd = match fs::read_dir(dir) {
        Ok(r) => r,
        Err(_) => return,
    };
    for e in rd.flatten() {
        let fname = e.file_name().to_string_lossy().to_string();
        let path = e.path();
        if fname.ends_with(".app") {
            // 进程名 = Contents/MacOS/ 里的主可执行文件名
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
        } else if path.is_dir() {
            // 递归一层(Utilities 等子目录)
            scan_apps(&path.to_string_lossy(), out);
        }
    }
}

/// 枚举本机全部 App 及其真实进程名(含系统、工具、用户目录)。
#[tauri::command]
pub fn list_apps() -> Result<Vec<AppItem>, String> {
    let dirs = [
        "/Applications".to_string(),
        "/System/Applications".to_string(),
        format!("{}/Applications", home()),
    ];
    let mut out = vec![];
    for d in dirs {
        scan_apps(&d, &mut out);
    }
    out.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    out.dedup_by(|a, b| a.process == b.process && a.name == b.name);
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
