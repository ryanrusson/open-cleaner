//! Installed programs, as listed in Windows' own uninstall registry, with a
//! way to start each program's own uninstaller.

use serde::Serialize;

#[derive(Serialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct InstalledApp {
    pub id: String,
    pub name: String,
    pub version: String,
    pub publisher: String,
    /// ISO date, when the installer recorded one.
    pub install_date: Option<String>,
    pub size_bytes: Option<u64>,
    pub can_uninstall: bool,
}

/// Installers record dates as `YYYYMMDD`.
pub fn parse_install_date(raw: &str) -> Option<String> {
    let raw = raw.trim();
    if raw.len() != 8 || !raw.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let (year, month, day) = (&raw[..4], &raw[4..6], &raw[6..]);
    let valid = (1..=12).contains(&month.parse::<u8>().ok()?)
        && (1..=31).contains(&day.parse::<u8>().ok()?)
        && year.parse::<u16>().ok()? >= 1990;
    valid.then(|| format!("{year}-{month}-{day}"))
}

/// MSI products are keyed by a braced GUID, which `msiexec` takes directly.
pub fn is_product_code(key: &str) -> bool {
    let bytes = key.as_bytes();
    bytes.len() == 38
        && bytes[0] == b'{'
        && bytes[37] == b'}'
        && bytes[1..37].iter().all(|b| b.is_ascii_hexdigit() || *b == b'-')
}

pub fn list() -> Result<Vec<InstalledApp>, String> {
    #[cfg(windows)]
    {
        Ok(windows_impl::list())
    }
    #[cfg(not(windows))]
    Err(crate::WINDOWS_ONLY.into())
}

/// Start the program's own uninstaller. It shows its own window (and its own
/// administrator prompt if it needs one); this returns as soon as it starts.
pub fn uninstall(id: &str) -> Result<(), String> {
    #[cfg(windows)]
    {
        windows_impl::uninstall(id)
    }
    #[cfg(not(windows))]
    {
        let _ = id;
        Err(crate::WINDOWS_ONLY.into())
    }
}

#[cfg(windows)]
mod windows_impl {
    use std::collections::HashSet;
    use std::os::windows::process::CommandExt;
    use std::process::Command;

    use winreg::enums::{HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE};
    use winreg::{RegKey, HKEY};

    use super::{is_product_code, parse_install_date, InstalledApp};

    const ROOTS: &[(&str, HKEY, &str)] = &[
        ("machine", HKEY_LOCAL_MACHINE, r"Software\Microsoft\Windows\CurrentVersion\Uninstall"),
        (
            "machine32",
            HKEY_LOCAL_MACHINE,
            r"Software\WOW6432Node\Microsoft\Windows\CurrentVersion\Uninstall",
        ),
        ("user", HKEY_CURRENT_USER, r"Software\Microsoft\Windows\CurrentVersion\Uninstall"),
    ];

    const CREATE_NO_WINDOW: u32 = 0x0800_0000;

    fn text(key: &RegKey, name: &str) -> String {
        key.get_value::<String, _>(name).map(|s| s.trim().to_string()).unwrap_or_default()
    }

    fn number(key: &RegKey, name: &str) -> Option<u32> {
        key.get_value::<u32, _>(name).ok()
    }

    pub fn list() -> Vec<InstalledApp> {
        let mut apps = Vec::new();
        let mut seen = HashSet::new();
        for (root_id, hive, path) in ROOTS {
            let Ok(root) = RegKey::predef(*hive).open_subkey(path) else { continue };
            for subkey in root.enum_keys().flatten() {
                let Ok(key) = root.open_subkey(&subkey) else { continue };
                let name = text(&key, "DisplayName");
                // Hidden components, patches and sub-features are not apps.
                if name.is_empty()
                    || number(&key, "SystemComponent") == Some(1)
                    || !text(&key, "ParentKeyName").is_empty()
                    || !text(&key, "ReleaseType").is_empty()
                {
                    continue;
                }
                let version = text(&key, "DisplayVersion");
                // The same app is often registered in both 32- and 64-bit views.
                if !seen.insert((name.to_lowercase(), version.clone())) {
                    continue;
                }
                let msi = number(&key, "WindowsInstaller") == Some(1) && is_product_code(&subkey);
                apps.push(InstalledApp {
                    id: format!("{root_id}|{subkey}"),
                    version,
                    publisher: text(&key, "Publisher"),
                    install_date: parse_install_date(&text(&key, "InstallDate")),
                    // Recorded in kilobytes.
                    size_bytes: number(&key, "EstimatedSize")
                        .filter(|kb| *kb > 0)
                        .map(|kb| u64::from(kb) * 1024),
                    can_uninstall: msi || !text(&key, "UninstallString").is_empty(),
                    name,
                });
            }
        }
        apps.sort_by_cached_key(|a| a.name.to_lowercase());
        apps
    }

    pub fn uninstall(id: &str) -> Result<(), String> {
        // The command always comes from the registry, never from the caller.
        let (root_id, subkey) = id.split_once('|').ok_or("Unknown app.")?;
        let (_, hive, path) = ROOTS.iter().find(|(r, _, _)| *r == root_id).ok_or("Unknown app.")?;
        let key = RegKey::predef(*hive)
            .open_subkey(format!(r"{path}\{subkey}"))
            .map_err(|_| "That app is no longer installed. Refresh the list.".to_string())?;

        let mut command;
        if number(&key, "WindowsInstaller") == Some(1) && is_product_code(subkey) {
            command = Command::new("msiexec.exe");
            command.args(["/x", subkey]);
        } else {
            let uninstaller = text(&key, "UninstallString");
            if uninstaller.is_empty() {
                return Err("This app does not provide an uninstaller.".into());
            }
            // Run it exactly as Windows' own Apps list would. Going through
            // cmd lets uninstallers that need elevation raise their prompt.
            command = Command::new("cmd.exe");
            command.raw_arg(format!("/S /C \"{uninstaller}\"")).creation_flags(CREATE_NO_WINDOW);
        }
        command.spawn().map(|_| ()).map_err(|e| format!("Could not start the uninstaller: {e}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_installer_dates() {
        assert_eq!(parse_install_date("20240315").as_deref(), Some("2024-03-15"));
        assert_eq!(parse_install_date(" 20240315 ").as_deref(), Some("2024-03-15"));
        assert_eq!(parse_install_date("20241345"), None);
        assert_eq!(parse_install_date("3/15/2024"), None);
        assert_eq!(parse_install_date(""), None);
    }

    #[test]
    fn recognises_msi_product_codes() {
        assert!(is_product_code("{90160000-008C-0000-1000-0000000FF1CE}"));
        assert!(!is_product_code("Steam"));
        assert!(!is_product_code("{90160000-008C-0000-1000-0000000FF1CE} /qn"));
    }
}
