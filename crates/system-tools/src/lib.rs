//! PC maintenance tools that sit alongside the driver updater.
//!
//! Each module keeps its decisions (what counts as junk, what is safe to
//! delete, how values are parsed) in plain Rust that tests on any OS, and
//! confines Windows API and registry access to small `cfg(windows)` parts.

pub mod apps;
pub mod cleaner;
pub mod disk;
pub mod startup;

#[cfg(not(windows))]
pub(crate) const WINDOWS_ONLY: &str = "This only works on Windows.";
