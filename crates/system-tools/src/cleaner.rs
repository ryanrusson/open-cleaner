//! Junk file cleaner.
//!
//! Safety rules, because this module deletes things:
//! - The UI only ever sends category IDs. Every path is resolved here, from
//!   well-known per-user folders.
//! - A folder is only touched if it is absolute and several levels deep, so
//!   a missing environment variable can never aim this at a drive root.
//! - Only the *contents* of a target folder are removed, never the folder.
//! - Links and junctions are never followed or removed.
//! - Files that are in use simply fail to delete and are counted as skipped.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use serde::{Deserialize, Serialize};

#[derive(Serialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CleanCategory {
    pub id: String,
    pub label: String,
    pub description: String,
    pub size_bytes: u64,
    pub item_count: u64,
    /// Ticked by default in the UI.
    pub recommended: bool,
    /// Lives in a system folder; cleaning it raises an administrator prompt.
    pub needs_admin: bool,
    /// False when the folder cannot be read without administrator rights,
    /// so the size shown is only a lower bound.
    pub size_known: bool,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CleanResult {
    pub freed_bytes: u64,
    pub deleted: u64,
    /// In use, protected, or otherwise could not be removed.
    pub skipped: u64,
}

impl CleanResult {
    pub fn add(&mut self, other: &CleanResult) {
        self.freed_bytes += other.freed_bytes;
        self.deleted += other.deleted;
        self.skipped += other.skipped;
    }
}

/// The well-known folders everything is resolved from.
#[derive(Clone, Debug, Default)]
pub struct Folders {
    pub temp: Option<PathBuf>,
    pub local_app_data: Option<PathBuf>,
    /// `C:\Windows`
    pub system_root: Option<PathBuf>,
    /// `C:\ProgramData`
    pub program_data: Option<PathBuf>,
}

impl Folders {
    pub fn from_env() -> Self {
        let dir = |name: &str| std::env::var_os(name).filter(|v| !v.is_empty()).map(PathBuf::from);
        Self {
            temp: dir("TEMP"),
            local_app_data: dir("LOCALAPPDATA"),
            system_root: dir("SystemRoot"),
            program_data: dir("ProgramData"),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
enum Target {
    /// Everything inside `path`, optionally only files untouched for a while.
    Dir { path: PathBuf, min_age: Option<Duration> },
    /// Files directly inside `dir` whose names start with one of `prefixes`.
    Files { dir: PathBuf, prefixes: &'static [&'static str] },
    /// One specific file.
    File { path: PathBuf },
    RecycleBin,
}

struct Category {
    id: &'static str,
    label: &'static str,
    description: &'static str,
    recommended: bool,
    needs_admin: bool,
    targets: Vec<Target>,
}

const DAY: Duration = Duration::from_secs(24 * 60 * 60);

/// Chromium keeps one set of caches per profile folder.
fn chromium_caches(user_data: &Path) -> Vec<Target> {
    let Ok(entries) = fs::read_dir(user_data) else { return Vec::new() };
    entries
        .flatten()
        .filter(|e| {
            let name = e.file_name().to_string_lossy().to_string();
            name == "Default" || name.starts_with("Profile ")
        })
        .flat_map(|e| {
            ["Cache", "Code Cache", "GPUCache"]
                .map(|cache| Target::Dir { path: e.path().join(cache), min_age: None })
        })
        .collect()
}

fn firefox_caches(profiles: &Path) -> Vec<Target> {
    let Ok(entries) = fs::read_dir(profiles) else { return Vec::new() };
    entries
        .flatten()
        .map(|e| Target::Dir { path: e.path().join("cache2"), min_age: None })
        .collect()
}

fn categories(folders: &Folders) -> Vec<Category> {
    let mut list = Vec::new();

    if let Some(temp) = &folders.temp {
        list.push(Category {
            id: "temp",
            label: "Temporary files",
            description: "Leftovers from apps and installers, untouched for at least a day.",
            recommended: true,
            needs_admin: false,
            // Fresh temp files may belong to something still running.
            targets: vec![Target::Dir { path: temp.clone(), min_age: Some(DAY) }],
        });
    }

    if let Some(local) = &folders.local_app_data {
        let browser = |id, label, user_data: PathBuf| Category {
            id,
            label,
            description: "Cached web pages and images. Sign-ins, history and passwords are kept.",
            recommended: true,
            needs_admin: false,
            targets: chromium_caches(&user_data),
        };
        list.push(browser("chrome-cache", "Chrome cache", local.join("Google/Chrome/User Data")));
        list.push(browser("edge-cache", "Edge cache", local.join("Microsoft/Edge/User Data")));
        list.push(browser(
            "brave-cache",
            "Brave cache",
            local.join("BraveSoftware/Brave-Browser/User Data"),
        ));
        list.push(Category {
            id: "firefox-cache",
            label: "Firefox cache",
            description: "Cached web pages and images. Sign-ins, history and passwords are kept.",
            recommended: true,
            needs_admin: false,
            targets: firefox_caches(&local.join("Mozilla/Firefox/Profiles")),
        });
        list.push(Category {
            id: "crash-dumps",
            label: "Crash dumps and error reports",
            description: "Diagnostic files saved when apps crashed.",
            recommended: true,
            needs_admin: false,
            targets: vec![
                Target::Dir { path: local.join("CrashDumps"), min_age: None },
                Target::Dir { path: local.join("Microsoft/Windows/WER/ReportArchive"), min_age: None },
                Target::Dir { path: local.join("Microsoft/Windows/WER/ReportQueue"), min_age: None },
            ],
        });
        list.push(Category {
            id: "thumbnails",
            label: "Thumbnail cache",
            description: "Preview images for folders. Windows rebuilds them as needed.",
            recommended: false,
            needs_admin: false,
            targets: vec![Target::Files {
                dir: local.join("Microsoft/Windows/Explorer"),
                prefixes: &["thumbcache_", "iconcache_"],
            }],
        });
    }

    list.push(Category {
        id: "recycle-bin",
        label: "Recycle Bin",
        description: "Files you already deleted. Emptying it is permanent.",
        recommended: false,
        needs_admin: false,
        targets: vec![Target::RecycleBin],
    });

    if let Some(windows) = &folders.system_root {
        list.push(Category {
            id: "windows-temp",
            label: "Windows temporary files",
            description: "Leftovers from Windows and installers, untouched for at least a day.",
            recommended: false,
            needs_admin: true,
            targets: vec![Target::Dir { path: windows.join("Temp"), min_age: Some(DAY) }],
        });
        list.push(Category {
            id: "update-cache",
            label: "Windows Update downloads",
            description: "Already-installed update files. Windows downloads again anything it still needs.",
            recommended: false,
            needs_admin: true,
            targets: vec![Target::Dir {
                path: windows.join("SoftwareDistribution/Download"),
                min_age: Some(DAY),
            }],
        });
        let mut dumps = vec![
            Target::Dir { path: windows.join("Minidump"), min_age: None },
            Target::File { path: windows.join("MEMORY.DMP") },
        ];
        if let Some(program_data) = &folders.program_data {
            for queue in ["ReportArchive", "ReportQueue"] {
                dumps.push(Target::Dir {
                    path: program_data.join("Microsoft/Windows/WER").join(queue),
                    min_age: None,
                });
            }
        }
        list.push(Category {
            id: "system-dumps",
            label: "System crash dumps and error reports",
            description: "Memory dumps and reports saved when Windows itself crashed.",
            recommended: false,
            needs_admin: true,
            targets: dumps,
        });
    }

    // A browser that is not installed has nothing to offer.
    list.retain(|c| !c.targets.is_empty());
    list
}

/// Deep enough that it cannot be a drive root or a top-level folder.
fn is_safe_root(path: &Path) -> bool {
    path.is_absolute() && path.components().count() >= 4
}

fn is_link(meta: &fs::Metadata) -> bool {
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x400;
        if meta.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0 {
            return true;
        }
    }
    meta.file_type().is_symlink()
}

fn old_enough(meta: &fs::Metadata, min_age: Option<Duration>, now: SystemTime) -> bool {
    let Some(min_age) = min_age else { return true };
    meta.modified()
        .ok()
        .and_then(|modified| now.duration_since(modified).ok())
        .is_some_and(|age| age >= min_age)
}

#[derive(Default)]
struct Tally {
    bytes: u64,
    files: u64,
    skipped: u64,
    /// A folder could not even be listed (no permission).
    unreadable: bool,
}

/// Walk `dir`, counting eligible files; with `delete`, remove them too.
/// Returns whether `dir` ended up empty.
fn sweep(dir: &Path, min_age: Option<Duration>, now: SystemTime, delete: bool, tally: &mut Tally) -> bool {
    let entries = match fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(e) => {
            tally.unreadable |= e.kind() == std::io::ErrorKind::PermissionDenied;
            return false;
        }
    };
    let mut empty = true;
    for entry in entries.flatten() {
        let path = entry.path();
        let Ok(meta) = fs::symlink_metadata(&path) else {
            empty = false;
            continue;
        };
        if is_link(&meta) {
            empty = false;
        } else if meta.is_dir() {
            let emptied = sweep(&path, min_age, now, delete, tally);
            if !(delete && emptied && fs::remove_dir(&path).is_ok()) {
                empty = false;
            }
        } else if !old_enough(&meta, min_age, now) {
            empty = false;
        } else if !delete {
            tally.bytes += meta.len();
            tally.files += 1;
        } else if fs::remove_file(&path).is_ok() {
            tally.bytes += meta.len();
            tally.files += 1;
        } else {
            tally.skipped += 1;
            empty = false;
        }
    }
    empty
}

fn sweep_files(dir: &Path, prefixes: &[&str], delete: bool, tally: &mut Tally) {
    let Ok(entries) = fs::read_dir(dir) else { return };
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().to_ascii_lowercase();
        if !prefixes.iter().any(|p| name.starts_with(p)) {
            continue;
        }
        let Ok(meta) = fs::symlink_metadata(entry.path()) else { continue };
        if is_link(&meta) || !meta.is_file() {
            continue;
        }
        if !delete || fs::remove_file(entry.path()).is_ok() {
            tally.bytes += meta.len();
            tally.files += 1;
        } else {
            tally.skipped += 1;
        }
    }
}

fn process(target: &Target, now: SystemTime, delete: bool, tally: &mut Tally) {
    match target {
        Target::Dir { path, min_age } if is_safe_root(path) => {
            sweep(path, *min_age, now, delete, tally);
        }
        Target::Files { dir, prefixes } if is_safe_root(dir) => {
            sweep_files(dir, prefixes, delete, tally);
        }
        Target::File { path } if is_safe_root(path) => {
            let Ok(meta) = fs::symlink_metadata(path) else { return };
            if is_link(&meta) || !meta.is_file() {
                return;
            }
            if !delete || fs::remove_file(path).is_ok() {
                tally.bytes += meta.len();
                tally.files += 1;
            } else {
                tally.skipped += 1;
            }
        }
        Target::RecycleBin => {
            let (bytes, items) = recycle_bin::query();
            if !delete || items == 0 || recycle_bin::empty() {
                tally.bytes += bytes;
                tally.files += items;
            } else {
                tally.skipped += items;
            }
        }
        _ => {}
    }
}

pub fn scan_in(folders: &Folders) -> Vec<CleanCategory> {
    let now = SystemTime::now();
    categories(folders)
        .into_iter()
        .map(|c| {
            let mut tally = Tally::default();
            for target in &c.targets {
                process(target, now, false, &mut tally);
            }
            CleanCategory {
                id: c.id.to_string(),
                label: c.label.to_string(),
                description: c.description.to_string(),
                size_bytes: tally.bytes,
                item_count: tally.files,
                recommended: c.recommended,
                needs_admin: c.needs_admin,
                size_known: !tally.unreadable,
            }
        })
        .collect()
}

pub fn clean_in(folders: &Folders, ids: &[String]) -> CleanResult {
    let now = SystemTime::now();
    let mut tally = Tally::default();
    for category in categories(folders).iter().filter(|c| ids.iter().any(|id| id == c.id)) {
        for target in &category.targets {
            process(target, now, true, &mut tally);
        }
    }
    CleanResult { freed_bytes: tally.bytes, deleted: tally.files, skipped: tally.skipped }
}

/// Whether cleaning this category must be done by an elevated process.
pub fn needs_admin(id: &str) -> bool {
    categories(&Folders::from_env()).iter().any(|c| c.id == id && c.needs_admin)
}

/// Measure what each category would free. Deletes nothing.
pub fn scan() -> Vec<CleanCategory> {
    scan_in(&Folders::from_env())
}

/// Delete the junk in the chosen categories.
pub fn clean(ids: &[String]) -> CleanResult {
    clean_in(&Folders::from_env(), ids)
}

#[cfg(windows)]
mod recycle_bin {
    use windows::Win32::UI::Shell::{
        SHEmptyRecycleBinW, SHQueryRecycleBinW, SHERB_NOCONFIRMATION, SHERB_NOPROGRESSUI,
        SHERB_NOSOUND, SHQUERYRBINFO,
    };

    /// (bytes, items) across all drives.
    pub fn query() -> (u64, u64) {
        let mut info = SHQUERYRBINFO {
            cbSize: std::mem::size_of::<SHQUERYRBINFO>() as u32,
            ..Default::default()
        };
        match unsafe { SHQueryRecycleBinW(None, &mut info) } {
            Ok(()) => (info.i64Size.max(0) as u64, info.i64NumItems.max(0) as u64),
            Err(_) => (0, 0),
        }
    }

    pub fn empty() -> bool {
        unsafe {
            SHEmptyRecycleBinW(None, None, SHERB_NOCONFIRMATION | SHERB_NOPROGRESSUI | SHERB_NOSOUND)
        }
        .is_ok()
    }
}

#[cfg(not(windows))]
mod recycle_bin {
    pub fn query() -> (u64, u64) {
        (0, 0)
    }
    pub fn empty() -> bool {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A fresh scratch area laid out like a user profile.
    fn sandbox(name: &str) -> (PathBuf, Folders) {
        let root = std::env::temp_dir()
            .join(format!("system-tools-test-{}-{name}", std::process::id()))
            .join("Users/someone/AppData");
        let _ = fs::remove_dir_all(&root);
        let folders = Folders {
            temp: Some(root.join("Local/Temp")),
            local_app_data: Some(root.join("Local")),
            system_root: Some(root.join("Windows")),
            program_data: Some(root.join("ProgramData")),
        };
        fs::create_dir_all(folders.temp.as_ref().unwrap()).unwrap();
        (root, folders)
    }

    fn write(path: &Path, bytes: usize, age: Duration) {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, vec![0u8; bytes]).unwrap();
        let file = fs::File::options().write(true).open(path).unwrap();
        file.set_modified(SystemTime::now() - age).unwrap();
    }

    fn category<'a>(list: &'a [CleanCategory], id: &str) -> &'a CleanCategory {
        list.iter().find(|c| c.id == id).unwrap()
    }

    #[test]
    fn temp_cleaning_spares_recent_files_and_the_folder_itself() {
        let (_root, folders) = sandbox("temp");
        let temp = folders.temp.clone().unwrap();
        write(&temp.join("old.tmp"), 100, DAY * 3);
        write(&temp.join("nested/deep/old.log"), 50, DAY * 3);
        write(&temp.join("in-use.tmp"), 999, Duration::ZERO);

        let found = scan_in(&folders);
        assert_eq!(category(&found, "temp").size_bytes, 150);
        assert_eq!(category(&found, "temp").item_count, 2);

        let result = clean_in(&folders, &["temp".to_string()]);
        assert_eq!(result, CleanResult { freed_bytes: 150, deleted: 2, skipped: 0 });
        assert!(temp.join("in-use.tmp").exists());
        assert!(!temp.join("nested").exists());
        assert!(temp.exists());
    }

    #[test]
    fn browser_cache_is_cleared_but_profile_data_is_not() {
        let (_root, folders) = sandbox("browser");
        let profile = folders.local_app_data.clone().unwrap().join("Google/Chrome/User Data/Default");
        write(&profile.join("Cache/Cache_Data/f_0001"), 400, Duration::ZERO);
        write(&profile.join("Code Cache/js/abc"), 100, Duration::ZERO);
        write(&profile.join("Login Data"), 10, Duration::ZERO);
        write(&profile.join("History"), 10, Duration::ZERO);

        assert_eq!(category(&scan_in(&folders), "chrome-cache").size_bytes, 500);
        // Browsers that are not installed are not listed at all.
        assert!(scan_in(&folders).iter().all(|c| c.id != "edge-cache"));

        clean_in(&folders, &["chrome-cache".to_string()]);
        assert!(!profile.join("Cache/Cache_Data/f_0001").exists());
        assert!(profile.join("Login Data").exists());
        assert!(profile.join("History").exists());
    }

    #[test]
    fn only_selected_categories_are_cleaned() {
        let (_root, folders) = sandbox("selection");
        let temp = folders.temp.clone().unwrap();
        let dumps = folders.local_app_data.clone().unwrap().join("CrashDumps");
        write(&temp.join("old.tmp"), 10, DAY * 2);
        write(&dumps.join("app.dmp"), 10, Duration::ZERO);

        clean_in(&folders, &["crash-dumps".to_string(), "no-such-category".to_string()]);
        assert!(temp.join("old.tmp").exists());
        assert!(!dumps.join("app.dmp").exists());
    }

    #[test]
    fn thumbnail_cleaning_only_takes_matching_files() {
        let (_root, folders) = sandbox("thumbs");
        let explorer = folders.local_app_data.clone().unwrap().join("Microsoft/Windows/Explorer");
        write(&explorer.join("thumbcache_256.db"), 30, Duration::ZERO);
        write(&explorer.join("something-else.db"), 30, Duration::ZERO);

        let result = clean_in(&folders, &["thumbnails".to_string()]);
        assert_eq!(result.deleted, 1);
        assert!(explorer.join("something-else.db").exists());
    }

    #[test]
    fn system_categories_are_marked_and_cleaned_within_their_folders() {
        let (root, folders) = sandbox("system");
        let windows = root.join("Windows");
        write(&windows.join("Temp/setup.log"), 40, DAY * 2);
        write(&windows.join("Temp/fresh.log"), 40, Duration::ZERO);
        write(&windows.join("Minidump/crash.dmp"), 70, Duration::ZERO);
        write(&windows.join("MEMORY.DMP"), 900, Duration::ZERO);
        write(&windows.join("System32/kernel.dll"), 5, DAY * 400);
        write(&windows.join("notepad.exe"), 5, DAY * 400);

        let found = scan_in(&folders);
        assert!(category(&found, "windows-temp").needs_admin);
        assert!(!category(&found, "temp").needs_admin);
        assert_eq!(category(&found, "windows-temp").size_bytes, 40);
        assert_eq!(category(&found, "system-dumps").size_bytes, 970);
        assert!(category(&found, "system-dumps").size_known);

        clean_in(&folders, &["windows-temp".to_string(), "system-dumps".to_string()]);
        assert!(!windows.join("Temp/setup.log").exists());
        assert!(windows.join("Temp/fresh.log").exists());
        assert!(!windows.join("MEMORY.DMP").exists());
        assert!(!windows.join("Minidump/crash.dmp").exists());
        assert!(windows.join("System32/kernel.dll").exists());
        assert!(windows.join("notepad.exe").exists());
    }

    #[cfg(unix)]
    #[test]
    fn links_are_never_followed_or_removed() {
        let (root, folders) = sandbox("links");
        let precious = root.join("Documents/precious.txt");
        write(&precious, 5, DAY * 30);
        let temp = folders.temp.clone().unwrap();
        std::os::unix::fs::symlink(root.join("Documents"), temp.join("shortcut")).unwrap();

        clean_in(&folders, &["temp".to_string()]);
        assert!(precious.exists());
        assert!(temp.join("shortcut").exists());
    }

    #[test]
    fn shallow_or_missing_folders_are_refused() {
        let (drive, top, deep) = if cfg!(windows) {
            (r"C:\", r"C:\Users", r"C:\Users\someone\AppData\Local\Temp")
        } else {
            ("/", "/tmp", "/Users/someone/AppData/Local/Temp")
        };
        assert!(!is_safe_root(Path::new(drive)));
        assert!(!is_safe_root(Path::new(top)));
        assert!(!is_safe_root(Path::new("relative/path/that/is/deep")));
        assert!(is_safe_root(Path::new(deep)));

        // No environment at all: nothing but the Recycle Bin is offered.
        let found = scan_in(&Folders::default());
        assert!(found.iter().all(|c| c.id == "recycle-bin"));
    }
}
