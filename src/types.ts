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
