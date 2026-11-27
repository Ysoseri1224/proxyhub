// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    if std::env::args().any(|a| a == "--verify-sample") {
        app_lib::headless_sample();
        return;
    }
    let args: Vec<String> = std::env::args().collect();
    if let Some(i) = args.iter().position(|a| a == "--verify-report") {
        app_lib::headless_report(args.get(i + 1).cloned());
        return;
    }
    app_lib::run()
}
