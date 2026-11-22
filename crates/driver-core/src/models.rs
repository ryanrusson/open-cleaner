use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SystemInfo {
    pub manufacturer: String,
    pub model: String,
    pub os: String,
}

/// A driver exactly as Windows reports it, before any judgement is applied.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct RawDriver {
    pub device_id: String,
    pub device_name: String,
    pub device_class: String,
    pub manufacturer: String,
    pub provider: String,
    pub version: String,
    /// ISO date, `YYYY-MM-DD`.
    pub date: Option<String>,
    pub hardware_id: String,
    pub inf_name: String,
    pub is_signed: bool,
    pub signer: String,
    /// Device Manager problem code; `None` when the device is working.
    pub problem_code: Option<u32>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct VendorHint {
    pub vendor: String,
    pub url: String,
    pub reason: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Problem {
    pub code: u32,
    pub description: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Driver {
    pub device_id: String,
    pub device_name: String,
    pub device_class: String,
    pub manufacturer: String,
    pub provider: String,
    pub version: String,
    pub date: Option<String>,
    pub age_days: Option<i64>,
    pub hardware_id: String,
    pub inf_name: String,
    pub is_signed: bool,
    pub signer: String,
    /// Shipped with Windows itself; these are serviced by normal Windows updates.
    pub is_inbox: bool,
    pub problem: Option<Problem>,
    pub vendor_hint: Option<VendorHint>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ScanResult {
    pub system: SystemInfo,
    pub drivers: Vec<Driver>,
    /// ISO date the scan ran.
    pub scanned_on: String,
}

/// A driver update offered by Windows Update for this machine.
#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AvailableUpdate {
    pub update_id: String,
    pub title: String,
    pub description: String,
    pub driver_model: String,
    pub hardware_id: String,
    pub provider: String,
    pub manufacturer: String,
    pub driver_class: String,
    pub date: Option<String>,
    pub size_bytes: u64,
    pub support_url: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct MatchedUpdate {
    pub device_id: String,
    pub update: AvailableUpdate,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct UpdateReport {
    /// Updates tied to a specific installed driver.
    pub matched: Vec<MatchedUpdate>,
    /// Updates Windows offers that we could not tie to a scanned device.
    pub unmatched: Vec<AvailableUpdate>,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub enum InstallStage {
    Downloading,
    Installing,
    Installed,
    Failed,
}

/// Progress reported while updates install. Sent as JSON lines from the
/// elevated helper process to the app, then on to the UI.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(tag = "kind", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum InstallEvent {
    /// What the installer is doing between updates.
    Status { message: String },
    Update { update_id: String, stage: InstallStage, detail: String },
    Finished { installed: u32, failed: u32, reboot_required: bool },
    /// The run could not proceed at all.
    Error { message: String },
}
