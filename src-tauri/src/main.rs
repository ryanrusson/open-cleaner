// No console window behind the app in release builds.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    // Relaunched elevated by the app itself to install updates: do that
    // work and exit without ever opening a window.
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.first().map(String::as_str) == Some(driver_core::INSTALL_HELPER_FLAG) {
        std::process::exit(driver_core::run_install_helper(&args[1..]));
    }
    app_lib::run()
}
