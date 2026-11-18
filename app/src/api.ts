// 后端命令封装(Tauri invoke)。
import { invoke } from "@tauri-apps/api/core";

// 模块一 · 镜像源
export const engineStatus = () => invoke<string>("engine_status");
export const engineDiagnose = () => invoke<string>("engine_diagnose");
export const engineCacheClear = () => invoke<string>("engine_cache_clear");
export const engineRunning = () => invoke<boolean>("engine_running");
export const engineStart = () => invoke<string>("engine_start");
export const engineStop = () => invoke<string>("engine_stop");
export const mirrorConfigGet = () => invoke<string>("mirror_config_get");
export const mirrorConfigSave = (json: string) => invoke<void>("mirror_config_save", { json });

// 模块二 · 网络分流
export type Rule = { category: string; value: string; target: string };
export const routingGet = () => invoke<string>("routing_get");
export const routingSave = (json: string) => invoke<string>("routing_save", { json });
export const routingBackups = () => invoke<string[]>("routing_backups");
export const routingRollback = (name: string) => invoke<string>("routing_rollback", { name });
export const routingImportClash = () => invoke<string>("routing_import_clash");
export type AppItem = { name: string; path: string };
export const listApps = () => invoke<AppItem[]>("list_apps");
export const importBookmarks = () => invoke<string[]>("import_bookmarks");
export const launchClash = () => invoke<string>("launch_clash");
export const exitIpCheck = () => invoke<string>("exit_ip_check");

// 模块三 · 凭据与账号
export type CredMeta = { name: string; category: string; note: string };
export const credList = () => invoke<CredMeta[]>("cred_list");
export const credGet = (name: string) => invoke<string>("cred_get", { name });
export const credSet = (name: string, value: string, category: string, note: string) =>
  invoke<void>("cred_set", { name, value, category, note });
export const credDelete = (name: string) => invoke<void>("cred_delete", { name });
export const ghAccounts = () => invoke<string>("gh_accounts");
export const ghSwitch = (user: string) => invoke<string>("gh_switch", { user });

// 模块三.6 · Cloudflare / 邮箱
export const cfVerify = () => invoke<string>("cf_verify");
export const cfZones = () => invoke<string>("cf_zones");
export const cfEmailRules = (zoneId: string) => invoke<string>("cf_email_rules", { zoneId });
export const cfEmailDestinations = (accountId: string) =>
  invoke<string>("cf_email_destinations", { accountId });
export const cfEmailAdd = (zoneId: string, alias: string, dest: string) =>
  invoke<string>("cf_email_add", { zoneId, alias, dest });
export const cfOpenConsole = (section: string) => invoke<string>("cf_open_console", { section });
