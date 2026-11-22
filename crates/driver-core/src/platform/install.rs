//! Download and install driver updates through the Windows Update Agent.
//! Must run elevated; see [`super::elevate`] for how the app gets there.

use std::collections::HashMap;

use windows::core::Interface;
use windows::Win32::System::Com::{CoCreateInstance, CLSCTX_INPROC_SERVER};
use windows::Win32::System::UpdateAgent::{
    orcSucceeded, orcSucceededWithErrors, IUpdate, IUpdateCollection, IUpdateSession,
    IWindowsDriverUpdate, OperationResultCode, UpdateCollection,
};

use super::restore;
use super::windows_update::{explain, open_and_search, update_id, ComGuard};
use crate::models::{InstallEvent, InstallStage};

fn succeeded(code: OperationResultCode) -> bool {
    code == orcSucceeded || code == orcSucceededWithErrors
}

fn failure(hresult: i32) -> String {
    format!("Windows Update reported error 0x{:08X}", hresult as u32)
}

/// Returns whether Windows wants a restart to finish this update.
unsafe fn install_one(
    session: &IUpdateSession,
    update: &IUpdate,
    id: &str,
    emit: &mut dyn FnMut(InstallEvent),
) -> Result<bool, String> {
    let mut stage = |stage| {
        emit(InstallEvent::Update { update_id: id.to_string(), stage, detail: String::new() })
    };

    let batch: IUpdateCollection =
        CoCreateInstance(&UpdateCollection, None, CLSCTX_INPROC_SERVER).map_err(explain)?;
    batch.Add(update).map_err(explain)?;
    if !update.EulaAccepted().map_err(explain)?.as_bool() {
        update.AcceptEula().map_err(explain)?;
    }

    if !update.IsDownloaded().map_err(explain)?.as_bool() {
        stage(InstallStage::Downloading);
        let downloader = session.CreateUpdateDownloader().map_err(explain)?;
        downloader.SetUpdates(&batch).map_err(explain)?;
        let result = downloader.Download().map_err(explain)?;
        if !succeeded(result.ResultCode().map_err(explain)?) {
            return Err(format!("Download failed. {}", failure(result.HResult().unwrap_or(0))));
        }
    }

    stage(InstallStage::Installing);
    let installer = session.CreateUpdateInstaller().map_err(explain)?;
    installer.SetUpdates(&batch).map_err(explain)?;
    let result = installer.Install().map_err(explain)?;
    if !succeeded(result.ResultCode().map_err(explain)?) {
        let hresult = result
            .GetUpdateResult(0)
            .and_then(|r| r.HResult())
            .or_else(|_| result.HResult())
            .unwrap_or(0);
        return Err(failure(hresult));
    }
    Ok(result.RebootRequired().map(|b| b.as_bool()).unwrap_or(false))
}

/// Install the updates with the given Windows Update IDs, one at a time so
/// that one bad driver does not stop the rest.
pub fn run(ids: &[String], emit: &mut dyn FnMut(InstallEvent)) -> Result<(), String> {
    unsafe {
        let _com = ComGuard::init();

        emit(InstallEvent::Status { message: "Contacting Windows Update…".into() });
        let (session, offered) = open_and_search()?;
        let mut by_id: HashMap<String, IUpdate> = HashMap::new();
        for i in 0..offered.Count().map_err(explain)? {
            if let Ok(update) = offered.get_Item(i) {
                // Drivers only, whatever the caller asked for.
                if update.cast::<IWindowsDriverUpdate>().is_ok() {
                    by_id.insert(update_id(&update).to_ascii_lowercase(), update);
                }
            }
        }

        emit(InstallEvent::Status { message: "Creating a restore point…".into() });
        let restore_point = restore::begin("Driver Updater: before driver updates");
        emit(InstallEvent::Status {
            message: if restore_point.is_some() {
                "Restore point created.".into()
            } else {
                "Windows did not create a restore point (System Protection may be off, or one \
                 was made recently). Continuing."
                    .into()
            },
        });

        let (mut installed, mut failed, mut reboot_required) = (0, 0, false);
        for id in ids {
            let outcome = match by_id.get(&id.to_ascii_lowercase()) {
                Some(update) => install_one(&session, update, id, emit),
                None => Err("Windows Update no longer offers this driver.".into()),
            };
            let (stage, detail) = match outcome {
                Ok(reboot) => {
                    installed += 1;
                    reboot_required |= reboot;
                    (InstallStage::Installed, String::new())
                }
                Err(message) => {
                    failed += 1;
                    (InstallStage::Failed, message)
                }
            };
            emit(InstallEvent::Update { update_id: id.clone(), stage, detail });
        }

        if let Some(sequence) = restore_point {
            restore::end(sequence);
        }
        emit(InstallEvent::Finished { installed, failed, reboot_required });
        Ok(())
    }
}
