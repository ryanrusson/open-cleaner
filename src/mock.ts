// Sample data for working on the UI in a browser, without Windows.
import type { AvailableUpdate, Driver, ScanResult, UpdateReport } from "./types";

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
