import { useState } from "react";
import { startupList, startupSetEnabled } from "../api";
import { errorText } from "../format";
import type { StartupItem } from "../types";
import { Failed, Loading, PageHeader, useLoad } from "../ui";

export default function StartupPage() {
  const { state, reload, setState } = useLoad(startupList);
  const [error, setError] = useState<string | null>(null);

  const toggle = async (item: StartupItem) => {
    const set = (enabled: boolean) =>
      setState((s) =>
        s.step === "ready"
          ? { ...s, data: s.data.map((i) => (i.id === item.id ? { ...i, enabled } : i)) }
          : s,
      );
    setError(null);
    set(!item.enabled);
    try {
      await startupSetEnabled(item.id, !item.enabled);
    } catch (e) {
      set(item.enabled);
      setError(errorText(e));
    }
  };

  const items = state.step === "ready" ? state.data : [];
  const on = items.filter((i) => i.enabled).length;

  return (
    <div className="page">
      <PageHeader title="Startup" subtitle="Choose which apps launch when you sign in">
        <button className="btn" onClick={() => void reload()} disabled={state.step === "loading"}>
          Refresh
        </button>
      </PageHeader>

      {state.step === "loading" && <Loading label="Reading startup apps…" />}
      {state.step === "failed" && <Failed message={state.message} onRetry={() => void reload()} />}

      {state.step === "ready" && (
        <div className="list">
          {error && (
            <div className="banner error">
              <span>{error}</span>
              <button className="icon-btn" onClick={() => setError(null)} aria-label="Dismiss">
                ✕
              </button>
            </div>
          )}
          <p className="muted">
            {on} of {items.length} start with Windows. Turning one off does not uninstall it, and you can turn it
            back on here or in Task Manager.
          </p>
          <div className="table-wrap">
            <table>
              <thead>
                <tr>
                  <th>App</th>
                  <th>Where it is set</th>
                  <th>Starts with Windows</th>
                </tr>
              </thead>
              <tbody>
                {items.map((item) => (
                  <tr key={item.id} className="static">
                    <td>
                      <div className="device">{item.name}</div>
                      <div className="muted small mono wrap">{item.command}</div>
                    </td>
                    <td className="nowrap">{item.source}</td>
                    <td className="nowrap">
                      <button
                        role="switch"
                        aria-checked={item.enabled}
                        aria-label={`${item.name} starts with Windows`}
                        className="switch"
                        disabled={!item.canToggle}
                        title={item.canToggle ? undefined : "Applies to all users. Change it in Task Manager."}
                        onClick={() => void toggle(item)}
                      >
                        <span />
                      </button>
                      <span className="switch-label">
                        {item.enabled ? "On" : "Off"}
                        {!item.canToggle && <span className="muted small"> · all users</span>}
                      </span>
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
            {items.length === 0 && <p className="empty muted">Nothing is set to start with Windows.</p>}
          </div>
        </div>
      )}
    </div>
  );
}
