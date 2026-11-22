//! Installed-driver inventory via WMI.

use std::collections::HashMap;

use serde::Deserialize;
use wmi::WMIConnection;

use crate::date;
use crate::models::{RawDriver, SystemInfo};

#[derive(Deserialize)]
struct SignedDriver {
    #[serde(rename = "DeviceID")]
    device_id: Option<String>,
    #[serde(rename = "DeviceName")]
    device_name: Option<String>,
    #[serde(rename = "DeviceClass")]
    device_class: Option<String>,
    #[serde(rename = "Manufacturer")]
    manufacturer: Option<String>,
    #[serde(rename = "DriverProviderName")]
    provider: Option<String>,
    #[serde(rename = "DriverVersion")]
    version: Option<String>,
    #[serde(rename = "DriverDate")]
    date: Option<String>,
    #[serde(rename = "HardWareID")]
    hardware_id: Option<String>,
    #[serde(rename = "InfName")]
    inf_name: Option<String>,
    #[serde(rename = "IsSigned")]
    is_signed: Option<bool>,
    #[serde(rename = "Signer")]
    signer: Option<String>,
}

#[derive(Deserialize)]
struct ProblemDevice {
    #[serde(rename = "PNPDeviceID")]
    device_id: Option<String>,
    #[serde(rename = "Name")]
    name: Option<String>,
    #[serde(rename = "PNPClass")]
    class: Option<String>,
    #[serde(rename = "Manufacturer")]
    manufacturer: Option<String>,
    #[serde(rename = "ConfigManagerErrorCode")]
    code: Option<u32>,
    #[serde(rename = "HardwareID")]
    hardware_ids: Option<Vec<String>>,
}

#[derive(Deserialize)]
struct ComputerSystem {
    #[serde(rename = "Manufacturer")]
    manufacturer: Option<String>,
    #[serde(rename = "Model")]
    model: Option<String>,
}

#[derive(Deserialize)]
struct OperatingSystem {
    #[serde(rename = "Caption")]
    caption: Option<String>,
    #[serde(rename = "Version")]
    version: Option<String>,
}

fn text(s: Option<String>) -> String {
    s.map(|s| s.trim().to_string()).unwrap_or_default()
}

fn err(context: &str, e: wmi::WMIError) -> String {
    format!("{context}: {e}")
}

pub fn scan() -> Result<(SystemInfo, Vec<RawDriver>), String> {
    let wmi = WMIConnection::new().map_err(|e| err("Could not connect to WMI", e))?;

    let signed: Vec<SignedDriver> = wmi
        .raw_query(
            "SELECT DeviceID, DeviceName, DeviceClass, Manufacturer, DriverProviderName, \
             DriverVersion, DriverDate, HardWareID, InfName, IsSigned, Signer \
             FROM Win32_PnPSignedDriver",
        )
        .map_err(|e| err("Could not list installed drivers", e))?;

    let mut drivers: Vec<RawDriver> = signed
        .into_iter()
        .filter(|d| d.device_name.as_deref().is_some_and(|n| !n.trim().is_empty()))
        .map(|d| RawDriver {
            device_id: text(d.device_id),
            device_name: text(d.device_name),
            device_class: text(d.device_class),
            manufacturer: text(d.manufacturer),
            provider: text(d.provider),
            version: text(d.version),
            date: d.date.as_deref().and_then(date::iso_from_cim),
            hardware_id: text(d.hardware_id),
            inf_name: text(d.inf_name),
            is_signed: d.is_signed.unwrap_or(false),
            signer: text(d.signer),
            problem_code: None,
        })
        .collect();

    // Devices in an error state. Those with no driver at all (code 28) never
    // appear in Win32_PnPSignedDriver, so they are added as their own rows.
    // A failure here should not sink the whole scan.
    let problems: Vec<ProblemDevice> = wmi
        .raw_query(
            "SELECT PNPDeviceID, Name, PNPClass, Manufacturer, ConfigManagerErrorCode, HardwareID \
             FROM Win32_PnPEntity WHERE ConfigManagerErrorCode <> 0",
        )
        .unwrap_or_default();

    let index: HashMap<String, usize> = drivers
        .iter()
        .enumerate()
        .map(|(i, d)| (d.device_id.to_ascii_lowercase(), i))
        .collect();
    for p in problems {
        let Some(code) = p.code else { continue };
        let device_id = text(p.device_id);
        match index.get(&device_id.to_ascii_lowercase()) {
            Some(&i) => drivers[i].problem_code = Some(code),
            None => drivers.push(RawDriver {
                device_id,
                device_name: p
                    .name
                    .map(|n| n.trim().to_string())
                    .filter(|n| !n.is_empty())
                    .unwrap_or_else(|| "Unknown device".to_string()),
                device_class: text(p.class),
                manufacturer: text(p.manufacturer),
                hardware_id: p.hardware_ids.and_then(|h| h.into_iter().next()).unwrap_or_default(),
                problem_code: Some(code),
                ..Default::default()
            }),
        }
    }

    let computer = wmi
        .raw_query::<ComputerSystem>("SELECT Manufacturer, Model FROM Win32_ComputerSystem")
        .ok()
        .and_then(|v| v.into_iter().next());
    let os = wmi
        .raw_query::<OperatingSystem>("SELECT Caption, Version FROM Win32_OperatingSystem")
        .ok()
        .and_then(|v| v.into_iter().next());
    let (manufacturer, model) =
        computer.map(|c| (text(c.manufacturer), text(c.model))).unwrap_or_default();
    let os = os
        .map(|o| format!("{} ({})", text(o.caption), text(o.version)))
        .unwrap_or_default();

    Ok((SystemInfo { manufacturer, model, os }, drivers))
}
