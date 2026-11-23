use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::Mutex;

use driver_core::{analyze, export, ScanResult, UpdateReport};
use system_tools::apps::InstalledApp;
use system_tools::cleaner::{CleanCategory, CleanResult};
use system_tools::disk::DiskReport;
use system_tools::startup::StartupItem;
use tauri::{AppHandle, Emitter, State};
use tauri_plugin_opener::OpenerExt;

/// The latest results, kept so exports reflect exactly what is on screen.
#[derive(Default)]
struct AppState {
    scan: Mutex<Option<ScanResult>>,
    report: Mutex<Option<UpdateReport>>,
    /// Paths from the latest disk scan: the only ones Explorer may be asked to show.
    disk_paths: Mutex<HashSet<String>>,
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

/// Installs driver updates from the latest check. Progress arrives in the UI
/// as `install-progress` events; this returns once everything is done.
#[tauri::command]
async fn install_updates(
    app: AppHandle,
    state: State<'_, AppState>,
    update_ids: Vec<String>,
) -> Result<(), String> {
    // Only updates this app found and matched to a device may be installed.
    let offered = state
        .report
        .lock()
        .unwrap()
        .as_ref()
        .is_some_and(|r| update_ids.iter().all(|id| r.matched.iter().any(|m| &m.update.update_id == id)));
    if !offered {
        return Err("Check for updates again before installing.".into());
    }
    blocking(move || {
        driver_core::install_updates(&update_ids, &mut |event| {
            let _ = app.emit("install-progress", &event);
        })
    })
    .await
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

#[tauri::command]
async fn cleaner_scan() -> Result<Vec<CleanCategory>, String> {
    blocking(|| Ok(system_tools::cleaner::scan())).await
}

#[tauri::command]
async fn cleaner_clean(ids: Vec<String>) -> Result<CleanResult, String> {
    blocking(move || Ok(system_tools::cleaner::clean(&ids))).await
}

#[tauri::command]
async fn startup_list() -> Result<Vec<StartupItem>, String> {
    blocking(system_tools::startup::list).await
}

#[tauri::command]
async fn startup_set_enabled(id: String, enabled: bool) -> Result<(), String> {
    blocking(move || system_tools::startup::set_enabled(&id, enabled)).await
}

#[tauri::command]
async fn apps_list() -> Result<Vec<InstalledApp>, String> {
    blocking(system_tools::apps::list).await
}

#[tauri::command]
async fn apps_uninstall(id: String) -> Result<(), String> {
    blocking(move || system_tools::apps::uninstall(&id)).await
}

/// Measures a folder; with no path, the user's own profile folder.
#[tauri::command]
async fn disk_scan(state: State<'_, AppState>, path: Option<String>) -> Result<DiskReport, String> {
    let root = path
        .map(PathBuf::from)
        .or_else(system_tools::disk::default_root)
        .ok_or("Choose a folder to analyse.")?;
    let report = blocking(move || system_tools::disk::scan(&root)).await?;
    *state.disk_paths.lock().unwrap() = report
        .folders
        .iter()
        .map(|f| f.path.clone())
        .chain(report.largest_files.iter().map(|f| f.path.clone()))
        .collect();
    Ok(report)
}

/// Opens Explorer with the given file or folder selected.
#[tauri::command]
fn reveal_in_explorer(state: State<'_, AppState>, path: String) -> Result<(), String> {
    if !state.disk_paths.lock().unwrap().contains(&path) {
        return Err("Analyse the folder again first.".into());
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        std::process::Command::new("explorer.exe")
            .raw_arg(format!("/select,\"{path}\""))
            .spawn()
            .map(|_| ())
            .map_err(|e| format!("Could not open Explorer: {e}"))
    }
    #[cfg(not(windows))]
    Err("This only works on Windows.".into())
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
            install_updates,
            open_link,
            cleaner_scan,
            cleaner_clean,
            startup_list,
            startup_set_enabled,
            apps_list,
            apps_uninstall,
            disk_scan,
            reveal_in_explorer
        ])
        .run(tauri::generate_context!())
        .expect("failed to start the application");
}
