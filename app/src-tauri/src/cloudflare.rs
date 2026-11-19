// 模块三.6:Cloudflare 控制台 + 域名邮箱(Email Routing)。
// token 从 Keychain 读(名 CLOUDFLARE_API_TOKEN),直接调 Cloudflare API。
use crate::common::sh;

const SERVICE: &str = "proxyhub";

// 从 Keychain 取指定用途的 token:all-round(CLOUDFLARE_API_TOKEN)或邮件分部(CLOUDFLARE_EMAIL_TOKEN)。
fn cf_token(account: &str) -> Result<String, String> {
    sh(
        "security",
        &["find-generic-password", "-s", SERVICE, "-a", account, "-w"],
    )
    .map(|s| s.trim_end().to_string())
    .map_err(|_| format!("未在凭据库找到 {account}(先在「凭据账号」存入)"))
}

fn cf_api(method: &str, path: &str, body: Option<&str>) -> Result<String, String> {
    cf_api_with("CLOUDFLARE_API_TOKEN", method, path, body)
}

fn cf_api_with(token_account: &str, method: &str, path: &str, body: Option<&str>) -> Result<String, String> {
    let token = cf_token(token_account)?;
    let url = format!("https://api.cloudflare.com/client/v4{path}");
    let auth = format!("Authorization: Bearer {token}");
    let mut args: Vec<String> = vec![
        "-s".into(),
        "--max-time".into(),
        "15".into(),
        "-X".into(),
        method.into(),
        "-H".into(),
        auth,
        "-H".into(),
        "Content-Type: application/json".into(),
    ];
    if let Some(b) = body {
        args.push("-d".into());
        args.push(b.into());
    }
    args.push(url);
    let refs: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
    sh("curl", &refs)
}

/// 校验 token 是否有效。
#[tauri::command]
pub fn cf_verify() -> Result<String, String> {
    cf_api("GET", "/user/tokens/verify", None)
}

/// 列出 zone(域名)。
#[tauri::command]
pub fn cf_zones() -> Result<String, String> {
    cf_api("GET", "/zones?per_page=50", None)
}

/// 列出某 zone 的邮箱路由规则(catch-all / 别名 / 转发目标)。
#[tauri::command]
pub fn cf_email_rules(zone_id: String) -> Result<String, String> {
    cf_api_with(
        "CLOUDFLARE_EMAIL_TOKEN",
        "GET",
        &format!("/zones/{zone_id}/email/routing/rules"),
        None,
    )
}

/// 列出已验证的转发目标地址。
#[tauri::command]
pub fn cf_email_destinations(account_id: String) -> Result<String, String> {
    cf_api_with(
        "CLOUDFLARE_EMAIL_TOKEN",
        "GET",
        &format!("/accounts/{account_id}/email/routing/addresses"),
        None,
    )
}

/// 新增一条别名转发规则:alias@域名 → dest。
#[tauri::command]
pub fn cf_email_add(zone_id: String, alias: String, dest: String) -> Result<String, String> {
    let body = serde_json::json!({
        "enabled": true,
        "name": format!("ProxyHub: {alias}"),
        "matchers": [{"type":"literal","field":"to","value":alias}],
        "actions": [{"type":"forward","value":[dest]}]
    })
    .to_string();
    cf_api_with(
        "CLOUDFLARE_EMAIL_TOKEN",
        "POST",
        &format!("/zones/{zone_id}/email/routing/rules"),
        Some(&body),
    )
}

/// 删除一条邮箱路由规则。
#[tauri::command]
pub fn cf_email_delete(zone_id: String, rule_id: String) -> Result<String, String> {
    cf_api_with(
        "CLOUDFLARE_EMAIL_TOKEN",
        "DELETE",
        &format!("/zones/{zone_id}/email/routing/rules/{rule_id}"),
        None,
    )
}

/// 打开 Cloudflare 控制台的某个区域(省去每次手动登录找页面)。
#[tauri::command]
pub fn cf_open_console(section: String) -> Result<String, String> {
    let url = match section.as_str() {
        "workers" => "https://dash.cloudflare.com/?to=/:account/workers-and-pages",
        "email" => "https://dash.cloudflare.com/?to=/:account/:zone/email/routing",
        "d1" => "https://dash.cloudflare.com/?to=/:account/workers/d1",
        "r2" => "https://dash.cloudflare.com/?to=/:account/r2",
        _ => "https://dash.cloudflare.com",
    };
    sh("open", &[url])?;
    Ok(format!("已打开 {section} 控制台"))
}
