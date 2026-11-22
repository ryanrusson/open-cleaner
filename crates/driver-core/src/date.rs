//! Minimal calendar maths so we do not need a date crate.

/// Days since 1970-01-01 for a proleptic Gregorian date.
pub fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400;
    let doy = (153 * (m + if m > 2 { -3 } else { 9 }) + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

pub fn civil_from_days(z: i64) -> (i64, i64, i64) {
    let z = z + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    (yoe + era * 400 + if m <= 2 { 1 } else { 0 }, m, d)
}

pub fn iso_from_days(days: i64) -> String {
    let (y, m, d) = civil_from_days(days);
    format!("{y:04}-{m:02}-{d:02}")
}

/// Parse `YYYY-MM-DD` into days since the Unix epoch.
pub fn days_from_iso(s: &str) -> Option<i64> {
    let mut parts = s.splitn(3, '-');
    let y: i64 = parts.next()?.parse().ok()?;
    let m: i64 = parts.next()?.parse().ok()?;
    let d: i64 = parts.next()?.get(..2)?.parse().ok()?;
    ((1..=12).contains(&m) && (1..=31).contains(&d)).then(|| days_from_civil(y, m, d))
}

/// WMI dates look like `20230615000000.000000-000`.
pub fn iso_from_cim(s: &str) -> Option<String> {
    let digits = s.get(..8)?;
    if !digits.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let iso = format!("{}-{}-{}", &digits[..4], &digits[4..6], &digits[6..8]);
    days_from_iso(&iso).map(|_| iso)
}

/// COM `DATE`: fractional days since 1899-12-30.
pub fn iso_from_ole(date: f64) -> Option<String> {
    // Anything before 1990 is a placeholder, not a real driver date.
    (date.is_finite() && date > 32_874.0).then(|| iso_from_days(date.floor() as i64 - 25_569))
}

/// Today as days since the Unix epoch (UTC).
pub fn today() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| (d.as_secs() / 86_400) as i64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips() {
        assert_eq!(days_from_civil(1970, 1, 1), 0);
        assert_eq!(iso_from_days(days_from_civil(2024, 2, 29)), "2024-02-29");
        assert_eq!(days_from_iso("2026-11-22"), Some(days_from_civil(2026, 11, 22)));
        assert_eq!(days_from_iso("nonsense"), None);
    }

    #[test]
    fn parses_windows_formats() {
        assert_eq!(iso_from_cim("20230615000000.000000-000").as_deref(), Some("2023-06-15"));
        assert_eq!(iso_from_cim("garbage"), None);
        // 45000 is 2023-03-15 in OLE automation dates.
        assert_eq!(iso_from_ole(45_000.0).as_deref(), Some("2023-03-15"));
        assert_eq!(iso_from_ole(0.0), None);
    }
}
