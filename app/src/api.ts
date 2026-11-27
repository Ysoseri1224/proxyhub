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
export type ToolPointing = { tool: string; current: string; on_engine: boolean };
export const toolPointing = () => invoke<ToolPointing[]>("tool_pointing");
export const engineTakeover = () => invoke<string>("engine_takeover");
export const engineRestore = () => invoke<string>("engine_restore");
export const mirrorAddUpstream = (eco: string, name: string, base: string) =>
  invoke<string>("mirror_add_upstream", { eco, name, base });
export const mirrorRemoveUpstream = (eco: string, name: string) =>
  invoke<string>("mirror_remove_upstream", { eco, name });
export const mirrorAddEcosystem = (name: string, prefix: string, upName: string, upBase: string) =>
  invoke<string>("mirror_add_ecosystem", { name, prefix, upName, upBase });

// 模块二 · 网络分流
export type Rule = { category: string; value: string; target: string };
export const routingGet = () => invoke<string>("routing_get");
export const routingSave = (json: string) => invoke<string>("routing_save", { json });
export const routingReapply = () => invoke<string>("routing_reapply");
export type RoutingStatus = {
  clash_running: boolean;
  merge_exists: boolean;
  merge_managed: boolean;
  subscription_present: boolean;
  model_rules: number;
  runtime_rules: number;
  runtime_groups: string[];
  notes: string[];
};
export const routingStatus = () => invoke<RoutingStatus>("routing_status");
export type DiagHit = { rule_type: string; value: string; target: string; certainty: string };
export type DiagResult = { host: string; hits: DiagHit[]; conclusion: string };
export const routingDiagnose = (host: string) => invoke<DiagResult>("routing_diagnose", { host });
export const routingBackups = () => invoke<string[]>("routing_backups");
export const routingRollback = (name: string) => invoke<string>("routing_rollback", { name });
export const routingImportClash = () => invoke<string>("routing_import_clash");
export type AppItem = { name: string; process: string; path: string };
export const listApps = () => invoke<AppItem[]>("list_apps");
export const importBookmarks = () => invoke<string[]>("import_bookmarks");
export const clashGroups = () => invoke<string[]>("clash_groups");
export const launchClash = () => invoke<string>("launch_clash");

// 模块三 · 凭据与账号
export type CredMeta = { name: string; category: string; note: string };
export const credList = () => invoke<CredMeta[]>("cred_list");
export const credGet = (name: string) => invoke<string>("cred_get", { name });
export const credSet = (name: string, value: string, category: string, note: string) =>
  invoke<void>("cred_set", { name, value, category, note });
export const credDelete = (name: string) => invoke<void>("cred_delete", { name });
export type GhAccount = { user: string; active: boolean };
export const ghAccounts = () => invoke<GhAccount[]>("gh_accounts");
export const ghSwitch = (user: string) => invoke<string>("gh_switch", { user });
export const importPasswordBook = () => invoke<string>("import_password_book");
export type ToolLogin = { tool: string; logged_in: boolean; detail: string; login_cmd: string };
export const toolLogins = () => invoke<ToolLogin[]>("tool_logins");
export const runInTerminal = (cmd: string) => invoke<string>("run_in_terminal", { cmd });
export type CredDetail = {
  name: string;
  value: string;
  type: string;
  account: string;
  scopes: string;
  expiration: string;
  status: string;
};
export const credDetail = (name: string) => invoke<string>("cred_detail", { name });
export type ChromeProfile = { name: string; path: string };
export const chromeProfiles = () => invoke<ChromeProfile[]>("chrome_profiles");
export const launchApp = (path: string) => invoke<string>("launch_app", { path });

// 模块三.6 · Cloudflare / 邮箱
export const cfVerify = () => invoke<string>("cf_verify");
export const cfZones = () => invoke<string>("cf_zones");
export const cfEmailRules = (zoneId: string) => invoke<string>("cf_email_rules", { zoneId });
export const cfEmailDestinations = (accountId: string) =>
  invoke<string>("cf_email_destinations", { accountId });
export const cfEmailAdd = (zoneId: string, alias: string, dest: string) =>
  invoke<string>("cf_email_add", { zoneId, alias, dest });
export const cfEmailDelete = (zoneId: string, ruleId: string) =>
  invoke<string>("cf_email_delete", { zoneId, ruleId });
export const cfOpenConsole = (section: string) => invoke<string>("cf_open_console", { section });

// 模块五 · 节点核验
export type VProfile = { uid: string; name: string; current: boolean; nodes: number };
export type IpIntel = {
  ip: string; cc: string; country: string; city: string; isp: string; org: string;
  asn: number; as_name: string; rdns: string; hosting: boolean; proxy: boolean; mobile: boolean;
  datacenter: boolean; dc_reason: string;
};
export type VCheck = { item: string; claim: string; observed: string; result: "pass" | "fail" | "unverified"; note: string };
export type AiAccess = { service: string; ok: boolean; basis: string };
export type Purity = { level: "clean" | "warn" | "risky" | "unknown"; label: string; reasons: string[] };
export type Verdict = "consistent" | "mismatch" | "unverified" | "unreachable";
export type NodeResult = {
  name: string; ptype: string; server: string; port: number;
  entry_ip: string; dns_source: string; entry: IpIntel | null; entry_tcp_ms: number[];
  reachable: boolean; error: string; exit: IpIntel | null; ai_loc: string;
  http_ms: number[]; http_fail: number; http_tries: number;
  claims: string[]; checks: VCheck[]; verdict: Verdict; missing: string[]; purity: Purity; ai: AiAccess[]; shared_exit_with: string[];
};
export type VReport = {
  at: number; profile: string; profile_name: string; local_isp: string; local_ip: string;
  skipped_info: string[]; notes: string[]; nodes: NodeResult[];
};
export type VProgress = { running: boolean; stage: string; done: number; total: number };
export type ExitView = { label: string; ip: IpIntel | null; ai_loc: string; purity: Purity | null; ai: AiAccess[]; note: string };
export type Hop = { n: number; ip: string; ms: number | null; cc: string; asn: number; org: string; private: boolean };
export type VSchedule = { installed: boolean; loaded: boolean; slots: string[]; binary: string; samples: number };
export type PeakStat = {
  node: string; off_n: number; off_ms: number | null; off_loss: number | null;
  peak_n: number; peak_ms: number | null; peak_loss: number | null; local_isp: string;
};
export const verifyProfiles = () => invoke<VProfile[]>("verify_profiles");
export const verifyRun = (profile: string) => invoke<VReport>("verify_run", { profile });
export const verifyProgress = () => invoke<VProgress>("verify_progress");
export const verifyLatest = () => invoke<VReport | null>("verify_latest");
export const verifyCurrentExit = () => invoke<ExitView[]>("verify_current_exit");
export const verifyTrace = (ip: string) => invoke<Hop[]>("verify_trace", { ip });
export const verifySampleNow = () => invoke<string>("verify_sample_now");
export const verifyScheduleGet = () => invoke<VSchedule>("verify_schedule_get");
export const verifyScheduleSet = (enable: boolean) => invoke<VSchedule>("verify_schedule_set", { enable });
export const verifyPeakStats = () => invoke<PeakStat[]>("verify_peak_stats");
