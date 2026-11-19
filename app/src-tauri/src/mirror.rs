// 模块一:镜像源管理。与 Go 引擎(本地路由代理)通过 HTTP 控制接口交互,并读写其配置。
use crate::common::{home, sh, write_file};
use serde_json::{json, Value};
use std::fs;
use tauri::Manager;

fn cfg_path() -> String {
    format!("{}/.config/proxyhub/config.json", home())
}

/// 引擎二进制候选:优先用打包进 app 的,其次用工作区构建产物。
fn engine_candidates(app: &tauri::AppHandle) -> Vec<String> {
    let mut v = vec![];
    if let Ok(p) = app
        .path()
        .resolve("resources/proxyhub-engine", tauri::path::BaseDirectory::Resource)
    {
        v.push(p.to_string_lossy().to_string());
    }
    v.push(format!("{}/Workspace/proxyhub/engine/proxyhub", home()));
    v
}

/// 若引擎未运行则启动(app 启动时调用)。
pub fn ensure_engine(app: &tauri::AppHandle) {
    if engine_running() {
        return;
    }
    for bin in engine_candidates(app) {
        if std::path::Path::new(&bin).exists() {
            let _ = std::process::Command::new(&bin).spawn();
            return;
        }
    }
}

fn port() -> u16 {
    if let Ok(s) = fs::read_to_string(cfg_path()) {
        if let Ok(v) = serde_json::from_str::<serde_json::Value>(&s) {
            if let Some(p) = v.get("port").and_then(|p| p.as_u64()) {
                return p as u16;
            }
        }
    }
    8899
}

fn api(path: &str) -> Result<String, String> {
    sh(
        "curl",
        &[
            "-s",
            "--max-time",
            "3",
            &format!("http://127.0.0.1:{}{}", port(), path),
        ],
    )
}

#[tauri::command]
pub fn engine_status() -> Result<String, String> {
    api("/_ph/status")
}

#[tauri::command]
pub fn engine_diagnose() -> Result<String, String> {
    api("/_ph/diagnose")
}

#[tauri::command]
pub fn engine_cache_clear() -> Result<String, String> {
    api("/_ph/cache/clear")
}

#[tauri::command]
pub fn engine_running() -> bool {
    api("/_ph/status").is_ok()
}

#[tauri::command]
pub fn engine_start(app: tauri::AppHandle) -> Result<String, String> {
    if engine_running() {
        return Ok("引擎已在运行".into());
    }
    for bin in engine_candidates(&app) {
        if std::path::Path::new(&bin).exists() {
            std::process::Command::new(&bin)
                .spawn()
                .map_err(|e| e.to_string())?;
            // 确认真的在端口上监听,避免端口冲突导致"假启动"
            for _ in 0..12 {
                std::thread::sleep(std::time::Duration::from_millis(300));
                if engine_running() {
                    return Ok("引擎已启动".into());
                }
            }
            return Err(format!(
                "引擎进程已拉起但未在端口 {} 监听(可能端口被占用或引擎异常退出)",
                port()
            ));
        }
    }
    Err("未找到引擎二进制(resources/proxyhub-engine 或工作区 engine/proxyhub)".into())
}

#[tauri::command]
pub fn engine_stop() -> Result<String, String> {
    // 按端口精确定位引擎进程并结束(避免误杀同名/路径含 proxyhub 的进程)
    let p = format!("tcp:{}", port());
    if let Ok(pids) = sh("lsof", &["-ti", &p]) {
        for pid in pids.split_whitespace() {
            let _ = sh("kill", &[pid]);
        }
    }
    Ok("引擎已停止".into())
}

#[derive(serde::Serialize)]
pub struct ToolPointing {
    tool: String,
    current: String,
    on_engine: bool,
}

/// 看 pip/npm/go 当前各指向哪(官方 / 国内源 / 本引擎)。
#[tauri::command]
pub fn tool_pointing() -> Result<Vec<ToolPointing>, String> {
    let h = home();
    let eng = format!("127.0.0.1:{}", port());
    let mut out = vec![];

    let pip = fs::read_to_string(format!("{h}/.config/pip/pip.conf")).unwrap_or_default();
    let pip_url = pip
        .lines()
        .find(|l| l.contains("index-url"))
        .and_then(|l| l.split('=').nth(1))
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|| "官方 pypi.org".into());
    out.push(ToolPointing { on_engine: pip_url.contains(&eng), tool: "pip".into(), current: pip_url });

    let npm = sh("npm", &["config", "get", "registry"]).unwrap_or_else(|_| "?".into()).trim().to_string();
    out.push(ToolPointing { on_engine: npm.contains(&eng), tool: "npm".into(), current: npm });

    let go = sh("go", &["env", "GOPROXY"]).unwrap_or_default().trim().to_string();
    out.push(ToolPointing { on_engine: go.contains(&eng), tool: "go".into(), current: if go.is_empty() { "?".into() } else { go } });

    Ok(out)
}

/// 一键让 pip/npm/go 指向本引擎(所有拉取走引擎:择源+故障转移+缓存)。
#[tauri::command]
pub fn engine_takeover() -> Result<String, String> {
    let base = format!("http://127.0.0.1:{}", port());
    write_file(
        &format!("{}/.config/pip/pip.conf", home()),
        &format!("[global]\nindex-url = {base}/pypi/\ntrusted-host = 127.0.0.1\n"),
    )?;
    let _ = sh("npm", &["config", "set", "registry", &format!("{base}/npm/")]);
    let _ = sh("go", &["env", "-w", &format!("GOPROXY={base}/go/")]);
    Ok("pip / npm / go 已指向引擎".into())
}

/// 一键还原 pip/npm/go 为官方源。
#[tauri::command]
pub fn engine_restore() -> Result<String, String> {
    write_file(
        &format!("{}/.config/pip/pip.conf", home()),
        "[global]\nindex-url = https://pypi.org/simple/\n",
    )?;
    let _ = sh("npm", &["config", "set", "registry", "https://registry.npmjs.org"]);
    let _ = sh("go", &["env", "-w", "GOPROXY=https://proxy.golang.org,direct"]);
    Ok("pip / npm / go 已还原官方源".into())
}

#[tauri::command]
pub fn mirror_config_get() -> Result<String, String> {
    fs::read_to_string(cfg_path()).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn mirror_config_save(json: String) -> Result<(), String> {
    // 校验是合法 JSON 再写
    serde_json::from_str::<serde_json::Value>(&json).map_err(|e| format!("非法 JSON: {e}"))?;
    write_file(&cfg_path(), &json)
}

// ---- 镜像源增删(改配置 → 重启引擎应用) ----

fn load_cfg_value() -> Result<Value, String> {
    let s = fs::read_to_string(cfg_path()).map_err(|e| format!("读配置失败: {e}"))?;
    serde_json::from_str(&s).map_err(|e| format!("配置解析失败: {e}"))
}

fn save_cfg_value(v: &Value) -> Result<(), String> {
    let s = serde_json::to_string_pretty(v).map_err(|e| e.to_string())?;
    write_file(&cfg_path(), &s)
}

/// 引擎在启动时读取 config.json,改动后需重启才生效:先停、等端口释放、再起。
fn restart_engine(app: &tauri::AppHandle) -> Result<(), String> {
    let _ = engine_stop();
    for _ in 0..15 {
        if !engine_running() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(200));
    }
    engine_start(app.clone()).map(|_| ())
}

fn validate_base(base: &str) -> Result<(), String> {
    if !base.starts_with("http://") && !base.starts_with("https://") {
        return Err("镜像地址需以 http:// 或 https:// 开头".into());
    }
    Ok(())
}

/// 给某个生态新增一个上游镜像(name + base),写入配置并重载引擎。
#[tauri::command]
pub fn mirror_add_upstream(
    app: tauri::AppHandle,
    eco: String,
    name: String,
    base: String,
) -> Result<String, String> {
    let name = name.trim().to_string();
    let base = base.trim().trim_end_matches(' ').to_string();
    if name.is_empty() || base.is_empty() {
        return Err("镜像名称和地址不能为空".into());
    }
    validate_base(&base)?;
    let mut cfg = load_cfg_value()?;
    let ecos = cfg
        .get_mut("ecosystems")
        .and_then(|e| e.as_array_mut())
        .ok_or("配置缺少 ecosystems")?;
    let eco_obj = ecos
        .iter_mut()
        .find(|e| e.get("name").and_then(|n| n.as_str()) == Some(eco.as_str()))
        .ok_or_else(|| format!("未找到生态: {eco}"))?;
    let ups = eco_obj
        .get_mut("upstreams")
        .and_then(|u| u.as_array_mut())
        .ok_or("该生态缺少 upstreams")?;
    if ups
        .iter()
        .any(|u| u.get("name").and_then(|n| n.as_str()) == Some(name.as_str()))
    {
        return Err(format!("上游「{name}」已存在于 {eco}"));
    }
    ups.push(json!({ "name": name, "base": base }));
    save_cfg_value(&cfg)?;
    restart_engine(&app)?;
    Ok(format!("已给 {eco} 新增镜像「{name}」,引擎已重载"))
}

/// 从某个生态移除一个上游镜像(不允许移到 0 个)。
#[tauri::command]
pub fn mirror_remove_upstream(
    app: tauri::AppHandle,
    eco: String,
    name: String,
) -> Result<String, String> {
    let mut cfg = load_cfg_value()?;
    let ecos = cfg
        .get_mut("ecosystems")
        .and_then(|e| e.as_array_mut())
        .ok_or("配置缺少 ecosystems")?;
    let eco_obj = ecos
        .iter_mut()
        .find(|e| e.get("name").and_then(|n| n.as_str()) == Some(eco.as_str()))
        .ok_or_else(|| format!("未找到生态: {eco}"))?;
    let ups = eco_obj
        .get_mut("upstreams")
        .and_then(|u| u.as_array_mut())
        .ok_or("该生态缺少 upstreams")?;
    if ups.len() <= 1 {
        return Err(format!("{eco} 至少要保留一个镜像,不能删光"));
    }
    let before = ups.len();
    ups.retain(|u| u.get("name").and_then(|n| n.as_str()) != Some(name.as_str()));
    if ups.len() == before {
        return Err(format!("{eco} 下没有名为「{name}」的镜像"));
    }
    save_cfg_value(&cfg)?;
    restart_engine(&app)?;
    Ok(format!("已从 {eco} 移除「{name}」,引擎已重载"))
}

/// 新增一个生态(如某私有仓库):name + 路由前缀 + 首个上游镜像。
#[tauri::command]
pub fn mirror_add_ecosystem(
    app: tauri::AppHandle,
    name: String,
    prefix: String,
    up_name: String,
    up_base: String,
) -> Result<String, String> {
    let name = name.trim().to_string();
    let up_name = up_name.trim().to_string();
    let up_base = up_base.trim().to_string();
    if name.is_empty() || up_name.is_empty() || up_base.is_empty() {
        return Err("生态名、首个镜像名与地址都不能为空".into());
    }
    validate_base(&up_base)?;
    // 规整前缀为 /xxx/
    let mut prefix = prefix.trim().to_string();
    if prefix.is_empty() {
        prefix = format!("/{name}/");
    }
    if !prefix.starts_with('/') {
        prefix.insert(0, '/');
    }
    if !prefix.ends_with('/') {
        prefix.push('/');
    }
    let mut cfg = load_cfg_value()?;
    let ecos = cfg
        .get_mut("ecosystems")
        .and_then(|e| e.as_array_mut())
        .ok_or("配置缺少 ecosystems")?;
    if ecos
        .iter()
        .any(|e| e.get("name").and_then(|n| n.as_str()) == Some(name.as_str()))
    {
        return Err(format!("生态「{name}」已存在"));
    }
    if ecos
        .iter()
        .any(|e| e.get("prefix").and_then(|p| p.as_str()) == Some(prefix.as_str()))
    {
        return Err(format!("前缀「{prefix}」已被其它生态占用"));
    }
    ecos.push(json!({
        "name": name,
        "prefix": prefix,
        "upstreams": [ { "name": up_name, "base": up_base } ]
    }));
    save_cfg_value(&cfg)?;
    restart_engine(&app)?;
    Ok(format!("已新增生态「{name}」(前缀 {prefix}),引擎已重载"))
}
