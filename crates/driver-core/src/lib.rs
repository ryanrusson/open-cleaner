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
