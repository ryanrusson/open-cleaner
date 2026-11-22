use crate::models::*;

fn csv_field(s: &str) -> String {
    // Leading =,+,-,@ would be run as a formula by spreadsheet apps.
    let s = if s.starts_with(['=', '+', '-', '@']) { format!("'{s}") } else { s.to_string() };
    if s.contains([',', '"', '\n', '\r']) {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s
    }
}

pub fn status(driver: &Driver, update: Option<&AvailableUpdate>) -> &'static str {
    if driver.problem.is_some() {
        "Problem"
    } else if update.is_some() {
        "Update available"
    } else if driver.vendor_hint.is_some() {
        "Worth checking"
    } else {
        "Up to date"
    }
}

pub fn to_csv(scan: &ScanResult, report: Option<&UpdateReport>) -> String {
    let mut out = String::from(
        "Device,Class,Provider,Version,Driver date,Status,Details,Hardware ID,INF,Signed\r\n",
    );
    for d in &scan.drivers {
        let update = report
            .and_then(|r| r.matched.iter().find(|m| m.device_id == d.device_id))
            .map(|m| &m.update);
        let details = if let Some(p) = &d.problem {
            format!("Code {}: {}", p.code, p.description)
        } else if let Some(u) = update {
            u.title.clone()
        } else if let Some(h) = &d.vendor_hint {
            h.url.clone()
        } else {
            String::new()
        };
        let row = [
            d.device_name.as_str(),
            &d.device_class,
            &d.provider,
            &d.version,
            d.date.as_deref().unwrap_or(""),
            status(d, update),
            &details,
            &d.hardware_id,
            &d.inf_name,
            if d.is_signed { "Yes" } else { "No" },
        ];
        out.push_str(&row.map(csv_field).join(","));
        out.push_str("\r\n");
    }
    out
}

pub fn to_json(scan: &ScanResult, report: Option<&UpdateReport>) -> String {
    serde_json::to_string_pretty(&serde_json::json!({ "scan": scan, "updates": report }))
        .expect("report types always serialise")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn escapes_awkward_fields() {
        assert_eq!(csv_field("plain"), "plain");
        assert_eq!(csv_field("a, \"b\""), "\"a, \"\"b\"\"\"");
        assert_eq!(csv_field("=cmd()"), "'=cmd()");
    }
}
