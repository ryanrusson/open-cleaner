//! App updates through winget, the package manager built into Windows.
//!
//! winget has no machine-readable output for upgrades, so its table is
//! parsed. Rows are read from the right-hand side, where every column is a
//! single word, which keeps this independent of column widths and of the
//! display language of the header.

use serde::Serialize;

#[derive(Serialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SoftwareUpdate {
    pub id: String,
    pub name: String,
    pub current_version: String,
    pub available_version: String,
    pub source: String,
}

/// Package IDs are passed to winget on a command line, so only the
/// characters real IDs use are accepted.
pub fn is_package_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 128
        && id.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_' | '+'))
}

/// Progress spinners are drawn by rewriting the line; keep only what a
/// terminal would finally show.
fn visible(line: &str) -> &str {
    line.rsplit('\r').next().unwrap_or(line).trim_end()
}

fn is_rule(line: &str) -> bool {
    line.len() >= 10 && line.chars().all(|c| c == '-')
}

fn parse_row(line: &str) -> Option<SoftwareUpdate> {
    let mut words: Vec<&str> = line.split_whitespace().collect();
    let source = words.pop()?.to_string();
    let available_version = words.pop()?.to_string();
    let mut current_version = words.pop()?.to_string();
    // "< 1.2.3" is how winget shows a version it can only bound.
    if matches!(words.last(), Some(&"<") | Some(&">")) {
        current_version = format!("{} {current_version}", words.pop()?);
    }
    let id = words.pop()?.to_string();
    if words.is_empty() {
        return None;
    }
    Some(SoftwareUpdate { id, name: words.join(" "), current_version, available_version, source })
}

/// Parse the output of `winget upgrade`.
pub fn parse_upgrades(output: &str) -> Vec<SoftwareUpdate> {
    let lines: Vec<&str> = output.lines().map(visible).collect();
    let Some(rule) = lines.iter().position(|l| is_rule(l)) else { return Vec::new() };
    // Name, Id, Version, Available, Source. Fewer columns means a different
    // table (for example, one with no Source), which this does not handle.
    if rule == 0 || lines[rule - 1].split_whitespace().count() != 5 {
        return Vec::new();
    }
    lines[rule + 1..]
        .iter()
        .take_while(|l| !l.trim().is_empty())
        .filter_map(|l| parse_row(l))
        // A truncated ID cannot be used to upgrade.
        .filter(|u| is_package_id(&u.id))
        .collect()
}

pub fn list() -> Result<Vec<SoftwareUpdate>, String> {
    #[cfg(windows)]
    {
        windows_impl::list()
    }
    #[cfg(not(windows))]
    Err(crate::WINDOWS_ONLY.into())
}

/// Update one app. Blocks until its installer finishes, which can take
/// minutes; the installer may raise its own administrator prompt.
pub fn upgrade(id: &str) -> Result<(), String> {
    if !is_package_id(id) {
        return Err("Unknown app.".into());
    }
    #[cfg(windows)]
    {
        windows_impl::upgrade(id)
    }
    #[cfg(not(windows))]
    Err(crate::WINDOWS_ONLY.into())
}

#[cfg(windows)]
mod windows_impl {
    use std::os::windows::process::CommandExt;
    use std::process::{Command, Output};

    use super::{parse_upgrades, SoftwareUpdate};

    const CREATE_NO_WINDOW: u32 = 0x0800_0000;

    fn winget(args: &[&str]) -> Result<Output, String> {
        Command::new("winget.exe")
            .args(args)
            .args(["--accept-source-agreements", "--disable-interactivity"])
            .creation_flags(CREATE_NO_WINDOW)
            .output()
            .map_err(|e| match e.kind() {
                std::io::ErrorKind::NotFound => "winget is not installed on this PC. Install App \
                                                 Installer from the Microsoft Store to enable app updates."
                    .to_string(),
                _ => format!("Could not run winget: {e}"),
            })
    }

    pub fn list() -> Result<Vec<SoftwareUpdate>, String> {
        let output = winget(&["upgrade"])?;
        let text = String::from_utf8_lossy(&output.stdout);
        let updates = parse_upgrades(&text);
        // No table and a failure code means winget itself had a problem.
        if updates.is_empty() && !output.status.success() && !text.lines().any(|l| l.contains("---")) {
            let last = text.lines().map(str::trim).filter(|l| !l.is_empty()).last().unwrap_or("");
            // 0x8A15002B: "no installed package has an update", not an error.
            if output.status.code() != Some(0x8A15_002Bu32 as i32) && !last.is_empty() {
                return Err(format!("winget could not check for updates: {last}"));
            }
        }
        Ok(updates)
    }

    pub fn upgrade(id: &str) -> Result<(), String> {
        let output = winget(&["upgrade", "--id", id, "--exact", "--silent", "--accept-package-agreements"])?;
        if output.status.success() {
            return Ok(());
        }
        let text = String::from_utf8_lossy(&output.stdout);
        let last = text
            .lines()
            .map(|l| l.rsplit('\r').next().unwrap_or(l).trim())
            .filter(|l| !l.is_empty())
            .last()
            .unwrap_or("The installer did not finish.");
        Err(last.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = "\r   - \r   \\ \r                                                                \r\
Name                              Id                         Version       Available     Source\r\n\
-----------------------------------------------------------------------------------------------\r\n\
Git                               Git.Git                    2.45.1        2.47.0        winget\r\n\
Microsoft Visual Studio Code      Microsoft.VisualStudioCode 1.94.2        1.95.3        winget\r\n\
Mozilla Firefox (x64 en-US)       Mozilla.Firefox            < 131.0       132.0.2       winget\r\n\
A Very Long Application Name Tha… Some.Truncated.Identifi…   1.0           2.0           winget\r\n\
4 upgrades available.\r\n\
\r\n\
The following packages have an upgrade available, but require explicit targeting for upgrade:\r\n\
Name    Id              Version Available Source\r\n\
------------------------------------------------\r\n\
Discord Discord.Discord 1.0.9   1.0.10    winget\r\n";

    #[test]
    fn parses_the_upgrade_table() {
        let updates = parse_upgrades(SAMPLE);
        let ids: Vec<&str> = updates.iter().map(|u| u.id.as_str()).collect();
        // The truncated ID and the summary line are dropped; the second
        // table (apps pinned to manual upgrades) is left alone.
        assert_eq!(ids, ["Git.Git", "Microsoft.VisualStudioCode", "Mozilla.Firefox"]);
        assert_eq!(updates[1].name, "Microsoft Visual Studio Code");
        assert_eq!(updates[1].current_version, "1.94.2");
        assert_eq!(updates[1].available_version, "1.95.3");
        assert_eq!(updates[2].name, "Mozilla Firefox (x64 en-US)");
        assert_eq!(updates[2].current_version, "< 131.0");
    }

    #[test]
    fn header_language_does_not_matter() {
        let german = "Name    ID      Version Verfügbar Quelle\n\
                      ----------------------------------------\n\
                      7-Zip   7zip.7zip 23.01 24.08     winget\n\
                      1 Aktualisierungen verfügbar.\n";
        assert_eq!(parse_upgrades(german)[0].id, "7zip.7zip");
    }

    #[test]
    fn nothing_to_update_is_an_empty_list() {
        assert!(parse_upgrades("No installed package found matching input criteria.\n").is_empty());
        assert!(parse_upgrades("").is_empty());
        // A table without a Source column is not misread.
        assert!(parse_upgrades("Name Id Version Available\n--------------------\nA B.C 1 2\n").is_empty());
    }

    #[test]
    fn only_real_package_ids_reach_the_command_line() {
        assert!(is_package_id("Microsoft.VisualStudioCode"));
        assert!(is_package_id("Notepad++.Notepad++"));
        assert!(!is_package_id("Git.Git & calc"));
        assert!(!is_package_id("Some.Truncated.Identifi…"));
        assert!(!is_package_id(""));
    }
}
