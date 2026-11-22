use std::sync::Mutex;

use driver_core::{analyze, export, ScanResult, UpdateReport};
use tauri::{AppHandle, State};
use tauri_plugin_opener::OpenerExt;

/// The latest results, kept so exports reflect exactly what is on screen.
#[derive(Default)]
struct AppState {
    scan: Mutex<Option<ScanResult>>,
    report: Mutex<Option<UpdateReport>>,
}

/// Run slow, blocking Windows calls off the UI thread.
async fn blocking<T: Send + 'static>(
    f: impl FnOnce() -> Result<T, String> + Send + 'static,
) -> Result<T, String> {
    tauri::async_runtime::spawn_blocking(f).await.map_err(|e| e.to_string())?
}

#[tauri::command]
async fn scan_drivers(state: State<'_, AppState>) -> Result<ScanResult, String> {
    let scan = blocking(driver_core::scan_drivers).await?;
    *state.scan.lock().unwrap() = Some(scan.clone());
    *state.report.lock().unwrap() = None;
    Ok(scan)
}

#[tauri::command]
async fn check_updates(state: State<'_, AppState>) -> Result<UpdateReport, String> {
    let drivers = state
        .scan
        .lock()
        .unwrap()
        .as_ref()
        .map(|s| s.drivers.clone())
        .ok_or("Scan your drivers first.")?;
    let updates = blocking(driver_core::search_windows_update).await?;
    let report = analyze::match_updates(&drivers, updates);
    *state.report.lock().unwrap() = Some(report.clone());
    Ok(report)
}

#[tauri::command]
fn export_report(path: String, state: State<'_, AppState>) -> Result<(), String> {
    let scan = state.scan.lock().unwrap();
    let scan = scan.as_ref().ok_or("Nothing to export yet.")?;
    let report = state.report.lock().unwrap();
    let body = if path.to_ascii_lowercase().ends_with(".json") {
        export::to_json(scan, report.as_ref())
    } else {
        // BOM so Excel reads device names as UTF-8.
        format!("\u{feff}{}", export::to_csv(scan, report.as_ref()))
    };
    std::fs::write(&path, body).map_err(|e| format!("Could not save {path}: {e}"))
}

/// Opens a vendor page or the Windows Update settings page. Restricted to
/// those so the webview can never launch arbitrary programs or files.
#[tauri::command]
fn open_link(app: AppHandle, url: String) -> Result<(), String> {
    if !(url.starts_with("https://") || url.starts_with("ms-settings:windowsupdate")) {
        return Err("Refusing to open this link.".into());
    }
    app.opener().open_url(url, None::<&str>).map_err(|e| e.to_string())
}

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .manage(AppState::default())
        .invoke_handler(tauri::generate_handler![
            scan_drivers,
            check_updates,
            export_report,
            open_link
        ])
        .run(tauri::generate_context!())
        .expect("failed to start the application");
}
