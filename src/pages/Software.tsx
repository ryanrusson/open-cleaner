import { useState } from "react";
import { softwareUpgrade } from "../api";
import { errorText } from "../format";
import { software, useResource } from "../store";
import type { SoftwareUpdate } from "../types";
import { Failed, Loading, PageHeader } from "../ui";

type Progress = { step: "queued" | "updating" | "done" } | { step: "failed"; message: string };

export default function SoftwarePage() {
  const { state, reload } = useResource(software);
  const [progress, setProgress] = useState<Record<string, Progress>>({});
  const [running, setRunning] = useState(false);

  const updates = state.step === "ready" ? state.data : [];
  const pending = updates.filter((u) => progress[u.id]?.step !== "done");

  // One installer at a time: two at once tend to block each other.
  const run = async (targets: SoftwareUpdate[]) => {
    setRunning(true);
    setProgress((p) => ({ ...p, ...Object.fromEntries(targets.map((u) => [u.id, { step: "queued" }])) }));
    for (const update of targets) {
      setProgress((p) => ({ ...p, [update.id]: { step: "updating" } }));
      try {
        await softwareUpgrade(update.id);
        setProgress((p) => ({ ...p, [update.id]: { step: "done" } }));
      } catch (e) {
        setProgress((p) => ({ ...p, [update.id]: { step: "failed", message: errorText(e) } }));
      }
    }
    setRunning(false);
  };

  const refresh = () => {
    setProgress({});
    void reload();
  };

  return (
    <div className="page">
      <PageHeader title="App updates" subtitle="Update installed apps through winget, Windows' package manager">
        <button className="btn" onClick={refresh} disabled={state.step === "loading" || running}>
          Check again
        </button>
        <button className="btn primary" onClick={() => void run(pending)} disabled={running || pending.length === 0}>
          Update all
        </button>
      </PageHeader>

      {state.step === "loading" && <Loading label="Checking for app updates…" />}
      {state.step === "failed" && <Failed message={state.message} onRetry={refresh} />}

      {state.step === "ready" && (
        <div className="list">
          <p className="muted">
            {updates.length === 0
              ? "Every app winget knows about is up to date."
              : "Each app is updated by its own installer, which may ask for administrator permission. Close the app first."}
          </p>
          {updates.length > 0 && (
            <div className="table-wrap">
              <table>
                <thead>
                  <tr>
                    <th>App</th>
                    <th>Installed</th>
                    <th>Available</th>
                    <th />
                  </tr>
                </thead>
                <tbody>
                  {updates.map((u) => {
                    const p = progress[u.id];
                    return (
                      <tr key={u.id} className="static">
                        <td>
                          <div className="device">{u.name}</div>
                          <div className="muted small mono">{u.id}</div>
                          {p?.step === "failed" && <div className="small detail-error wrap">{p.message}</div>}
                        </td>
                        <td className="mono">{u.currentVersion}</td>
                        <td className="mono">{u.availableVersion}</td>
                        <td className="row-action">
                          {p?.step === "updating" ? (
                            <span className="stage">
                              <span className="spinner" />
                              Updating…
                            </span>
                          ) : p?.step === "queued" ? (
                            <span className="stage">Waiting</span>
                          ) : p?.step === "done" ? (
                            <span className="stage installed">Updated</span>
                          ) : (
                            <button className="btn small" disabled={running} onClick={() => void run([u])}>
                              {p?.step === "failed" ? "Try again" : "Update"}
                            </button>
                          )}
                        </td>
                      </tr>
                    );
                  })}
                </tbody>
              </table>
            </div>
          )}
        </div>
      )}
    </div>
  );
}
