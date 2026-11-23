// No console window behind the app in release builds.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    // Relaunched elevated by the app itself for work that needs
    // administrator rights: do that work and exit without opening a window.
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some(driver_core::INSTALL_HELPER_FLAG) => {
            std::process::exit(driver_core::run_install_helper(&args[1..]))
        }
        Some(app_lib::CLEAN_HELPER_FLAG) => std::process::exit(app_lib::run_clean_helper(&args[1..])),
        _ => app_lib::run(),
    }
}
