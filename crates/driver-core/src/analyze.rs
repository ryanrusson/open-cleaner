//! Turns raw Windows data into what the UI shows.

use std::collections::HashMap;

use crate::date;
use crate::models::*;
use crate::vendor;

/// Device Manager problem codes worth surfacing. Codes that reflect a
/// deliberate user choice (22: disabled) or absence (45: unplugged) are not
/// problems and return `None`.
pub fn describe_problem(code: u32) -> Option<&'static str> {
    Some(match code {
        0 | 22 | 45 => return None,
        1 => "Device is not configured correctly",
        3 => "Driver may be corrupted, or the system is low on memory",
        10 => "Device cannot start",
        12 => "Not enough free resources for this device",
        14 => "Restart required before this device works",
        18 => "Drivers need to be reinstalled",
        19 => "Registry configuration is damaged",
        24 => "Device is missing or its driver is not installed correctly",
        28 => "No driver is installed for this device",
        31 => "Windows cannot load the driver for this device",
        32 => "The driver's service is disabled",
        37 => "Windows cannot initialise the driver",
        39 => "Driver is corrupted or missing",
        40 | 41 | 48 => "Driver is blocked or could not be loaded",
        43 => "Device reported a problem and was stopped",
        52 => "Driver's digital signature could not be verified",
        _ => "Device has a problem",
    })
}

/// Friendly name for a device setup class, matching Device Manager's wording.
pub fn class_label(class: &str) -> String {
    match class.to_ascii_uppercase().as_str() {
        "DISPLAY" => "Display adapters",
        "NET" => "Network adapters",
        "MEDIA" => "Sound, video and game controllers",
        "AUDIOENDPOINT" => "Audio inputs and outputs",
        "SYSTEM" => "System devices",
        "USB" => "USB controllers",
        "BLUETOOTH" => "Bluetooth",
        "HDC" => "IDE ATA/ATAPI controllers",
        "SCSIADAPTER" => "Storage controllers",
        "DISKDRIVE" => "Disk drives",
        "MONITOR" => "Monitors",
        "KEYBOARD" => "Keyboards",
        "MOUSE" => "Mice and pointing devices",
        "HIDCLASS" => "Human Interface Devices",
        "CAMERA" | "IMAGE" => "Cameras",
        "PRINTER" | "PRINTQUEUE" => "Printers",
        "FIRMWARE" => "Firmware",
        "PROCESSOR" => "Processors",
        "BIOMETRIC" => "Biometric devices",
        "SOFTWARECOMPONENT" => "Software components",
        "SOFTWAREDEVICE" => "Software devices",
        "EXTENSION" => "Extensions",
        "VOLUME" => "Storage volumes",
        "COMPUTER" => "Computer",
        "BATTERY" => "Batteries",
        "PORTS" => "Ports (COM & LPT)",
        "SECURITYDEVICES" => "Security devices",
        "" => "Other devices",
        _ => return class.to_string(),
    }
    .to_string()
}

pub fn build_scan(system: SystemInfo, raw: Vec<RawDriver>, today: i64) -> ScanResult {
    let mut drivers: Vec<Driver> = raw
        .into_iter()
        .map(|r| {
            let age_days = r
                .date
                .as_deref()
                .and_then(date::days_from_iso)
                .map(|d| today - d)
                .filter(|age| *age >= 0);
            let vendor_hint = vendor::hint(&r, age_days, &system);
            let problem = r.problem_code.and_then(|code| {
                describe_problem(code).map(|d| Problem { code, description: d.to_string() })
            });
            Driver {
                is_inbox: vendor::is_inbox(&r.provider),
                device_class: class_label(&r.device_class),
                device_id: r.device_id,
                device_name: r.device_name,
                manufacturer: r.manufacturer,
                provider: r.provider,
                version: r.version,
                date: r.date,
                age_days,
                hardware_id: r.hardware_id,
                inf_name: r.inf_name,
                is_signed: r.is_signed,
                signer: r.signer,
                problem,
                vendor_hint,
            }
        })
        .collect();
    drivers.sort_by_cached_key(|d| {
        (d.device_class.to_lowercase(), d.device_name.to_lowercase(), d.device_id.clone())
    });
    ScanResult { system, drivers, scanned_on: date::iso_from_days(today) }
}

/// Tie each offered update to the installed driver(s) it would replace.
///
/// Windows Update's optional list routinely includes drivers *older* than
/// what is installed; those are dropped into `unmatched` untouched rather
/// than being presented as upgrades.
pub fn match_updates(drivers: &[Driver], updates: Vec<AvailableUpdate>) -> UpdateReport {
    let mut by_hardware_id: HashMap<String, Vec<&Driver>> = HashMap::new();
    for d in drivers.iter().filter(|d| !d.hardware_id.is_empty()) {
        by_hardware_id.entry(d.hardware_id.to_ascii_lowercase()).or_default().push(d);
    }

    let mut best: HashMap<&str, AvailableUpdate> = HashMap::new();
    let mut unmatched = Vec::new();
    for update in updates {
        let targets: Vec<&Driver> = by_hardware_id
            .get(&update.hardware_id.to_ascii_lowercase())
            .into_iter()
            .flatten()
            .copied()
            // ISO dates compare correctly as strings.
            .filter(|d| match (&update.date, &d.date) {
                (Some(new), Some(installed)) => new > installed,
                _ => true,
            })
            .collect();
        if targets.is_empty() {
            unmatched.push(update);
            continue;
        }
        for d in targets {
            match best.get(d.device_id.as_str()) {
                Some(current) if current.date >= update.date => {}
                _ => {
                    best.insert(&d.device_id, update.clone());
                }
            }
        }
    }

    let mut matched: Vec<MatchedUpdate> = best
        .into_iter()
        .map(|(device_id, update)| MatchedUpdate { device_id: device_id.to_string(), update })
        .collect();
    matched.sort_by(|a, b| a.device_id.cmp(&b.device_id));
    UpdateReport { matched, unmatched }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn raw(id: &str, hw: &str, date: &str) -> RawDriver {
        RawDriver {
            device_id: id.into(),
            device_name: format!("Device {id}"),
            device_class: "NET".into(),
            provider: "Intel".into(),
            hardware_id: hw.into(),
            date: Some(date.into()),
            ..Default::default()
        }
    }

    fn update(id: &str, hw: &str, date: &str) -> AvailableUpdate {
        AvailableUpdate {
            update_id: id.into(),
            hardware_id: hw.into(),
            date: Some(date.into()),
            ..Default::default()
        }
    }

    fn scan(raws: Vec<RawDriver>) -> Vec<Driver> {
        build_scan(SystemInfo::default(), raws, date::days_from_civil(2026, 1, 1)).drivers
    }

    #[test]
    fn scan_computes_age_and_flags() {
        let mut r = raw("a", "PCI\\VEN_8086", "2023-01-01");
        r.problem_code = Some(28);
        let mut disabled = raw("b", "X", "2030-01-01");
        disabled.problem_code = Some(22);
        let drivers = scan(vec![r, disabled]);
        assert_eq!(drivers[0].age_days, Some(1096));
        assert_eq!(drivers[0].problem.as_ref().unwrap().code, 28);
        assert!(drivers[0].vendor_hint.is_some());
        assert_eq!(drivers[0].device_class, "Network adapters");
        // Disabled is not a problem; a date in the future yields no age.
        assert!(drivers[1].problem.is_none());
        assert_eq!(drivers[1].age_days, None);
    }

    #[test]
    fn matches_case_insensitively_and_keeps_newest() {
        let drivers = scan(vec![raw("a", "PCI\\VEN_8086&DEV_1", "2023-01-01")]);
        let report = match_updates(
            &drivers,
            vec![
                update("u1", "pci\\ven_8086&dev_1", "2024-01-01"),
                update("u2", "pci\\ven_8086&dev_1", "2025-01-01"),
            ],
        );
        assert_eq!(report.matched.len(), 1);
        assert_eq!(report.matched[0].update.update_id, "u2");
        assert!(report.unmatched.is_empty());
    }

    #[test]
    fn older_or_unknown_updates_are_not_presented_as_upgrades() {
        let drivers = scan(vec![raw("a", "HW1", "2025-06-01")]);
        let report = match_updates(
            &drivers,
            vec![update("old", "HW1", "2022-01-01"), update("other", "HW9", "2025-12-01")],
        );
        assert!(report.matched.is_empty());
        assert_eq!(report.unmatched.len(), 2);
    }

    #[test]
    fn one_update_can_cover_several_identical_devices() {
        let drivers = scan(vec![raw("a", "HW1", "2023-01-01"), raw("b", "HW1", "2023-01-01")]);
        let report = match_updates(&drivers, vec![update("u", "HW1", "2024-01-01")]);
        assert_eq!(report.matched.len(), 2);
    }
}
