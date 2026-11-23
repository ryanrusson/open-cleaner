// Data shared between pages, so the Overview and each tool's own page show
// the same results and nothing is scanned twice.
import { useEffect, useSyncExternalStore } from "react";
import { appsList, checkUpdates, cleanerScan, scanDrivers, softwareList, startupList } from "./api";
import { errorText } from "./format";

export type Load<T> =
  | { step: "loading" }
  | { step: "failed"; message: string }
  | { step: "ready"; data: T };

export interface Resource<T> {
  get: () => Load<T>;
  subscribe: (listener: () => void) => () => void;
  /** Fetch again. Resolves to whether it succeeded. */
  reload: () => Promise<boolean>;
  /** Fetch only if nothing has been fetched yet. */
  ensure: () => void;
  update: (change: (state: Load<T>) => Load<T>) => void;
  reset: () => void;
}

function createResource<T>(fetcher: () => Promise<T>): Resource<T> {
  let state: Load<T> = { step: "loading" };
  let started = false;
  let latest = 0;
  const listeners = new Set<() => void>();
  const set = (next: Load<T>) => {
    state = next;
    listeners.forEach((listener) => listener());
  };
  const reload = async () => {
    started = true;
    const run = ++latest;
    set({ step: "loading" });
    try {
      const data = await fetcher();
      if (run === latest) set({ step: "ready", data });
      return true;
    } catch (e) {
      if (run === latest) set({ step: "failed", message: errorText(e) });
      return false;
    }
  };
  return {
    get: () => state,
    subscribe: (listener) => {
      listeners.add(listener);
      return () => listeners.delete(listener);
    },
    reload,
    ensure: () => {
      if (!started) void reload();
    },
    update: (change) => set(change(state)),
    reset: () => {
      latest++;
      set({ step: "loading" });
    },
  };
}

/** Subscribe to a resource without triggering a fetch. */
export function useWatch<T>(resource: Resource<T>): Load<T> {
  return useSyncExternalStore(resource.subscribe, resource.get);
}

/** Subscribe to a resource, fetching it the first time anything needs it. */
export function useResource<T>(resource: Resource<T>) {
  const state = useWatch(resource);
  useEffect(() => resource.ensure(), [resource]);
  return { state, reload: resource.reload, setState: resource.update };
}

export const junk = createResource(cleanerScan);
export const startup = createResource(startupList);
export const apps = createResource(appsList);
export const software = createResource(softwareList);
export const drivers = createResource(scanDrivers);
/** Windows Update's answer for the drivers above; only valid after a scan. */
export const driverUpdates = createResource(checkUpdates);

/** Scan drivers, then ask Windows Update about them. */
export async function rescanDrivers() {
  driverUpdates.reset();
  if (await drivers.reload()) await driverUpdates.reload();
}

let driversStarted = false;
export function ensureDrivers() {
  if (!driversStarted) {
    driversStarted = true;
    void rescanDrivers();
  }
}
