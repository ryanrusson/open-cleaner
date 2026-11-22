//! Driver updates offered by Windows Update, via the Windows Update Agent
//! COM API. Read-only: this searches, it never downloads or installs.

use windows::core::{Interface, BSTR};
use windows::Win32::Foundation::VARIANT_TRUE;
use windows::Win32::System::Com::{
    CoCreateInstance, CoInitializeEx, CoUninitialize, CLSCTX_INPROC_SERVER, COINIT_MULTITHREADED,
};
use windows::Win32::System::UpdateAgent::{
    ssWindowsUpdate, IUpdate, IUpdateSession, IWindowsDriverUpdate, UpdateSession,
};

use crate::date;
use crate::models::AvailableUpdate;

/// Balances a successful `CoInitializeEx` on this thread.
struct ComGuard(bool);

impl Drop for ComGuard {
    fn drop(&mut self) {
        if self.0 {
            unsafe { CoUninitialize() };
        }
    }
}

fn explain(e: windows::core::Error) -> String {
    // The codes people actually hit, in plain words.
    let hint = match e.code().0 as u32 {
        0x8024_402C | 0x8024_0438 | 0x8007_2EE2 | 0x8007_2EE7 | 0x8007_2EFD | 0x8024_401C => {
            "Could not reach Windows Update. Check your internet connection."
        }
        0x8024_002E | 0x8024_500C => {
            "Windows Update is disabled or managed by your organisation on this PC."
        }
        0x8024_001E | 0x8024_0016 | 0x8024_8007 => {
            "The Windows Update service is busy or not running. Try again in a moment."
        }
        _ => "Windows Update search failed.",
    };
    format!("{hint} (0x{:08X})", e.code().0 as u32)
}

fn text(r: windows::core::Result<BSTR>) -> String {
    r.map(|b| b.to_string().trim().to_string()).unwrap_or_default()
}

unsafe fn describe(update: &IUpdate) -> Option<AvailableUpdate> {
    // Anything that is not a driver update has no business in this list.
    let driver: IWindowsDriverUpdate = update.cast().ok()?;
    let size_bytes = update
        .MaxDownloadSize()
        .ok()
        // A COM DECIMAL; sizes are whole numbers that fit the low 64 bits.
        .map(|d| d.Anonymous2.Lo64)
        .unwrap_or(0);
    Some(AvailableUpdate {
        update_id: update
            .Identity()
            .and_then(|i| i.UpdateID())
            .map(|b| b.to_string())
            .unwrap_or_default(),
        title: text(update.Title()),
        description: text(update.Description()),
        driver_model: text(driver.DriverModel()),
        hardware_id: text(driver.DriverHardwareID()),
        provider: text(driver.DriverProvider()),
        manufacturer: text(driver.DriverManufacturer()),
        driver_class: text(driver.DriverClass()),
        date: driver.DriverVerDate().ok().and_then(date::iso_from_ole),
        size_bytes,
        support_url: text(update.SupportUrl()),
    })
}

pub fn search() -> Result<Vec<AvailableUpdate>, String> {
    unsafe {
        let _com = ComGuard(CoInitializeEx(None, COINIT_MULTITHREADED).is_ok());

        let session: IUpdateSession =
            CoCreateInstance(&UpdateSession, None, CLSCTX_INPROC_SERVER).map_err(explain)?;
        let _ = session.SetClientApplicationID(&BSTR::from("Windows Driver Updater"));
        let searcher = session.CreateUpdateSearcher().map_err(explain)?;
        // Ask Microsoft directly rather than a corporate WSUS server, which
        // usually does not carry drivers at all.
        let _ = searcher.SetServerSelection(ssWindowsUpdate);
        let _ = searcher.SetOnline(VARIANT_TRUE);

        let result = searcher
            .Search(&BSTR::from("IsInstalled=0 and Type='Driver' and IsHidden=0"))
            .map_err(explain)?;
        let updates = result.Updates().map_err(explain)?;
        let count = updates.Count().map_err(explain)?;

        let mut found = Vec::new();
        for i in 0..count {
            if let Ok(update) = updates.get_Item(i) {
                found.extend(describe(&update));
            }
        }
        Ok(found)
    }
}
