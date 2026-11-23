//! Startup manager: what launches when you sign in, and a switch for each.
//!
//! Turning an item off uses the same mechanism as Task Manager's Startup
//! tab (a flag under `Explorer\StartupApproved`), so the entry itself is
//! never deleted and can always be turned back on, here or in Task Manager.

use serde::Serialize;

#[derive(Serialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct StartupItem {
    pub id: String,
    pub name: String,
    pub command: String,
    /// Where the entry lives, in plain words.
    pub source: String,
    pub enabled: bool,
    /// False for all-users entries when the app is not running as administrator.
    pub can_toggle: bool,
}

/// `StartupApproved` values are 12 bytes: a flag, then the time it was
/// disabled. An even flag (normally 2) means enabled, odd (normally 3)
/// disabled. No value at all means enabled.
pub fn approved_is_enabled(value: Option<&[u8]>) -> bool {
    value.and_then(|v| v.first()).is_none_or(|flag| flag & 1 == 0)
}

pub fn approved_value(enabled: bool, filetime: u64) -> [u8; 12] {
    let mut value = [0u8; 12];
    value[0] = if enabled { 2 } else { 3 };
    if !enabled {
        value[4..].copy_from_slice(&filetime.to_le_bytes());
    }
    value
}

pub fn list() -> Result<Vec<StartupItem>, String> {
    #[cfg(windows)]
    {
        Ok(windows_impl::list())
    }
    #[cfg(not(windows))]
    Err(crate::WINDOWS_ONLY.into())
}

pub fn set_enabled(id: &str, enabled: bool) -> Result<(), String> {
    #[cfg(windows)]
    {
        windows_impl::set_enabled(id, enabled)
    }
    #[cfg(not(windows))]
    {
        let _ = (id, enabled);
        Err(crate::WINDOWS_ONLY.into())
    }
}

#[cfg(windows)]
mod windows_impl {
    use std::path::PathBuf;

    use winreg::enums::{HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE, KEY_READ, KEY_WRITE, REG_BINARY};
    use winreg::{RegKey, RegValue, HKEY};

    use super::{approved_is_enabled, approved_value, StartupItem};

    const APPROVED: &str = r"Software\Microsoft\Windows\CurrentVersion\Explorer\StartupApproved";

    /// One place Windows looks for things to start at sign-in.
    struct Location {
        /// Stable prefix for item IDs.
        key: &'static str,
        hive: HKEY,
        source: &'static str,
        /// Registry path of the Run key, or `None` for a Startup folder.
        run_key: Option<&'static str>,
        /// Subkey of `StartupApproved` holding the on/off flags.
        approved: &'static str,
    }

    const LOCATIONS: &[Location] = &[
        Location {
            key: "user-run",
            hive: HKEY_CURRENT_USER,
            source: "Registry (your account)",
            run_key: Some(r"Software\Microsoft\Windows\CurrentVersion\Run"),
            approved: "Run",
        },
        Location {
            key: "machine-run",
            hive: HKEY_LOCAL_MACHINE,
            source: "Registry (all users)",
            run_key: Some(r"Software\Microsoft\Windows\CurrentVersion\Run"),
            approved: "Run",
        },
        Location {
            key: "machine-run32",
            hive: HKEY_LOCAL_MACHINE,
            source: "Registry (all users, 32-bit)",
            run_key: Some(r"Software\WOW6432Node\Microsoft\Windows\CurrentVersion\Run"),
            approved: "Run32",
        },
        Location {
            key: "user-folder",
            hive: HKEY_CURRENT_USER,
            source: "Startup folder (your account)",
            run_key: None,
            approved: "StartupFolder",
        },
        Location {
            key: "machine-folder",
            hive: HKEY_LOCAL_MACHINE,
            source: "Startup folder (all users)",
            run_key: None,
            approved: "StartupFolder",
        },
    ];

    fn startup_folder(location: &Location) -> Option<PathBuf> {
        let base = if location.hive == HKEY_CURRENT_USER { "APPDATA" } else { "PROGRAMDATA" };
        let base = std::env::var_os(base).filter(|v| !v.is_empty())?;
        Some(PathBuf::from(base).join(r"Microsoft\Windows\Start Menu\Programs\Startup"))
    }

    fn approved_path(location: &Location) -> String {
        format!(r"{APPROVED}\{}", location.approved)
    }

    /// (value name used for the on/off flag, display name, command)
    fn entries(location: &Location) -> Vec<(String, String, String)> {
        match location.run_key {
            Some(path) => {
                let Ok(key) = RegKey::predef(location.hive).open_subkey(path) else {
                    return Vec::new();
                };
                key.enum_values()
                    .flatten()
                    .filter(|(name, _)| !name.is_empty())
                    .filter_map(|(name, _)| {
                        let command: String = key.get_value(&name).ok()?;
                        Some((name.clone(), name, command))
                    })
                    .collect()
            }
            None => {
                let Some(folder) = startup_folder(location) else { return Vec::new() };
                let Ok(files) = std::fs::read_dir(&folder) else { return Vec::new() };
                files
                    .flatten()
                    .filter(|f| f.file_type().is_ok_and(|t| t.is_file()))
                    .filter_map(|f| {
                        let file_name = f.file_name().to_string_lossy().to_string();
                        if file_name.eq_ignore_ascii_case("desktop.ini") {
                            return None;
                        }
                        let display = f.path().file_stem()?.to_string_lossy().to_string();
                        Some((file_name, display, f.path().to_string_lossy().to_string()))
                    })
                    .collect()
            }
        }
    }

    pub fn list() -> Vec<StartupItem> {
        let mut items = Vec::new();
        for location in LOCATIONS {
            let hive = RegKey::predef(location.hive);
            let approved = hive.open_subkey(approved_path(location)).ok();
            // All-users flags live under HKLM, writable only when elevated.
            let can_toggle = location.hive == HKEY_CURRENT_USER
                || hive.open_subkey_with_flags(approved_path(location), KEY_READ | KEY_WRITE).is_ok();
            for (value_name, name, command) in entries(location) {
                let flag = approved.as_ref().and_then(|k| k.get_raw_value(&value_name).ok());
                items.push(StartupItem {
                    id: format!("{}|{value_name}", location.key),
                    name,
                    command,
                    source: location.source.to_string(),
                    enabled: approved_is_enabled(flag.as_ref().map(|v| &v.bytes[..])),
                    can_toggle,
                });
            }
        }
        items.sort_by_cached_key(|i| i.name.to_lowercase());
        items
    }

    fn now_filetime() -> u64 {
        // FILETIME counts 100 ns ticks from 1601; the Unix epoch is this far in.
        const EPOCH_GAP_SECS: u64 = 11_644_473_600;
        let unix = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        (unix + EPOCH_GAP_SECS) * 10_000_000
    }

    pub fn set_enabled(id: &str, enabled: bool) -> Result<(), String> {
        let (location_key, value_name) = id.split_once('|').ok_or("Unknown startup item.")?;
        let location = LOCATIONS
            .iter()
            .find(|l| l.key == location_key)
            .ok_or("Unknown startup item.")?;
        // Only flag entries that really exist; never create stray values.
        if !entries(location).iter().any(|(name, _, _)| name == value_name) {
            return Err("That startup item no longer exists. Refresh the list.".into());
        }
        let (key, _) = RegKey::predef(location.hive)
            .create_subkey(approved_path(location))
            .map_err(|e| match e.kind() {
                std::io::ErrorKind::PermissionDenied => {
                    "This item applies to all users. Change it from Task Manager, or run Driver \
                     Updater as administrator."
                        .to_string()
                }
                _ => format!("Could not change the startup setting: {e}"),
            })?;
        let value = RegValue {
            vtype: REG_BINARY,
            bytes: approved_value(enabled, now_filetime()).to_vec().into(),
        };
        key.set_raw_value(value_name, &value)
            .map_err(|e| format!("Could not change the startup setting: {e}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_task_manager_flags() {
        assert!(approved_is_enabled(None));
        assert!(approved_is_enabled(Some(&[])));
        assert!(approved_is_enabled(Some(&[2, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0])));
        assert!(approved_is_enabled(Some(&[6, 0, 0, 0])));
        assert!(!approved_is_enabled(Some(&[3, 0, 0, 0, 1, 2, 3, 4, 5, 6, 7, 8])));
        assert!(!approved_is_enabled(Some(&[7, 0, 0, 0])));
    }

    #[test]
    fn writes_flags_task_manager_understands() {
        assert_eq!(approved_value(true, 99), [2, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
        let disabled = approved_value(false, 0x0102_0304_0506_0708);
        assert_eq!(disabled, [3, 0, 0, 0, 8, 7, 6, 5, 4, 3, 2, 1]);
        assert!(!approved_is_enabled(Some(&disabled)));
    }
}
