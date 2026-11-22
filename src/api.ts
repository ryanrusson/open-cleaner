import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { save } from "@tauri-apps/plugin-dialog";
import { mockReport, mockScan } from "./mock";
import type { InstallEvent, ScanResult, UpdateReport } from "./types";

/** False when the UI is opened in a plain browser, where it runs on sample data. */
export const inApp = "__TAURI_INTERNALS__" in window;

const later = <T,>(value: T, ms: number) =>
  new Promise<T>((resolve) => setTimeout(() => resolve(value), ms));

export const scanDrivers = (): Promise<ScanResult> =>
  inApp ? invoke("scan_drivers") : later(mockScan, 500);

export const checkUpdates = (): Promise<UpdateReport> =>
  inApp ? invoke("check_updates") : later(mockReport, 1800);

export const openLink = (url: string): Promise<void> =>
  inApp ? invoke("open_link", { url }) : later(void window.open(url, "_blank"), 0);

export const WINDOWS_UPDATE_SETTINGS = "ms-settings:windowsupdate-optionalupdates";

/** Returns the saved path, or null if the user cancelled. */
export async function exportReport(): Promise<string | null> {
  if (!inApp) return null;
  const path = await save({
    title: "Export driver report",
    defaultPath: "driver-report.csv",
    filters: [
      { name: "Spreadsheet (CSV)", extensions: ["csv"] },
      { name: "JSON", extensions: ["json"] },
    ],
  });
  if (!path) return null;
  await invoke("export_report", { path });
  return path;
}

/** Installs the given updates, reporting progress until all are done. */
export async function installUpdates(
  updateIds: string[],
  onEvent: (event: InstallEvent) => void,
): Promise<void> {
  if (!inApp) return mockInstall(updateIds, onEvent);
  const stop = await listen<InstallEvent>("install-progress", (e) => onEvent(e.payload));
  try {
    await invoke("install_updates", { updateIds });
  } finally {
    stop();
  }
}

async function mockInstall(ids: string[], onEvent: (event: InstallEvent) => void) {
  onEvent(await later({ kind: "status", message: "Contacting Windows Update…" }, 300));
  onEvent(await later({ kind: "status", message: "Restore point created." }, 600));
  for (const [i, updateId] of ids.entries()) {
    onEvent(await later({ kind: "update", updateId, stage: "downloading", detail: "" }, 300));
    onEvent(await later({ kind: "update", updateId, stage: "installing", detail: "" }, 700));
    const failed = i === 1;
    onEvent(
      await later(
        {
          kind: "update",
          updateId,
          stage: failed ? "failed" : "installed",
          detail: failed ? "Windows Update reported error 0x80240022" : "",
        },
        700,
      ),
    );
  }
  const failed = ids.length > 1 ? 1 : 0;
  onEvent({ kind: "finished", installed: ids.length - failed, failed, rebootRequired: true });
}
