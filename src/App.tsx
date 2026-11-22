import { useCallback, useEffect, useMemo, useState } from "react";
import {
  type AppUpdate,
  WINDOWS_UPDATE_SETTINGS,
  checkForAppUpdate,
  checkUpdates,
  exportReport,
  inApp,
  installUpdates,
  openLink,
  scanDrivers,
} from "./api";
import type {
  AvailableUpdate,
  Driver,
  InstallEvent,
  InstallStage,
  ScanResult,
  Status,
  UpdateReport,
} from "./types";

type Phase =
  | { step: "scanning" }
  | { step: "failed"; message: string }
  | { step: "ready"; scan: ScanResult };

type UpdateCheck =
  | { step: "checking" }
  | { step: "failed"; message: string }
  | { step: "done"; report: UpdateReport };

type Filter = Status | "all";

interface InstallItem {
  update: AvailableUpdate;
  deviceName: string;
  stage: InstallStage | "waiting";
  detail: string;
}

interface InstallRun {
  step: "confirm" | "running" | "done" | "failed";
  items: InstallItem[];
  message: string;
  rebootRequired: boolean;
}

const STAGE_LABEL: Record<InstallItem["stage"], string> = {
  waiting: "Waiting",
  downloading: "Downloading…",
  installing: "Installing…",
  installed: "Installed",
  failed: "Failed",
};

const STATUS: Record<Status, { label: string; order: number }> = {
  problem: { label: "Problem", order: 0 },
  update: { label: "Update available", order: 1 },
  check: { label: "Worth checking", order: 2 },
  ok: { label: "Up to date", order: 3 },
};

const errorText = (e: unknown) => (e instanceof Error ? e.message : String(e));

function formatAge(days: number | null): string {
  if (days === null) return "";
  if (days < 31) return "this month";
  if (days < 365) return `${Math.round(days / 30)} mo ago`;
  return `${(days / 365.25).toFixed(1)} yr ago`;
}

function formatSize(bytes: number): string {
  if (!bytes) return "";
  const mb = bytes / (1024 * 1024);
  return mb >= 1 ? `${mb.toFixed(mb >= 10 ? 0 : 1)} MB` : `${Math.max(1, Math.round(bytes / 1024))} KB`;
}

export default function App() {
  const [phase, setPhase] = useState<Phase>({ step: "scanning" });
  const [check, setCheck] = useState<UpdateCheck>({ step: "checking" });
  const [filter, setFilter] = useState<Filter>("all");
  const [query, setQuery] = useState("");
  const [showInbox, setShowInbox] = useState(false);
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [toast, setToast] = useState<string | null>(null);
  const [install, setInstall] = useState<InstallRun | null>(null);
  const [appUpdate, setAppUpdate] = useState<AppUpdate | null>(null);

  useEffect(() => {
    void checkForAppUpdate().then(setAppUpdate);
  }, []);

  const runUpdateCheck = useCallback(async () => {
    setCheck({ step: "checking" });
    try {
      setCheck({ step: "done", report: await checkUpdates() });
    } catch (e) {
      setCheck({ step: "failed", message: errorText(e) });
    }
  }, []);

  const runScan = useCallback(async () => {
    setPhase({ step: "scanning" });
    setSelectedId(null);
    try {
      setPhase({ step: "ready", scan: await scanDrivers() });
    } catch (e) {
      setPhase({ step: "failed", message: errorText(e) });
      return;
    }
    await runUpdateCheck();
  }, [runUpdateCheck]);

  useEffect(() => {
    void runScan();
  }, [runScan]);

  useEffect(() => {
    if (!toast) return;
    const timer = setTimeout(() => setToast(null), 4000);
    return () => clearTimeout(timer);
  }, [toast]);

  const scan = phase.step === "ready" ? phase.scan : null;
  const report = check.step === "done" ? check.report : null;

  const updates = useMemo(
    () => new Map(report?.matched.map((m) => [m.deviceId, m.update])),
    [report],
  );

  const statusOf = useCallback(
    (d: Driver): Status =>
      d.problem ? "problem" : updates.has(d.deviceId) ? "update" : d.vendorHint ? "check" : "ok",
    [updates],
  );

  // Windows' own built-in drivers are noise unless something is wrong with them.
  const relevant = useMemo(
    () => (scan?.drivers ?? []).filter((d) => showInbox || !d.isInbox || statusOf(d) !== "ok"),
    [scan, showInbox, statusOf],
  );

  const counts = useMemo(() => {
    const c: Record<Status, number> = { problem: 0, update: 0, check: 0, ok: 0 };
    for (const d of relevant) c[statusOf(d)]++;
    return c;
  }, [relevant, statusOf]);

  const rows = useMemo(() => {
    const q = query.trim().toLowerCase();
    return relevant
      .filter((d) => filter === "all" || statusOf(d) === filter)
      .filter(
        (d) =>
          !q ||
          [d.deviceName, d.deviceClass, d.provider, d.version, d.hardwareId]
            .join("\n")
            .toLowerCase()
            .includes(q),
      )
      .sort(
        (a, b) =>
          STATUS[statusOf(a)].order - STATUS[statusOf(b)].order ||
          a.deviceName.localeCompare(b.deviceName),
      );
  }, [relevant, filter, query, statusOf]);

  const selected = scan?.drivers.find((d) => d.deviceId === selectedId) ?? null;

  /** Open the confirmation dialog for one device's update, or for all of them. */
  const reviewInstall = (deviceId?: string) => {
    const items = (report?.matched ?? [])
      .filter((m) => !deviceId || m.deviceId === deviceId)
      // One update can serve several identical devices; install it once.
      .filter((m, i, all) => all.findIndex((o) => o.update.updateId === m.update.updateId) === i)
      .map((m) => ({
        update: m.update,
        deviceName: scan?.drivers.find((d) => d.deviceId === m.deviceId)?.deviceName ?? "",
        stage: "waiting" as const,
        detail: "",
      }));
    if (items.length) setInstall({ step: "confirm", items, message: "", rebootRequired: false });
  };

  const startInstall = async () => {
    if (!install) return;
    setInstall({ ...install, step: "running", message: "Waiting for administrator permission…" });
    const onEvent = (event: InstallEvent) =>
      setInstall((run) => {
        if (!run) return run;
        switch (event.kind) {
          case "status":
            return { ...run, message: event.message };
          case "update":
            return {
              ...run,
              message: "",
              items: run.items.map((item) =>
                item.update.updateId === event.updateId
                  ? { ...item, stage: event.stage, detail: event.detail }
                  : item,
              ),
            };
          case "finished":
            return { ...run, step: "done", message: "", rebootRequired: event.rebootRequired };
          case "error":
            return { ...run, step: "failed", message: event.message };
        }
      });
    try {
      await installUpdates(
        install.items.map((item) => item.update.updateId),
        onEvent,
      );
    } catch (e) {
      setInstall((run) => run && { ...run, step: "failed", message: errorText(e) });
    }
  };

  const closeInstall = () => {
    const ran = install?.step === "done" || install?.items.some((i) => i.stage !== "waiting");
    setInstall(null);
    // Anything may have changed; show the truth rather than stale results.
    if (ran) void runScan();
  };

  const onExport = async () => {
    try {
      const path = await exportReport();
      if (path) setToast(`Saved ${path}`);
      else if (!inApp) setToast("Export is available in the desktop app.");
    } catch (e) {
      setToast(errorText(e));
    }
  };

  return (
    <div className="app">
      <header className="topbar">
        <div className="brand">
          <Logo />
          <div>
            <h1>Driver Updater</h1>
            <p className="muted">
              {scan
                ? [scan.system.manufacturer, scan.system.model].filter(Boolean).join(" ") +
                  (scan.system.os ? ` · ${scan.system.os}` : "")
                : "Checking the drivers installed on this PC"}
            </p>
          </div>
        </div>
        <div className="actions">
          <button className="btn" onClick={onExport} disabled={!scan}>
            Export report
          </button>
          <button
            className="btn primary"
            onClick={() => void runScan()}
            disabled={phase.step === "scanning" || (scan !== null && check.step === "checking")}
          >
            Scan again
          </button>
        </div>
      </header>

      {appUpdate && (
        <div className="app-update">
          <span>Driver Updater {appUpdate.version} is available.</span>
          <button className="btn small" onClick={() => void openLink(appUpdate.url)}>
            Download
          </button>
          <button className="icon-btn" onClick={() => setAppUpdate(null)} aria-label="Dismiss">
            ✕
          </button>
        </div>
      )}

      {phase.step === "scanning" && (
        <div className="center">
          <span className="spinner large" />
          <p>Reading installed drivers…</p>
        </div>
      )}

      {phase.step === "failed" && (
        <div className="center">
          <h2>The scan could not run</h2>
          <p className="muted">{phase.message}</p>
          <button className="btn primary" onClick={() => void runScan()}>
            Try again
          </button>
        </div>
      )}

      {scan && (
        <main className={selected ? "content with-panel" : "content"}>
          <section className="list">
            <UpdateBanner
              check={check}
              onRetry={() => void runUpdateCheck()}
              onInstall={() => reviewInstall()}
            />

            <div className="stats">
              {(["update", "check", "problem", "ok"] as const).map((s) => (
                <button
                  key={s}
                  className={`stat ${s} ${filter === s ? "active" : ""}`}
                  onClick={() => setFilter(filter === s ? "all" : s)}
                  aria-pressed={filter === s}
                >
                  <span className="stat-value">
                    {s === "update" && check.step !== "done" ? "–" : counts[s]}
                  </span>
                  <span className="stat-label">{STATUS[s].label}</span>
                </button>
              ))}
            </div>

            <div className="toolbar">
              <input
                type="search"
                placeholder="Search devices, providers, hardware IDs"
                value={query}
                onChange={(e) => setQuery(e.target.value)}
              />
              <label className="toggle">
                <input
                  type="checkbox"
                  checked={showInbox}
                  onChange={(e) => setShowInbox(e.target.checked)}
                />
                Show drivers built into Windows
              </label>
            </div>

            <div className="table-wrap">
              <table>
                <thead>
                  <tr>
                    <th>Device</th>
                    <th>Provider</th>
                    <th>Installed version</th>
                    <th>Driver date</th>
                    <th>Status</th>
                  </tr>
                </thead>
                <tbody>
                  {rows.map((d) => {
                    const status = statusOf(d);
                    return (
                      <tr
                        key={d.deviceId}
                        className={d.deviceId === selectedId ? "selected" : ""}
                        onClick={() => setSelectedId(d.deviceId === selectedId ? null : d.deviceId)}
                        tabIndex={0}
                        onKeyDown={(e) => e.key === "Enter" && setSelectedId(d.deviceId)}
                      >
                        <td>
                          <div className="device">{d.deviceName}</div>
                          <div className="muted small">{d.deviceClass}</div>
                        </td>
                        <td>{d.provider || "–"}</td>
                        <td className="mono">{d.version || "–"}</td>
                        <td className="nowrap">
                          {d.date ?? "–"}
                          <div className="muted small">{formatAge(d.ageDays)}</div>
                        </td>
                        <td>
                          <span className={`pill ${status}`}>{STATUS[status].label}</span>
                        </td>
                      </tr>
                    );
                  })}
                </tbody>
              </table>
              {rows.length === 0 && (
                <p className="empty muted">
                  {filter === "update" && check.step === "checking"
                    ? "Still checking Windows Update…"
                    : "No drivers match."}
                </p>
              )}
            </div>

            {report && report.unmatched.length > 0 && <OtherUpdates updates={report.unmatched} />}
          </section>

          {selected && (
            <DetailPanel
              driver={selected}
              status={statusOf(selected)}
              update={updates.get(selected.deviceId)}
              onInstall={() => reviewInstall(selected.deviceId)}
              onClose={() => setSelectedId(null)}
            />
          )}
        </main>
      )}

      {install && <InstallDialog run={install} onStart={() => void startInstall()} onClose={closeInstall} />}

      {toast && <div className="toast">{toast}</div>}
    </div>
  );
}

function UpdateBanner({
  check,
  onRetry,
  onInstall,
}: {
  check: UpdateCheck;
  onRetry: () => void;
  onInstall: () => void;
}) {
  if (check.step === "checking") {
    return (
      <div className="banner">
        <span className="spinner" />
        <span>
          Checking Windows Update for newer drivers. <span className="muted">This can take a minute.</span>
        </span>
      </div>
    );
  }
  if (check.step === "failed") {
    return (
      <div className="banner error">
        <span>{check.message}</span>
        <button className="btn small" onClick={onRetry}>
          Retry
        </button>
      </div>
    );
  }
  const count = check.report.matched.length;
  if (count === 0) {
    return (
      <div className="banner">
        <span>Windows Update has no newer drivers for this PC.</span>
      </div>
    );
  }
  return (
    <div className="banner good">
      <span>
        Windows Update has {count} newer driver{count === 1 ? "" : "s"} for this PC.
      </span>
      <button className="btn small primary" onClick={onInstall}>
        Review and install
      </button>
    </div>
  );
}

function DetailPanel({
  driver: d,
  status,
  update,
  onInstall,
  onClose,
}: {
  driver: Driver;
  status: Status;
  update: AvailableUpdate | undefined;
  onInstall: () => void;
  onClose: () => void;
}) {
  const facts: [string, string][] = [
    ["Class", d.deviceClass],
    ["Provider", d.provider],
    ["Manufacturer", d.manufacturer],
    ["Version", d.version],
    ["Driver date", d.date ? `${d.date} (${formatAge(d.ageDays)})` : ""],
    ["Signed", d.version ? (d.isSigned ? d.signer || "Yes" : "No") : ""],
    ["INF file", d.infName],
    ["Hardware ID", d.hardwareId],
  ];
  return (
    <aside className="panel">
      <div className="panel-head">
        <div>
          <span className={`pill ${status}`}>{STATUS[status].label}</span>
          <h2>{d.deviceName}</h2>
        </div>
        <button className="icon-btn" onClick={onClose} aria-label="Close details">
          ✕
        </button>
      </div>

      {d.problem && (
        <div className="callout problem">
          <strong>{d.problem.description}</strong>
          <p>
            Device Manager code {d.problem.code}.{" "}
            {update
              ? "The update below may fix it."
              : "Check Windows Update or the manufacturer's support page for a driver."}
          </p>
        </div>
      )}

      {update && (
        <div className="callout update">
          <strong>{update.title}</strong>
          <p>
            {[update.date && `Dated ${update.date}`, formatSize(update.sizeBytes)]
              .filter(Boolean)
              .join(" · ")}
          </p>
          <div className="row">
            <button className="btn small primary" onClick={onInstall}>
              Install this update
            </button>
            <button className="btn small" onClick={() => void openLink(WINDOWS_UPDATE_SETTINGS)}>
              Open Windows Update
            </button>
          </div>
        </div>
      )}

      {d.vendorHint && !update && (
        <div className="callout check">
          <p>{d.vendorHint.reason}</p>
          <button className="btn small" onClick={() => void openLink(d.vendorHint!.url)}>
            Open {d.vendorHint.vendor} downloads
          </button>
        </div>
      )}

      <dl>
        {facts
          .filter(([, value]) => value)
          .map(([label, value]) => (
            <div key={label}>
              <dt>{label}</dt>
              <dd className={label === "Hardware ID" || label === "Version" ? "mono" : ""}>{value}</dd>
            </div>
          ))}
      </dl>
    </aside>
  );
}

function InstallDialog({
  run,
  onStart,
  onClose,
}: {
  run: InstallRun;
  onStart: () => void;
  onClose: () => void;
}) {
  const count = run.items.length;
  const installed = run.items.filter((i) => i.stage === "installed").length;
  const failed = run.items.filter((i) => i.stage === "failed").length;
  const title =
    run.step === "confirm"
      ? `Install ${count} driver update${count === 1 ? "" : "s"}?`
      : run.step === "running"
        ? "Installing driver updates"
        : run.step === "failed"
          ? "Installation did not finish"
          : failed === 0
            ? "Driver updates installed"
            : installed === 0
              ? "No updates were installed"
              : `${installed} installed, ${failed} failed`;
  return (
    <div className="overlay">
      <div className="dialog" role="dialog" aria-modal="true" aria-label={title}>
        <h2>{title}</h2>

        {run.step === "confirm" && (
          <p className="muted">
            Windows will ask for administrator permission. Driver Updater then tries to create a
            restore point and installs these from Windows Update. Your screen, network or sound may
            cut out briefly, so save your work first.
          </p>
        )}

        <ul className="install-list">
          {run.items.map((item) => (
            <li key={item.update.updateId}>
              <div>
                <div className="device">{item.deviceName || item.update.driverModel}</div>
                <div className="muted small">
                  {[item.update.title, formatSize(item.update.sizeBytes)].filter(Boolean).join(" · ")}
                </div>
                {item.detail && <div className="small detail-error">{item.detail}</div>}
              </div>
              {run.step !== "confirm" && (
                <span className={`stage ${item.stage}`}>
                  {(item.stage === "downloading" || item.stage === "installing") && (
                    <span className="spinner" />
                  )}
                  {STAGE_LABEL[item.stage]}
                </span>
              )}
            </li>
          ))}
        </ul>

        {run.message && (
          <p className={run.step === "failed" ? "detail-error" : "muted"}>{run.message}</p>
        )}
        {run.step === "done" && run.rebootRequired && (
          <p className="callout check">Restart your PC to finish installing.</p>
        )}

        <div className="dialog-actions">
          {run.step === "confirm" && (
            <>
              <button className="btn" onClick={onClose}>
                Cancel
              </button>
              <button className="btn primary" onClick={onStart}>
                Install
              </button>
            </>
          )}
          {run.step === "running" && (
            <span className="muted small">Keep this window open until it finishes.</span>
          )}
          {(run.step === "done" || run.step === "failed") && (
            <button className="btn primary" onClick={onClose}>
              Done
            </button>
          )}
        </div>
      </div>
    </div>
  );
}

function OtherUpdates({ updates }: { updates: AvailableUpdate[] }) {
  return (
    <details className="other">
      <summary>
        Also listed by Windows Update ({updates.length})
        <span className="muted"> — not newer than what is installed, or for a device not in this scan</span>
      </summary>
      <ul>
        {updates.map((u) => (
          <li key={u.updateId}>
            <span>{u.title}</span>
            <span className="muted small">
              {[u.driverModel, u.date, formatSize(u.sizeBytes)].filter(Boolean).join(" · ")}
            </span>
          </li>
        ))}
      </ul>
    </details>
  );
}

function Logo() {
  return (
    <svg className="logo" viewBox="0 0 32 32" aria-hidden="true">
      <rect width="32" height="32" rx="8" fill="var(--accent)" />
      <path
        d="M16 7v11m0 0-4.5-4.5M16 18l4.5-4.5M9 22.5h14"
        fill="none"
        stroke="#fff"
        strokeWidth="2.4"
        strokeLinecap="round"
        strokeLinejoin="round"
      />
    </svg>
  );
}
