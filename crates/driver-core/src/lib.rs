//! Driver inventory and update detection.
//!
//! Everything that talks to Windows lives in [`platform`]; the rest is plain
//! logic that builds and tests on any OS.

pub mod analyze;
pub mod date;
pub mod export;
pub mod models;
pub mod vendor;

#[cfg(windows)]
mod platform;

pub use models::*;

#[cfg(not(windows))]
const WINDOWS_ONLY: &str = "Driver scanning only works on Windows.";

/// Enumerate installed drivers and problem devices. Fast (a few seconds).
pub fn scan_drivers() -> Result<ScanResult, String> {
    #[cfg(windows)]
    {
        let (system, raw) = platform::inventory::scan()?;
        Ok(analyze::build_scan(system, raw, date::today()))
    }
    #[cfg(not(windows))]
    Err(WINDOWS_ONLY.into())
}

/// Ask Windows Update which driver updates apply to this machine. Slow: goes
/// online and typically takes 20-90 seconds.
pub fn search_windows_update() -> Result<Vec<AvailableUpdate>, String> {
    #[cfg(windows)]
    {
        platform::windows_update::search()
    }
    #[cfg(not(windows))]
    Err(WINDOWS_ONLY.into())
}

/// First argument that makes the app exe run as the elevated install helper
/// instead of opening a window.
pub const INSTALL_HELPER_FLAG: &str = "--install-updates";

/// Install the given Windows Update driver updates. Raises one UAC prompt,
/// then reports progress through `on_event` until everything is done.
pub fn install_updates(
    ids: &[String],
    on_event: &mut dyn FnMut(InstallEvent),
) -> Result<(), String> {
    #[cfg(windows)]
    {
        platform::elevate::install_elevated(ids, on_event)
    }
    #[cfg(not(windows))]
    {
        let _ = (ids, on_event);
        Err(WINDOWS_ONLY.into())
    }
}

/// Entry point for the elevated helper; `args` follow [`INSTALL_HELPER_FLAG`].
/// Returns the process exit code.
pub fn run_install_helper(args: &[String]) -> i32 {
    #[cfg(windows)]
    {
        platform::elevate::run_helper(args)
    }
    #[cfg(not(windows))]
    {
        let _ = args;
        1
    }
}
