//! Command-line scan: prints the driver inventory and the Windows Update
//! check as JSON. Handy for bug reports, and run in CI as a smoke test on a
//! real Windows machine.

fn main() {
    let scan = match driver_core::scan_drivers() {
        Ok(scan) => scan,
        Err(e) => {
            eprintln!("scan failed: {e}");
            std::process::exit(1);
        }
    };
    eprintln!("{} drivers found", scan.drivers.len());

    // The update check depends on the Windows Update service and the
    // network, so its failure is reported without failing the scan.
    let report = match driver_core::search_windows_update() {
        Ok(updates) => {
            eprintln!("{} driver updates offered by Windows Update", updates.len());
            Some(driver_core::analyze::match_updates(&scan.drivers, updates))
        }
        Err(e) => {
            eprintln!("update check failed: {e}");
            None
        }
    };
    println!("{}", driver_core::export::to_json(&scan, report.as_ref()));
}
