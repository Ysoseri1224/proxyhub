// ProxyHub —— 统一管理入口。后端命令按模块拆分。
mod cloudflare;
mod common;
mod creds;
mod mirror;
mod routing;
mod verify;

pub fn headless_report(uid: Option<String>) {
    verify::headless_report(uid);
}

/// launchd 定时采样的无界面入口
pub fn headless_sample() {
    verify::headless_sample();
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            common::ensure_path(); // 注入 PATH,GUI 下也能找到 gh/npm/wrangler 等
            mirror::ensure_engine(app.handle()); // 启动即拉起镜像引擎
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            // 模块一 · 镜像源
            mirror::engine_status,
            mirror::engine_diagnose,
            mirror::engine_cache_clear,
            mirror::engine_running,
            mirror::engine_start,
            mirror::engine_stop,
            mirror::mirror_config_get,
            mirror::mirror_config_save,
            mirror::tool_pointing,
            mirror::engine_takeover,
            mirror::engine_restore,
            mirror::mirror_add_upstream,
            mirror::mirror_remove_upstream,
            mirror::mirror_add_ecosystem,
            // 模块二 · 网络分流
            routing::routing_get,
            routing::routing_save,
            routing::routing_reapply,
            routing::routing_status,
            routing::routing_diagnose,
            routing::routing_backups,
            routing::routing_rollback,
            routing::routing_import_clash,
            routing::list_apps,
            routing::import_bookmarks,
            routing::clash_groups,
            routing::launch_clash,
            // 模块三 · 凭据与账号
            creds::cred_list,
            creds::cred_get,
            creds::cred_set,
            creds::cred_delete,
            creds::gh_accounts,
            creds::gh_switch,
            creds::import_password_book,
            creds::tool_logins,
            creds::run_in_terminal,
            creds::cred_detail,
            creds::chrome_profiles,
            creds::launch_app,
            // 模块三.6 · Cloudflare / 邮箱
            cloudflare::cf_verify,
            cloudflare::cf_zones,
            cloudflare::cf_email_rules,
            cloudflare::cf_email_destinations,
            cloudflare::cf_email_add,
            cloudflare::cf_email_delete,
            cloudflare::cf_open_console,
            // 模块五 · 节点核验
            verify::verify_profiles,
            verify::verify_run,
            verify::verify_progress,
            verify::verify_latest,
            verify::verify_current_exit,
            verify::verify_trace,
            verify::verify_sample_now,
            verify::verify_schedule_get,
            verify::verify_schedule_set,
            verify::verify_peak_stats,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
