// Sample data for working on the UI in a browser, without Windows.
import type {
  AvailableUpdate,
  CleanCategory,
  DiskReport,
  Driver,
  DuplicateReport,
  InstalledApp,
  ScanResult,
  SoftwareUpdate,
  StartupItem,
  UpdateReport,
} from "./types";

const driver = (
  deviceName: string,
  deviceClass: string,
  provider: string,
  version: string,
  date: string,
  extra: Partial<Driver> = {},
): Driver => ({
  deviceId: `PCI\\${deviceName.replace(/\W+/g, "_").toUpperCase()}`,
  deviceName,
  deviceClass,
  manufacturer: provider,
  provider,
  version,
  date,
  ageDays: Math.round((Date.parse("2026-11-22") - Date.parse(date)) / 86_400_000),
  hardwareId: `PCI\\VEN_${provider.slice(0, 4).toUpperCase()}&DEV_${version.replace(/\D/g, "").slice(0, 4)}`,
  infName: `oem${version.length}.inf`,
  isSigned: true,
  signer: "Microsoft Windows Hardware Compatibility Publisher",
  isInbox: provider === "Microsoft",
  problem: null,
  vendorHint: null,
  ...extra,
});

const drivers: Driver[] = [
  driver("NVIDIA GeForce RTX 4070", "Display adapters", "NVIDIA", "32.0.15.6094", "2024-08-14", {
    vendorHint: {
      vendor: "NVIDIA",
      url: "https://www.nvidia.com/en-us/drivers/",
      reason: "This driver is 2.3 years old. Windows Update has nothing newer, but NVIDIA may.",
    },
  }),
  driver("Intel(R) Wi-Fi 6E AX211 160MHz", "Network adapters", "Intel", "22.250.1.2", "2023-07-30"),
  driver("Intel(R) Ethernet Controller I226-V", "Network adapters", "Intel", "2.1.4.3", "2025-09-02"),
  driver("Realtek(R) Audio", "Sound, video and game controllers", "Realtek Semiconductor Corp.", "6.0.9549.1", "2023-06-27", {
    vendorHint: {
      vendor: "Dell",
      url: "https://www.dell.com/support/home",
      reason: "This driver is 3.4 years old. Windows Update has nothing newer, but Dell may.",
    },
  }),
  driver("Intel(R) Wireless Bluetooth(R)", "Bluetooth", "Intel", "23.60.0.1", "2024-05-12"),
  driver("Intel(R) Serial IO I2C Host Controller", "System devices", "Intel", "30.100.2405.44", "2025-10-21"),
  driver("Intel(R) Management Engine Interface #1", "System devices", "Intel", "2345.5.42.0", "2025-11-09"),
  driver("Samsung NVMe Controller", "Storage controllers", "Samsung Electronics", "3.3.0.2003", "2026-01-21"),
  driver("Goodix Fingerprint Sensor", "Biometric devices", "Goodix", "3.12804.0.170", "2022-03-08", {
    isSigned: false,
    signer: "",
  }),
  driver("PCI Encryption/Decryption Controller", "Other devices", "", "", "2026-11-22", {
    date: null,
    ageDays: null,
    isSigned: false,
    signer: "",
    infName: "",
    problem: { code: 28, description: "No driver is installed for this device" },
  }),
  driver("Logitech MX Master 3S", "Mice and pointing devices", "Logitech", "1.10.78.0", "2025-04-17"),
  driver("Dell U2723QE", "Monitors", "Dell Inc.", "1.0.0.0", "2022-05-10"),
  driver("USB xHCI Compliant Host Controller", "USB controllers", "Microsoft", "10.0.26100.1150", "2025-06-21"),
  driver("HID Keyboard Device", "Keyboards", "Microsoft", "10.0.26100.1", "2006-06-21"),
  driver("Microsoft Basic Display Adapter", "Display adapters", "Microsoft", "10.0.26100.1", "2006-06-21"),
];

const update = (d: Driver, title: string, date: string, sizeMb: number): AvailableUpdate => ({
  updateId: crypto.randomUUID(),
  title,
  description: `${d.provider} driver update for ${d.deviceName}`,
  driverModel: d.deviceName,
  hardwareId: d.hardwareId,
  provider: d.provider,
  manufacturer: d.manufacturer,
  driverClass: d.deviceClass,
  date,
  sizeBytes: sizeMb * 1024 * 1024,
  supportUrl: "",
});

export const mockScan: ScanResult = {
  system: { manufacturer: "Dell Inc.", model: "XPS 8960", os: "Microsoft Windows 11 Pro (10.0.26100)" },
  drivers,
  scannedOn: "2026-11-22",
};

export const mockReport: UpdateReport = {
  matched: [
    { deviceId: drivers[1].deviceId, update: update(drivers[1], "Intel - Net - 23.110.0.5", "2026-08-19", 48) },
    { deviceId: drivers[4].deviceId, update: update(drivers[4], "Intel Corporation - Bluetooth - 23.110.0.2", "2026-08-11", 21) },
  ],
  unmatched: [
    update(drivers[11], "Dell Inc. - Monitor - 1.0.0.0", "2021-11-02", 1),
  ],
};

const MB = 1024 * 1024;
const GB = 1024 * MB;
const cacheNote = "Cached web pages and images. Sign-ins, history and passwords are kept.";

export const mockJunk: CleanCategory[] = [
  { id: "temp", label: "Temporary files", description: "Leftovers from apps and installers, untouched for at least a day.", sizeBytes: 1.8 * GB, itemCount: 4210, recommended: true, needsAdmin: false, sizeKnown: true },
  { id: "chrome-cache", label: "Chrome cache", description: cacheNote, sizeBytes: 912 * MB, itemCount: 8804, recommended: true, needsAdmin: false, sizeKnown: true },
  { id: "edge-cache", label: "Edge cache", description: cacheNote, sizeBytes: 431 * MB, itemCount: 3920, recommended: true, needsAdmin: false, sizeKnown: true },
  { id: "crash-dumps", label: "Crash dumps and error reports", description: "Diagnostic files saved when apps crashed.", sizeBytes: 268 * MB, itemCount: 37, recommended: true, needsAdmin: false, sizeKnown: true },
  { id: "thumbnails", label: "Thumbnail cache", description: "Preview images for folders. Windows rebuilds them as needed.", sizeBytes: 96 * MB, itemCount: 12, recommended: false, needsAdmin: false, sizeKnown: true },
  { id: "windows-temp", label: "Windows temporary files", description: "Leftovers from Windows and installers, untouched for at least a day.", sizeBytes: 0, itemCount: 0, recommended: false, needsAdmin: true, sizeKnown: false },
  { id: "update-cache", label: "Windows Update downloads", description: "Already-installed update files. Windows downloads again anything it still needs.", sizeBytes: 2.1 * GB, itemCount: 318, recommended: false, needsAdmin: true, sizeKnown: true },
  { id: "recycle-bin", label: "Recycle Bin", description: "Files you already deleted. Emptying it is permanent.", sizeBytes: 3.4 * GB, itemCount: 152, recommended: false, needsAdmin: false, sizeKnown: true },
];

const startup = (name: string, command: string, enabled: boolean, allUsers = false): StartupItem => ({
  id: `${allUsers ? "machine-run" : "user-run"}|${name}`,
  name,
  command,
  source: allUsers ? "Registry (all users)" : "Registry (your account)",
  enabled,
  canToggle: !allUsers,
});

export const mockStartup: StartupItem[] = [
  startup("Discord", '"C:\\Users\\you\\AppData\\Local\\Discord\\Update.exe" --processStart Discord.exe', true),
  startup("Microsoft Teams", '"C:\\Users\\you\\AppData\\Local\\Microsoft\\Teams\\Update.exe" --processStart Teams.exe', false),
  startup("OneDrive", '"C:\\Program Files\\Microsoft OneDrive\\OneDrive.exe" /background', true),
  startup("RtkAudUService", '"C:\\Windows\\System32\\RtkAudUService64.exe" -background', true, true),
  startup("SecurityHealth", "%windir%\\system32\\SecurityHealthSystray.exe", true, true),
  startup("Spotify", '"C:\\Users\\you\\AppData\\Roaming\\Spotify\\Spotify.exe" --autostart --minimized', true),
  startup("Steam", '"C:\\Program Files (x86)\\Steam\\steam.exe" -silent', true),
];

const app = (name: string, version: string, publisher: string, installDate: string | null, sizeMb: number | null): InstalledApp => ({
  id: `machine|${name}`,
  name,
  version,
  publisher,
  installDate,
  sizeBytes: sizeMb === null ? null : sizeMb * MB,
  canUninstall: true,
});

export const mockApps: InstalledApp[] = [
  app("7-Zip 24.08 (x64)", "24.08", "Igor Pavlov", "2025-02-11", 5.6),
  app("Adobe Acrobat Reader", "24.003.20180", "Adobe", "2025-09-30", 612),
  app("Blender", "4.2.1", "Blender Foundation", "2024-11-02", 1150),
  app("Discord", "1.0.9163", "Discord Inc.", "2026-10-04", 412),
  app("Git", "2.47.0", "The Git Development Community", "2026-01-19", 368),
  app("Google Chrome", "131.0.6778.86", "Google LLC", "2026-11-18", null),
  app("Microsoft Visual Studio Code", "1.95.3", "Microsoft Corporation", "2026-11-14", 398),
  app("NVIDIA Graphics Driver 560.94", "560.94", "NVIDIA Corporation", "2024-08-20", 1890),
  app("Spotify", "1.2.50.335", "Spotify AB", "2026-10-28", 324),
  app("Steam", "2.10.91.91", "Valve Corporation", "2023-05-06", 1210),
  app("VLC media player", "3.0.21", "VideoLAN", null, 172),
  app("Zoom Workplace", "6.2.7", "Zoom Video Communications, Inc.", "2026-09-12", 286),
];

const folder = (name: string, sizeGb: number, fileCount: number) => ({
  name,
  path: `C:\\Users\\you\\${name}`,
  sizeBytes: sizeGb * GB,
  fileCount,
});

const file = (path: string, sizeGb: number, year: number) => ({
  path: `C:\\Users\\you\\${path}`,
  sizeBytes: sizeGb * GB,
  modified: Date.UTC(year, 5, 14) / 1000,
});

export const mockDisk: DiskReport = {
  root: "C:\\Users\\you",
  totalBytes: 187.3 * GB,
  fileCount: 412_877,
  folders: [
    folder("AppData", 61.2, 301_442),
    folder("Videos", 48.9, 312),
    folder("Downloads", 33.5, 1_904),
    folder("Documents", 21.4, 18_220),
    folder("Pictures", 17.8, 44_310),
    folder("Desktop", 3.1, 211),
    folder("Music", 1.4, 46_478),
  ],
  largestFiles: [
    file("Videos\\Captures\\raid-night-full.mkv", 14.2, 2025),
    file("Downloads\\Win11_24H2_English_x64.iso", 5.4, 2024),
    file("Videos\\wedding-export-final.mp4", 4.9, 2023),
    file("AppData\\Local\\Docker\\wsl\\disk\\docker_data.vhdx", 4.6, 2026),
    file("Downloads\\ubuntu-24.04-desktop-amd64.iso", 4.1, 2024),
    file("Documents\\Backups\\phone-backup-2022.zip", 3.3, 2022),
    file("Videos\\Captures\\clip-0042.mp4", 2.7, 2026),
    file("Downloads\\blender-4.2.1-windows-x64.msi", 1.1, 2024),
  ],
  unreadable: 3,
};

export const mockSoftware: SoftwareUpdate[] = [
  { id: "Git.Git", name: "Git", currentVersion: "2.45.1", availableVersion: "2.47.0", source: "winget" },
  { id: "Microsoft.VisualStudioCode", name: "Microsoft Visual Studio Code", currentVersion: "1.94.2", availableVersion: "1.95.3", source: "winget" },
  { id: "VideoLAN.VLC", name: "VLC media player", currentVersion: "3.0.20", availableVersion: "3.0.21", source: "winget" },
  { id: "Zoom.Zoom", name: "Zoom Workplace", currentVersion: "6.1.0", availableVersion: "6.2.7", source: "winget" },
];

const copy = (path: string, year: number) => ({
  path: `C:\\Users\\you\\${path}`,
  modified: Date.UTC(year, 2, 9) / 1000,
});

export const mockDuplicates: DuplicateReport = {
  root: "C:\\Users\\you",
  filesChecked: 18_442,
  wastedBytes: 4.9 * GB + 2 * 1.1 * GB + 310 * MB,
  groups: [
    { sizeBytes: 4.9 * GB, files: [copy("Videos\\wedding-export-final.mp4", 2023), copy("Desktop\\wedding-export-final (1).mp4", 2024)] },
    { sizeBytes: 1.1 * GB, files: [copy("Downloads\\blender-4.2.1-windows-x64.msi", 2024), copy("Downloads\\blender-4.2.1-windows-x64 (1).msi", 2024), copy("Documents\\Installers\\blender.msi", 2025)] },
    { sizeBytes: 310 * MB, files: [copy("Pictures\\2022\\trip.zip", 2022), copy("Documents\\Backups\\trip.zip", 2023)] },
  ],
};
