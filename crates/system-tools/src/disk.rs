//! Disk usage: which folders and files take the most space. Read-only.

use std::cmp::Reverse;
use std::collections::BinaryHeap;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use serde::Serialize;

const LARGEST_FILES: usize = 100;

#[derive(Serialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct FolderSize {
    pub name: String,
    pub path: String,
    pub size_bytes: u64,
    pub file_count: u64,
}

#[derive(Serialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct FileEntry {
    pub path: String,
    pub size_bytes: u64,
    /// Seconds since the Unix epoch.
    pub modified: Option<u64>,
}

#[derive(Serialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct DiskReport {
    pub root: String,
    pub total_bytes: u64,
    pub file_count: u64,
    /// Direct children of the root, biggest first. Loose files in the root
    /// are grouped as one entry.
    pub folders: Vec<FolderSize>,
    pub largest_files: Vec<FileEntry>,
    /// Folders that could not be read (no permission).
    pub unreadable: u64,
}

/// Links, junctions, and cloud files that are not actually on this disk.
pub(crate) fn takes_no_space(meta: &fs::Metadata) -> bool {
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        const REPARSE_POINT: u32 = 0x400;
        const OFFLINE: u32 = 0x1000;
        const RECALL_ON_OPEN: u32 = 0x4_0000;
        const RECALL_ON_DATA_ACCESS: u32 = 0x40_0000;
        if meta.file_attributes() & (REPARSE_POINT | OFFLINE | RECALL_ON_OPEN | RECALL_ON_DATA_ACCESS) != 0 {
            return true;
        }
    }
    meta.file_type().is_symlink()
}

struct Walk {
    largest: BinaryHeap<Reverse<(u64, PathBuf, Option<u64>)>>,
    unreadable: u64,
}

impl Walk {
    fn file(&mut self, path: PathBuf, meta: &fs::Metadata) {
        let modified = meta
            .modified()
            .ok()
            .and_then(|m| m.duration_since(UNIX_EPOCH).ok())
            .map(|d| d.as_secs());
        self.largest.push(Reverse((meta.len(), path, modified)));
        if self.largest.len() > LARGEST_FILES {
            self.largest.pop();
        }
    }

    /// (bytes, files) under `dir`.
    fn folder(&mut self, dir: &Path) -> (u64, u64) {
        let Ok(entries) = fs::read_dir(dir) else {
            self.unreadable += 1;
            return (0, 0);
        };
        let (mut bytes, mut files) = (0, 0);
        for entry in entries.flatten() {
            let Ok(meta) = fs::symlink_metadata(entry.path()) else { continue };
            if takes_no_space(&meta) {
                continue;
            }
            if meta.is_dir() {
                let (b, f) = self.folder(&entry.path());
                bytes += b;
                files += f;
            } else {
                bytes += meta.len();
                files += 1;
                self.file(entry.path(), &meta);
            }
        }
        (bytes, files)
    }
}

/// The folder scanned when the user has not picked one: their profile.
pub fn default_root() -> Option<PathBuf> {
    std::env::var_os("USERPROFILE").filter(|v| !v.is_empty()).map(PathBuf::from)
}

pub fn scan(root: &Path) -> Result<DiskReport, String> {
    let entries =
        fs::read_dir(root).map_err(|e| format!("Could not read {}: {e}", root.display()))?;
    let mut walk = Walk { largest: BinaryHeap::new(), unreadable: 0 };
    let mut folders = Vec::new();
    let mut loose = (0u64, 0u64);

    for entry in entries.flatten() {
        let Ok(meta) = fs::symlink_metadata(entry.path()) else { continue };
        if takes_no_space(&meta) {
            continue;
        }
        if meta.is_dir() {
            let (size_bytes, file_count) = walk.folder(&entry.path());
            folders.push(FolderSize {
                name: entry.file_name().to_string_lossy().to_string(),
                path: entry.path().to_string_lossy().to_string(),
                size_bytes,
                file_count,
            });
        } else {
            loose.0 += meta.len();
            loose.1 += 1;
            walk.file(entry.path(), &meta);
        }
    }
    if loose.1 > 0 {
        folders.push(FolderSize {
            name: "Files in this folder".to_string(),
            path: root.to_string_lossy().to_string(),
            size_bytes: loose.0,
            file_count: loose.1,
        });
    }
    folders.sort_by(|a, b| b.size_bytes.cmp(&a.size_bytes).then_with(|| a.name.cmp(&b.name)));

    let largest_files = walk
        .largest
        .into_sorted_vec()
        .into_iter()
        .map(|Reverse((size_bytes, path, modified))| FileEntry {
            path: path.to_string_lossy().to_string(),
            size_bytes,
            modified,
        })
        .collect();

    Ok(DiskReport {
        root: root.to_string_lossy().to_string(),
        total_bytes: folders.iter().map(|f| f.size_bytes).sum(),
        file_count: folders.iter().map(|f| f.file_count).sum(),
        folders,
        largest_files,
        unreadable: walk.unreadable,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write(path: &Path, bytes: usize) {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, vec![0u8; bytes]).unwrap();
    }

    #[test]
    fn totals_folders_and_ranks_files() {
        let root = std::env::temp_dir().join(format!("system-tools-disk-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        write(&root.join("Videos/holiday.mp4"), 5000);
        write(&root.join("Videos/clips/short.mp4"), 1000);
        write(&root.join("Documents/notes.txt"), 20);
        write(&root.join("loose.bin"), 300);

        let report = scan(&root).unwrap();
        assert_eq!(report.total_bytes, 6320);
        assert_eq!(report.file_count, 4);
        let names: Vec<&str> = report.folders.iter().map(|f| f.name.as_str()).collect();
        assert_eq!(names, ["Videos", "Files in this folder", "Documents"]);
        assert_eq!(report.folders[0].size_bytes, 6000);
        assert_eq!(report.folders[0].file_count, 2);
        assert!(report.largest_files[0].path.ends_with("holiday.mp4"));
        assert_eq!(report.largest_files.len(), 4);
        assert!(report.largest_files.windows(2).all(|w| w[0].size_bytes >= w[1].size_bytes));

        assert!(scan(&root.join("missing")).is_err());
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn keeps_only_the_largest_files() {
        let root = std::env::temp_dir().join(format!("system-tools-disk-many-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        for i in 0..(LARGEST_FILES + 20) {
            write(&root.join(format!("d/f{i}")), i + 1);
        }
        let report = scan(&root).unwrap();
        assert_eq!(report.largest_files.len(), LARGEST_FILES);
        assert_eq!(report.largest_files[0].size_bytes, (LARGEST_FILES + 20) as u64);
        assert_eq!(report.largest_files.last().unwrap().size_bytes, 21);
        let _ = fs::remove_dir_all(&root);
    }
}
