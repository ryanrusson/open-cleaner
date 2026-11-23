//! Command-line run of every read-only tool, printed as JSON. Run in CI as
//! a smoke test on a real Windows machine. Changes nothing.

fn main() {
    let junk = system_tools::cleaner::scan();
    let startup = system_tools::startup::list().unwrap_or_else(|e| fail("startup list", &e));
    let apps = system_tools::apps::list().unwrap_or_else(|e| fail("installed apps", &e));
    let disk = std::env::temp_dir();
    let disk = system_tools::disk::scan(&disk).unwrap_or_else(|e| fail("disk scan", &e));

    eprintln!("{} junk categories, {} bytes", junk.len(), junk.iter().map(|c| c.size_bytes).sum::<u64>());
    eprintln!("{} startup items", startup.len());
    eprintln!("{} installed apps", apps.len());
    eprintln!("{} files, {} bytes under {}", disk.file_count, disk.total_bytes, disk.root);
    // winget is not on every Windows image, so this one may be unavailable.
    match system_tools::software::list() {
        Ok(updates) => eprintln!("{} app updates offered by winget", updates.len()),
        Err(e) => eprintln!("app updates unavailable: {e}"),
    }
    let duplicates = system_tools::duplicates::find(
        &std::env::temp_dir(),
        system_tools::duplicates::DEFAULT_MIN_SIZE,
        &std::sync::atomic::AtomicBool::new(false),
    )
    .unwrap_or_else(|e| fail("duplicate scan", &e));
    eprintln!("{} duplicate groups among {} files", duplicates.groups.len(), duplicates.files_checked);
    if apps.is_empty() {
        fail("installed apps", "found none, which cannot be right on a real PC");
    }
    println!(
        "{}",
        serde_json::json!({ "junk": junk, "startup": startup, "apps": apps, "disk": disk.folders })
    );
}

fn fail(what: &str, error: &str) -> ! {
    eprintln!("{what} failed: {error}");
    std::process::exit(1);
}
