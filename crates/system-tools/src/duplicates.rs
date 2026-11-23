//! Duplicate file finder.
//!
//! Files are compared by size first, then by a hash of their first block,
//! then by a SHA-256 of the whole file, so only true byte-for-byte copies
//! are grouped. Removal goes to the Recycle Bin, never straight to deletion,
//! and at least one copy in every group is always kept.

use std::collections::{HashMap, HashSet};
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::UNIX_EPOCH;

use serde::Serialize;
use sha2::{Digest, Sha256};

use crate::disk::takes_no_space;

/// Smaller files are rarely worth the trouble and slow the scan a lot.
pub const DEFAULT_MIN_SIZE: u64 = 1024 * 1024;
const HEAD_BYTES: usize = 64 * 1024;

#[derive(Serialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct DuplicateFile {
    pub path: String,
    /// Seconds since the Unix epoch.
    pub modified: Option<u64>,
}

#[derive(Serialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct DuplicateGroup {
    pub size_bytes: u64,
    /// Identical files, oldest first.
    pub files: Vec<DuplicateFile>,
}

#[derive(Serialize, Clone, Debug, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct DuplicateReport {
    pub root: String,
    /// Biggest savings first.
    pub groups: Vec<DuplicateGroup>,
    /// Space freed by keeping one copy from each group.
    pub wasted_bytes: u64,
    pub files_checked: u64,
}

#[derive(Serialize, Clone, Debug, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct RemovalResult {
    pub removed: u64,
    pub freed_bytes: u64,
    pub failed: u64,
}

/// Hidden and system folders (AppData, `.git`, ...) hold copies that apps
/// rely on. Removing "duplicates" there breaks things, so they are skipped.
fn is_private_folder(path: &Path, meta: &fs::Metadata) -> bool {
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        const HIDDEN: u32 = 0x2;
        const SYSTEM: u32 = 0x4;
        if meta.file_attributes() & (HIDDEN | SYSTEM) != 0 {
            return true;
        }
    }
    let _ = meta;
    path.file_name().is_some_and(|n| {
        let name = n.to_string_lossy();
        name.starts_with('.') || name.starts_with('$') || name.eq_ignore_ascii_case("node_modules")
    })
}

const CANCELLED: &str = "Cancelled.";

fn collect(dir: &Path, min_size: u64, cancel: &AtomicBool, by_size: &mut HashMap<u64, Vec<PathBuf>>, checked: &mut u64) -> Result<(), String> {
    if cancel.load(Ordering::Relaxed) {
        return Err(CANCELLED.into());
    }
    let Ok(entries) = fs::read_dir(dir) else { return Ok(()) };
    for entry in entries.flatten() {
        let path = entry.path();
        let Ok(meta) = fs::symlink_metadata(&path) else { continue };
        if takes_no_space(&meta) {
            continue;
        }
        if meta.is_dir() {
            if !is_private_folder(&path, &meta) {
                collect(&path, min_size, cancel, by_size, checked)?;
            }
        } else if meta.len() >= min_size {
            *checked += 1;
            by_size.entry(meta.len()).or_default().push(path);
        }
    }
    Ok(())
}

/// SHA-256 of the first `limit` bytes, or of the whole file when `None`.
fn hash(path: &Path, limit: Option<usize>, cancel: &AtomicBool) -> Option<[u8; 32]> {
    let mut file = fs::File::open(path).ok()?;
    let mut hasher = Sha256::new();
    let mut buffer = vec![0u8; 1024 * 1024];
    let mut remaining = limit;
    loop {
        if cancel.load(Ordering::Relaxed) {
            return None;
        }
        let want = remaining.map_or(buffer.len(), |r| r.min(buffer.len()));
        if want == 0 {
            break;
        }
        let read = file.read(&mut buffer[..want]).ok()?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
        if let Some(r) = remaining.as_mut() {
            *r -= read;
        }
    }
    Some(hasher.finalize().into())
}

/// Split `paths` into sets that share a hash, dropping anything unique.
fn split(paths: Vec<PathBuf>, limit: Option<usize>, cancel: &AtomicBool) -> Vec<Vec<PathBuf>> {
    let mut by_hash: HashMap<[u8; 32], Vec<PathBuf>> = HashMap::new();
    for path in paths {
        if let Some(digest) = hash(&path, limit, cancel) {
            by_hash.entry(digest).or_default().push(path);
        }
    }
    by_hash.into_values().filter(|set| set.len() > 1).collect()
}

fn modified(path: &Path) -> Option<u64> {
    let time = fs::metadata(path).ok()?.modified().ok()?;
    Some(time.duration_since(UNIX_EPOCH).ok()?.as_secs())
}

/// Find identical files of at least `min_size` bytes under `root`. Setting
/// `cancel` stops the scan with an error.
pub fn find(root: &Path, min_size: u64, cancel: &AtomicBool) -> Result<DuplicateReport, String> {
    fs::read_dir(root).map_err(|e| format!("Could not read {}: {e}", root.display()))?;
    let mut by_size = HashMap::new();
    let mut files_checked = 0;
    // Never compare empty files: they are all "identical".
    collect(root, min_size.max(1), cancel, &mut by_size, &mut files_checked)?;

    let mut groups = Vec::new();
    for (size_bytes, paths) in by_size.into_iter().filter(|(_, paths)| paths.len() > 1) {
        for candidates in split(paths, Some(HEAD_BYTES), cancel) {
            // A file no bigger than the first block is already fully hashed.
            let sets = if size_bytes as usize <= HEAD_BYTES {
                vec![candidates]
            } else {
                split(candidates, None, cancel)
            };
            for set in sets {
                let mut files: Vec<DuplicateFile> = set
                    .iter()
                    .map(|p| DuplicateFile { path: p.to_string_lossy().to_string(), modified: modified(p) })
                    .collect();
                files.sort_by(|a, b| a.modified.cmp(&b.modified).then_with(|| a.path.cmp(&b.path)));
                groups.push(DuplicateGroup { size_bytes, files });
            }
        }
        if cancel.load(Ordering::Relaxed) {
            return Err(CANCELLED.into());
        }
    }

    let wasted = |g: &DuplicateGroup| g.size_bytes * (g.files.len() as u64 - 1);
    groups.sort_by(|a, b| wasted(b).cmp(&wasted(a)).then_with(|| a.files[0].path.cmp(&b.files[0].path)));
    Ok(DuplicateReport {
        root: root.to_string_lossy().to_string(),
        wasted_bytes: groups.iter().map(wasted).sum(),
        groups,
        files_checked,
    })
}

/// Check a removal request against the report it came from: every path must
/// be a known duplicate, and no group may lose all of its copies.
pub fn check_removal(report: &DuplicateReport, paths: &[String]) -> Result<(), String> {
    let chosen: HashSet<&str> = paths.iter().map(String::as_str).collect();
    let known: HashSet<&str> =
        report.groups.iter().flat_map(|g| g.files.iter().map(|f| f.path.as_str())).collect();
    if chosen.is_empty() || !chosen.is_subset(&known) {
        return Err("Scan for duplicates again before removing anything.".into());
    }
    if report.groups.iter().any(|g| g.files.iter().all(|f| chosen.contains(f.path.as_str()))) {
        return Err("At least one copy of each file must be kept.".into());
    }
    Ok(())
}

/// Move the chosen duplicates to the Recycle Bin.
pub fn remove(report: &DuplicateReport, paths: &[String]) -> Result<RemovalResult, String> {
    check_removal(report, paths)?;
    let size_of: HashMap<&str, u64> = report
        .groups
        .iter()
        .flat_map(|g| g.files.iter().map(move |f| (f.path.as_str(), g.size_bytes)))
        .collect();
    // Only files still exactly as scanned; anything changed since is left.
    let unchanged: Vec<&String> = paths
        .iter()
        .filter(|p| fs::metadata(p).is_ok_and(|m| m.is_file() && Some(&m.len()) == size_of.get(p.as_str())))
        .collect();
    recycle(&unchanged)?;

    let mut result = RemovalResult { failed: (paths.len() - unchanged.len()) as u64, ..Default::default() };
    for path in unchanged {
        if Path::new(path).exists() {
            result.failed += 1;
        } else {
            result.removed += 1;
            result.freed_bytes += size_of[path.as_str()];
        }
    }
    Ok(result)
}

#[cfg(windows)]
fn recycle(paths: &[&String]) -> Result<(), String> {
    use std::os::windows::ffi::OsStrExt;
    use windows::core::PCWSTR;
    use windows::Win32::UI::Shell::{
        SHFileOperationW, FOF_ALLOWUNDO, FOF_NOCONFIRMATION, FOF_NOERRORUI, FOF_SILENT, FO_DELETE,
        SHFILEOPSTRUCTW,
    };

    if paths.is_empty() {
        return Ok(());
    }
    // The API takes one buffer of NUL-separated paths ending in a double NUL.
    let mut from: Vec<u16> = Vec::new();
    for path in paths {
        from.extend(std::ffi::OsStr::new(path.as_str()).encode_wide());
        from.push(0);
    }
    from.push(0);
    let mut operation = SHFILEOPSTRUCTW {
        wFunc: FO_DELETE,
        pFrom: PCWSTR(from.as_ptr()),
        // ALLOWUNDO is what sends files to the Recycle Bin instead of deleting.
        fFlags: (FOF_ALLOWUNDO | FOF_NOCONFIRMATION | FOF_NOERRORUI | FOF_SILENT).0 as u16,
        ..Default::default()
    };
    // Per-file outcomes are checked by the caller, so the code is not needed.
    let _ = unsafe { SHFileOperationW(&mut operation) };
    Ok(())
}

#[cfg(not(windows))]
fn recycle(_paths: &[&String]) -> Result<(), String> {
    Err(crate::WINDOWS_ONLY.into())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sandbox(name: &str) -> PathBuf {
        let root = std::env::temp_dir().join(format!("system-tools-dupes-{}-{name}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        root
    }

    fn write(path: &Path, content: &[u8]) {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, content).unwrap();
    }

    fn names(group: &DuplicateGroup) -> Vec<String> {
        let mut names: Vec<String> = group
            .files
            .iter()
            .map(|f| Path::new(&f.path).file_name().unwrap().to_string_lossy().to_string())
            .collect();
        names.sort();
        names
    }

    #[test]
    fn groups_only_byte_identical_files() {
        let root = sandbox("identical");
        let big = vec![7u8; 200_000];
        let mut same_start = big.clone();
        *same_start.last_mut().unwrap() = 8;
        write(&root.join("a/photo.raw"), &big);
        write(&root.join("b/photo-copy.raw"), &big);
        write(&root.join("b/photo-edited.raw"), &same_start);
        write(&root.join("small-1.txt"), b"hello hello");
        write(&root.join("small-2.txt"), b"hello hello");
        write(&root.join("small-3.txt"), b"hello HELLO");
        write(&root.join("unique.bin"), &[1, 2, 3]);

        let report = find(&root, 1, &AtomicBool::new(false)).unwrap();
        assert_eq!(report.groups.len(), 2);
        assert_eq!(names(&report.groups[0]), ["photo-copy.raw", "photo.raw"]);
        assert_eq!(names(&report.groups[1]), ["small-1.txt", "small-2.txt"]);
        assert_eq!(report.wasted_bytes, 200_000 + 11);
        assert_eq!(report.files_checked, 7);
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn respects_minimum_size_and_private_folders() {
        let root = sandbox("filters");
        write(&root.join("one.bin"), &[5u8; 500]);
        write(&root.join("two.bin"), &[5u8; 500]);
        write(&root.join(".git/objects/x"), &[9u8; 5000]);
        write(&root.join("project/y"), &[9u8; 5000]);
        write(&root.join("project/node_modules/z"), &[9u8; 5000]);
        write(&root.join("empty-1"), b"");
        write(&root.join("empty-2"), b"");

        let report = find(&root, 1000, &AtomicBool::new(false)).unwrap();
        assert!(report.groups.is_empty());
        assert_eq!(find(&root, 0, &AtomicBool::new(false)).unwrap().groups.len(), 1);
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn can_be_cancelled() {
        let root = sandbox("cancel");
        write(&root.join("a"), &[1u8; 10]);
        assert_eq!(find(&root, 1, &AtomicBool::new(true)).unwrap_err(), CANCELLED);
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn removal_must_keep_a_copy_and_stay_within_the_report() {
        let file = |p: &str| DuplicateFile { path: p.into(), modified: None };
        let report = DuplicateReport {
            groups: vec![
                DuplicateGroup { size_bytes: 10, files: vec![file("a1"), file("a2"), file("a3")] },
                DuplicateGroup { size_bytes: 20, files: vec![file("b1"), file("b2")] },
            ],
            ..Default::default()
        };
        let paths = |list: &[&str]| list.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        assert!(check_removal(&report, &paths(&["a2", "a3", "b2"])).is_ok());
        assert!(check_removal(&report, &paths(&["b1", "b2"])).is_err());
        assert!(check_removal(&report, &paths(&["a1", "C:/Windows/notepad.exe"])).is_err());
        assert!(check_removal(&report, &[]).is_err());
    }
}
