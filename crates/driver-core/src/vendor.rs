//! "Worth checking" hints: drivers that Windows Update considers current but
//! that are old enough that the vendor has very likely shipped something newer.
//! We only ever link to the vendor's own official download page.

use crate::models::{RawDriver, SystemInfo, VendorHint};

/// How old a driver may get before we suggest a look, by setup class.
/// Classes not listed here (monitors, keyboards, ...) never get a hint.
fn stale_after_days(class: &str) -> Option<i64> {
    match class.to_ascii_uppercase().as_str() {
        "DISPLAY" => Some(180),
        "NET" | "BLUETOOTH" | "MEDIA" | "SYSTEM" | "USB" | "HDC" | "SCSIADAPTER" | "CAMERA"
        | "IMAGE" | "FIRMWARE" => Some(730),
        _ => None,
    }
}

const CHIP_VENDORS: &[(&[&str], &str, &str)] = &[
    (&["nvidia"], "NVIDIA", "https://www.nvidia.com/en-us/drivers/"),
    (
        &["advanced micro devices", "amd", "ati technologies"],
        "AMD",
        "https://www.amd.com/en/support/download/drivers.html",
    ),
    (
        &["intel"],
        "Intel",
        "https://www.intel.com/content/www/us/en/support/detect.html",
    ),
];

const PC_MAKERS: &[(&[&str], &str, &str)] = &[
    (&["dell", "alienware"], "Dell", "https://www.dell.com/support/home"),
    (&["hp", "hewlett"], "HP", "https://support.hp.com/drivers"),
    (&["lenovo"], "Lenovo", "https://pcsupport.lenovo.com/"),
    (&["asus"], "ASUS", "https://www.asus.com/support/download-center/"),
    (&["acer"], "Acer", "https://www.acer.com/support"),
    (&["micro-star", "msi"], "MSI", "https://www.msi.com/support/download"),
    (&["microsoft"], "Microsoft Surface", "https://support.microsoft.com/surface"),
    (&["samsung"], "Samsung", "https://www.samsung.com/support/"),
    (&["gigabyte"], "Gigabyte", "https://www.gigabyte.com/Support"),
];

/// Match on whole words so "amd" does not fire inside e.g. "Camden".
fn mentions(haystack: &str, needle: &str) -> bool {
    let haystack = haystack.to_ascii_lowercase();
    haystack.match_indices(needle).any(|(start, _)| {
        let end = start + needle.len();
        let boundary = |c: Option<char>| c.is_none_or(|c| !c.is_ascii_alphanumeric());
        boundary(haystack[..start].chars().next_back()) && boundary(haystack[end..].chars().next())
    })
}

fn lookup(
    table: &'static [(&[&str], &str, &str)],
    texts: &[&str],
) -> Option<(&'static str, &'static str)> {
    table
        .iter()
        .find(|(keys, _, _)| keys.iter().any(|k| texts.iter().any(|t| mentions(t, k))))
        .map(|&(_, name, url)| (name, url))
}

pub fn is_inbox(provider: &str) -> bool {
    provider.trim().to_ascii_lowercase().starts_with("microsoft")
}

fn describe_age(days: i64) -> String {
    if days >= 365 {
        format!("{:.1} years old", days as f64 / 365.25)
    } else {
        format!("{} months old", (days / 30).max(1))
    }
}

pub fn hint(driver: &RawDriver, age_days: Option<i64>, system: &SystemInfo) -> Option<VendorHint> {
    let age = age_days?;
    if is_inbox(&driver.provider) || age < stale_after_days(&driver.device_class)? {
        return None;
    }
    let (vendor, url) = lookup(CHIP_VENDORS, &[&driver.provider, &driver.manufacturer])
        .or_else(|| lookup(PC_MAKERS, &[&system.manufacturer]))?;
    Some(VendorHint {
        vendor: vendor.to_string(),
        url: url.to_string(),
        reason: format!(
            "This driver is {}. Windows Update has nothing newer, but {vendor} may.",
            describe_age(age)
        ),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn driver(class: &str, provider: &str) -> RawDriver {
        RawDriver {
            device_class: class.into(),
            provider: provider.into(),
            ..Default::default()
        }
    }

    #[test]
    fn old_gpu_driver_points_at_chip_vendor() {
        let h = hint(&driver("DISPLAY", "NVIDIA"), Some(400), &SystemInfo::default()).unwrap();
        assert_eq!(h.vendor, "NVIDIA");
        assert!(h.reason.contains("1.1 years"));
    }

    #[test]
    fn recent_or_inbox_or_unimportant_gets_nothing() {
        let sys = SystemInfo::default();
        assert!(hint(&driver("DISPLAY", "NVIDIA"), Some(90), &sys).is_none());
        assert!(hint(&driver("NET", "Microsoft"), Some(4000), &sys).is_none());
        assert!(hint(&driver("MONITOR", "Intel"), Some(4000), &sys).is_none());
        assert!(hint(&driver("NET", "Intel"), None, &sys).is_none());
    }

    #[test]
    fn unknown_chip_vendor_falls_back_to_pc_maker() {
        let sys = SystemInfo { manufacturer: "Dell Inc.".into(), ..Default::default() };
        assert_eq!(hint(&driver("MEDIA", "Realtek"), Some(900), &sys).unwrap().vendor, "Dell");
        assert!(hint(&driver("MEDIA", "Realtek"), Some(900), &SystemInfo::default()).is_none());
    }

    #[test]
    fn vendor_names_match_whole_words_only() {
        assert!(mentions("Advanced Micro Devices, Inc.", "advanced micro devices"));
        assert!(mentions("AMD", "amd"));
        assert!(!mentions("Camden Systems", "amd"));
        assert!(!mentions("Chipset", "hp"));
    }
}
