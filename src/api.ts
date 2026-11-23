import { getVersion } from "@tauri-apps/api/app";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { open, save } from "@tauri-apps/plugin-dialog";
import {
  mockApps,
  mockDisk,
  mockDuplicates,
  mockJunk,
  mockReport,
  mockScan,
  mockSoftware,
  mockStartup,
} from "./mock";
import type {
  CleanCategory,
  CleanOutcome,
  DiskReport,
  DuplicateReport,
  InstallEvent,
  InstalledApp,
  RemovalResult,
  ScanResult,
  SoftwareUpdate,
  StartupItem,
  UpdateReport,
} from "./types";

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

// --- Cleaner, startup, apps, disk -----------------------------------------

export const cleanerScan = (): Promise<CleanCategory[]> =>
  inApp ? invoke("cleaner_scan") : later(mockJunk, 700);

export const cleanerClean = (ids: string[]): Promise<CleanOutcome> =>
  inApp
    ? invoke("cleaner_clean", { ids })
    : later(
        {
          freedBytes: mockJunk.filter((c) => ids.includes(c.id)).reduce((n, c) => n + c.sizeBytes, 0),
          deleted: mockJunk.filter((c) => ids.includes(c.id)).reduce((n, c) => n + c.itemCount, 0),
          skipped: 14,
          note: null,
        },
        1200,
      );

export const startupList = (): Promise<StartupItem[]> =>
  inApp ? invoke("startup_list") : later(mockStartup, 300);

export const startupSetEnabled = (id: string, enabled: boolean): Promise<void> =>
  inApp ? invoke("startup_set_enabled", { id, enabled }) : later(undefined, 150);

export const appsList = (): Promise<InstalledApp[]> =>
  inApp ? invoke("apps_list") : later(mockApps, 300);

export const appsUninstall = (id: string): Promise<void> =>
  inApp ? invoke("apps_uninstall", { id }) : later(undefined, 300);

/** Analyses the given folder, or the user's profile folder when omitted. */
export const diskScan = (path?: string): Promise<DiskReport> =>
  inApp ? invoke("disk_scan", { path: path ?? null }) : later(mockDisk, 900);

export const revealInExplorer = (path: string): Promise<void> =>
  inApp ? invoke("reveal_in_explorer", { path }) : later(undefined, 0);

/** Returns the chosen folder, or null if the user cancelled. */
export async function pickFolder(): Promise<string | null> {
  if (!inApp) return null;
  const picked = await open({ directory: true, title: "Choose a folder" });
  return typeof picked === "string" ? picked : null;
}

// --- App updates and duplicates --------------------------------------------

export const softwareList = (): Promise<SoftwareUpdate[]> =>
  inApp ? invoke("software_list") : later(mockSoftware, 1200);

/** Resolves when the app's installer has finished; this can take minutes. */
export const softwareUpgrade = (id: string): Promise<void> =>
  inApp
    ? invoke("software_upgrade", { id })
    : id === "Zoom.Zoom"
      ? new Promise((_, reject) => setTimeout(() => reject("Installer failed with exit code: 1603"), 900))
      : later(undefined, 900);

/** Searches the given folder, or the user's profile folder when omitted. */
export const duplicatesFind = (path?: string): Promise<DuplicateReport> =>
  inApp ? invoke("duplicates_find", { path: path ?? null }) : later(mockDuplicates, 1500);

export const duplicatesCancel = (): Promise<void> =>
  inApp ? invoke("duplicates_cancel") : later(undefined, 0);

/** Moves the given files to the Recycle Bin. */
export const duplicatesRemove = (paths: string[]): Promise<RemovalResult> =>
  inApp
    ? invoke("duplicates_remove", { paths })
    : later(
        {
          removed: paths.length,
          freedBytes: mockDuplicates.groups.reduce(
            (n, g) => n + g.sizeBytes * g.files.filter((f) => paths.includes(f.path)).length,
            0,
          ),
          failed: 0,
        },
        600,
      );
