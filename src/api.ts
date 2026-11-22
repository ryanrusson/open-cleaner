import { invoke } from "@tauri-apps/api/core";
import { save } from "@tauri-apps/plugin-dialog";
import { mockReport, mockScan } from "./mock";
import type { ScanResult, UpdateReport } from "./types";

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
