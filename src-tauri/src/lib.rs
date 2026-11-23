use std::collections::HashSet;
use std::io::Write;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use driver_core::{analyze, export, ScanResult, UpdateReport};
use system_tools::apps::InstalledApp;
use system_tools::cleaner::{CleanCategory, CleanResult};
use system_tools::disk::DiskReport;
use system_tools::duplicates::{DuplicateReport, RemovalResult};
use system_tools::software::SoftwareUpdate;
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
    /// Package IDs from the latest app-update check: the only ones that may be upgraded.
    software_ids: Mutex<HashSet<String>>,
    /// The latest duplicate scan: removals are checked against it.
    duplicates: Mutex<Option<DuplicateReport>>,
    duplicates_cancel: Arc<AtomicBool>,
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

/// First argument that makes the exe run as the elevated cleaning helper.
pub const CLEAN_HELPER_FLAG: &str = "--clean-system";

/// Helper side of system cleaning; `args` are the pipe name, then category
/// IDs. Reports one line of JSON and returns the process exit code.
pub fn run_clean_helper(args: &[String]) -> i32 {
    let Some((pipe_name, ids)) = args.split_first() else { return 2 };
    let Some(mut pipe) = driver_core::open_helper_pipe(pipe_name) else { return 3 };
    // Elevated rights are only for the categories that need them.
    let ids: Vec<String> =
        ids.iter().filter(|id| system_tools::cleaner::needs_admin(id)).cloned().collect();
    let result = system_tools::cleaner::clean(&ids);
    let Ok(mut line) = serde_json::to_vec(&result) else { return 1 };
    line.push(b'\n');
    i32::from(pipe.write_all(&line).is_err())
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct CleanOutcome {
    #[serde(flatten)]
    result: CleanResult,
    /// Set when the system-wide part could not be done.
    note: Option<String>,
}

#[tauri::command]
async fn cleaner_clean(ids: Vec<String>) -> Result<CleanOutcome, String> {
    blocking(move || {
        let (system, user): (Vec<String>, Vec<String>) =
            ids.into_iter().partition(|id| system_tools::cleaner::needs_admin(id));
        let mut result = system_tools::cleaner::clean(&user);
        let mut note = None;
        if !system.is_empty() {
            let mut reported = None;
            let run = driver_core::run_elevated(CLEAN_HELPER_FLAG, &system, &mut |line| {
                reported = serde_json::from_slice::<CleanResult>(line).ok();
                Ok(())
            });
            match (run, reported) {
                (Ok(()), Some(system_result)) => result.add(&system_result),
                (Err(e), _) => note = Some(format!("{e} System files were not cleaned.")),
                (Ok(()), None) => note = Some("System files could not be cleaned.".to_string()),
            }
        }
        Ok(CleanOutcome { result, note })
    })
    .await
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
    let from_disk_scan = state.disk_paths.lock().unwrap().contains(&path);
    let from_duplicates = state.duplicates.lock().unwrap().as_ref().is_some_and(|report| {
        report.groups.iter().any(|g| g.files.iter().any(|f| f.path == path))
    });
    if !from_disk_scan && !from_duplicates {
        return Err("Scan the folder again first.".into());
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

#[tauri::command]
async fn software_list(state: State<'_, AppState>) -> Result<Vec<SoftwareUpdate>, String> {
    let updates = blocking(system_tools::software::list).await?;
    *state.software_ids.lock().unwrap() = updates.iter().map(|u| u.id.clone()).collect();
    Ok(updates)
}

#[tauri::command]
async fn software_upgrade(state: State<'_, AppState>, id: String) -> Result<(), String> {
    if !state.software_ids.lock().unwrap().contains(&id) {
        return Err("Check for app updates again first.".into());
    }
    blocking(move || system_tools::software::upgrade(&id)).await
}

/// Looks for identical files in a folder; with no path, the user's profile.
#[tauri::command]
async fn duplicates_find(
    state: State<'_, AppState>,
    path: Option<String>,
) -> Result<DuplicateReport, String> {
    let root = path
        .map(PathBuf::from)
        .or_else(system_tools::disk::default_root)
        .ok_or("Choose a folder to search.")?;
    let cancel = state.duplicates_cancel.clone();
    cancel.store(false, Ordering::Relaxed);
    let report = blocking(move || {
        system_tools::duplicates::find(&root, system_tools::duplicates::DEFAULT_MIN_SIZE, &cancel)
    })
    .await?;
    *state.duplicates.lock().unwrap() = Some(report.clone());
    Ok(report)
}

#[tauri::command]
fn duplicates_cancel(state: State<'_, AppState>) {
    state.duplicates_cancel.store(true, Ordering::Relaxed);
}

/// Moves the chosen duplicates to the Recycle Bin.
#[tauri::command]
async fn duplicates_remove(
    state: State<'_, AppState>,
    paths: Vec<String>,
) -> Result<RemovalResult, String> {
    let report = state
        .duplicates
        .lock()
        .unwrap()
        .clone()
        .ok_or("Scan for duplicates again before removing anything.")?;
    blocking(move || system_tools::duplicates::remove(&report, &paths)).await
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
            reveal_in_explorer,
            software_list,
            software_upgrade,
            duplicates_find,
            duplicates_cancel,
            duplicates_remove
        ])
        .run(tauri::generate_context!())
        .expect("failed to start the application");
}
