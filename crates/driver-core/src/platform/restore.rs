//! Best-effort system restore point around a driver install.
//!
//! Windows silently skips this when System Protection is off, or when a
//! restore point was already made in the last 24 hours, so failure here is
//! reported but never blocks the install.

use windows::Win32::System::Restore::{
    SRSetRestorePointW, BEGIN_SYSTEM_CHANGE, DEVICE_DRIVER_INSTALL, END_SYSTEM_CHANGE,
    RESTOREPOINTINFOW, STATEMGRSTATUS,
};

/// Returns the sequence number to pass to [`end`], or `None` if Windows
/// did not create a restore point.
pub fn begin(description: &str) -> Option<i64> {
    let mut info = RESTOREPOINTINFOW {
        dwEventType: BEGIN_SYSTEM_CHANGE,
        dwRestorePtType: DEVICE_DRIVER_INSTALL,
        ..Default::default()
    };
    let mut text = [0u16; 256];
    for (slot, unit) in text.iter_mut().take(255).zip(description.encode_utf16()) {
        *slot = unit;
    }
    info.szDescription = text;
    let mut status = STATEMGRSTATUS::default();
    let ok = unsafe { SRSetRestorePointW(&info, &mut status) }.as_bool();
    let sequence = status.llSequenceNumber;
    (ok && sequence != 0).then_some(sequence)
}

pub fn end(sequence: i64) {
    let info = RESTOREPOINTINFOW {
        dwEventType: END_SYSTEM_CHANGE,
        dwRestorePtType: DEVICE_DRIVER_INSTALL,
        llSequenceNumber: sequence,
        ..Default::default()
    };
    let mut status = STATEMGRSTATUS::default();
    let _ = unsafe { SRSetRestorePointW(&info, &mut status) };
}
