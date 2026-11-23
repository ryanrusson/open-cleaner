// Mirrors crates/driver-core/src/models.rs (serialised as camelCase).

export interface SystemInfo {
  manufacturer: string;
  model: string;
  os: string;
}

export interface VendorHint {
  vendor: string;
  url: string;
  reason: string;
}

export interface Problem {
  code: number;
  description: string;
}

export interface Driver {
  deviceId: string;
  deviceName: string;
  deviceClass: string;
  manufacturer: string;
  provider: string;
  version: string;
  date: string | null;
  ageDays: number | null;
  hardwareId: string;
  infName: string;
  isSigned: boolean;
  signer: string;
  isInbox: boolean;
  problem: Problem | null;
  vendorHint: VendorHint | null;
}

export interface ScanResult {
  system: SystemInfo;
  drivers: Driver[];
  scannedOn: string;
}

export interface AvailableUpdate {
  updateId: string;
  title: string;
  description: string;
  driverModel: string;
  hardwareId: string;
  provider: string;
  manufacturer: string;
  driverClass: string;
  date: string | null;
  sizeBytes: number;
  supportUrl: string;
}

export interface UpdateReport {
  matched: { deviceId: string; update: AvailableUpdate }[];
  unmatched: AvailableUpdate[];
}

export type Status = "problem" | "update" | "check" | "ok";

export type InstallStage = "downloading" | "installing" | "installed" | "failed";

export type InstallEvent =
  | { kind: "status"; message: string }
  | { kind: "update"; updateId: string; stage: InstallStage; detail: string }
  | { kind: "finished"; installed: number; failed: number; rebootRequired: boolean }
  | { kind: "error"; message: string };

// Mirrors crates/system-tools.

export interface CleanCategory {
  id: string;
  label: string;
  description: string;
  sizeBytes: number;
  itemCount: number;
  recommended: boolean;
  needsAdmin: boolean;
  sizeKnown: boolean;
}

export interface CleanResult {
  freedBytes: number;
  deleted: number;
  skipped: number;
}

export interface StartupItem {
  id: string;
  name: string;
  command: string;
  source: string;
  enabled: boolean;
  canToggle: boolean;
}

export interface InstalledApp {
  id: string;
  name: string;
  version: string;
  publisher: string;
  installDate: string | null;
  sizeBytes: number | null;
  canUninstall: boolean;
}

export interface FolderSize {
  name: string;
  path: string;
  sizeBytes: number;
  fileCount: number;
}

export interface FileEntry {
  path: string;
  sizeBytes: number;
  modified: number | null;
}

export interface DiskReport {
  root: string;
  totalBytes: number;
  fileCount: number;
  folders: FolderSize[];
  largestFiles: FileEntry[];
  unreadable: number;
}

export interface CleanOutcome extends CleanResult {
  /** Set when the system-wide part could not be done. */
  note: string | null;
}

export interface SoftwareUpdate {
  id: string;
  name: string;
  currentVersion: string;
  availableVersion: string;
  source: string;
}

export interface DuplicateFile {
  path: string;
  modified: number | null;
}

export interface DuplicateGroup {
  sizeBytes: number;
  files: DuplicateFile[];
}

export interface DuplicateReport {
  root: string;
  groups: DuplicateGroup[];
  wastedBytes: number;
  filesChecked: number;
}

export interface RemovalResult {
  removed: number;
  freedBytes: number;
  failed: number;
}
