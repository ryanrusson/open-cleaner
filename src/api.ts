import { getVersion } from "@tauri-apps/api/app";
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

const LATEST_RELEASE =
  "https://api.github.com/repos/ryanrusson/windows-driver-updater/releases/latest";

export interface AppUpdate {
  version: string;
  url: string;
}

/** Numeric compare of dotted versions: is `a` newer than `b`? */
export function isNewer(a: string, b: string): boolean {
  const parts = (v: string) => v.replace(/^v/, "").split(".").map((n) => parseInt(n, 10) || 0);
  const [x, y] = [parts(a), parts(b)];
  for (let i = 0; i < Math.max(x.length, y.length); i++) {
    if ((x[i] ?? 0) !== (y[i] ?? 0)) return (x[i] ?? 0) > (y[i] ?? 0);
  }
  return false;
}

/**
 * Looks for a newer release of this app on GitHub. Resolves to null when up
 * to date, offline, or anything else goes wrong: this must never get in the
 * way of scanning.
 */
export async function checkForAppUpdate(): Promise<AppUpdate | null> {
  if (!inApp) return null;
  try {
    const [current, response] = await Promise.all([
      getVersion(),
      fetch(LATEST_RELEASE, { headers: { Accept: "application/vnd.github+json" } }),
    ]);
    if (!response.ok) return null;
    const release: { tag_name?: string; html_url?: string } = await response.json();
    if (!release.tag_name || !release.html_url?.startsWith("https://github.com/")) return null;
    return isNewer(release.tag_name, current)
      ? { version: release.tag_name.replace(/^v/, ""), url: release.html_url }
      : null;
  } catch {
    return null;
  }
}
