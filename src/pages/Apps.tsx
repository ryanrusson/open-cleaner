import { useMemo, useState } from "react";
import { appsList, appsUninstall } from "../api";
import { errorText, formatSize } from "../format";
import type { InstalledApp } from "../types";
import { Dialog, Failed, Loading, PageHeader, useLoad } from "../ui";

type Sort = "name" | "size" | "date";

export default function AppsPage() {
  const { state, reload } = useLoad(appsList);
  const [query, setQuery] = useState("");
  const [sort, setSort] = useState<Sort>("name");
  const [confirming, setConfirming] = useState<InstalledApp | null>(null);
  const [notice, setNotice] = useState<{ error: boolean; text: string } | null>(null);

  const apps = state.step === "ready" ? state.data : [];
  const rows = useMemo(() => {
    const q = query.trim().toLowerCase();
    return apps
      .filter((a) => !q || `${a.name}\n${a.publisher}`.toLowerCase().includes(q))
      .sort((a, b) =>
        sort === "size"
          ? (b.sizeBytes ?? 0) - (a.sizeBytes ?? 0)
          : sort === "date"
            ? (b.installDate ?? "").localeCompare(a.installDate ?? "")
            : a.name.localeCompare(b.name),
      );
  }, [apps, query, sort]);

  const uninstall = async (app: InstalledApp) => {
    setConfirming(null);
    try {
      await appsUninstall(app.id);
      setNotice({ error: false, text: `The uninstaller for ${app.name} has opened. Refresh when it finishes.` });
    } catch (e) {
      setNotice({ error: true, text: errorText(e) });
    }
  };

  return (
    <div className="page">
      <PageHeader title="Apps" subtitle="See what is installed and remove what you do not use">
        <button className="btn" onClick={() => void reload()} disabled={state.step === "loading"}>
          Refresh
        </button>
      </PageHeader>

      {state.step === "loading" && <Loading label="Listing installed apps…" />}
      {state.step === "failed" && <Failed message={state.message} onRetry={() => void reload()} />}

      {state.step === "ready" && (
        <div className="list">
          {notice && (
            <div className={notice.error ? "banner error" : "banner"}>
              <span>{notice.text}</span>
              <button className="icon-btn" onClick={() => setNotice(null)} aria-label="Dismiss">
                ✕
              </button>
            </div>
          )}
          <div className="toolbar">
            <input
              type="search"
              placeholder={`Search ${apps.length} apps`}
              value={query}
              onChange={(e) => setQuery(e.target.value)}
            />
            <label className="toggle">
              Sort by
              <select value={sort} onChange={(e) => setSort(e.target.value as Sort)}>
                <option value="name">Name</option>
                <option value="size">Size</option>
                <option value="date">Recently installed</option>
              </select>
            </label>
          </div>
          <div className="table-wrap">
            <table>
              <thead>
                <tr>
                  <th>App</th>
                  <th>Version</th>
                  <th>Installed</th>
                  <th>Size</th>
                  <th />
                </tr>
              </thead>
              <tbody>
                {rows.map((app) => (
                  <tr key={app.id} className="static">
                    <td>
                      <div className="device">{app.name}</div>
                      <div className="muted small">{app.publisher}</div>
                    </td>
                    <td className="mono">{app.version || "–"}</td>
                    <td className="nowrap">{app.installDate ?? "–"}</td>
                    <td className="nowrap">{formatSize(app.sizeBytes) || "–"}</td>
                    <td className="row-action">
                      <button className="btn small" disabled={!app.canUninstall} onClick={() => setConfirming(app)}>
                        Uninstall
                      </button>
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
            {rows.length === 0 && <p className="empty muted">No apps match.</p>}
          </div>
        </div>
      )}

      {confirming && (
        <Dialog
          title={`Uninstall ${confirming.name}?`}
          actions={
            <>
              <button className="btn" onClick={() => setConfirming(null)}>
                Cancel
              </button>
              <button className="btn primary" onClick={() => void uninstall(confirming)}>
                Open uninstaller
              </button>
            </>
          }
        >
          <p className="muted">
            This opens the app's own uninstaller, which will walk you through removing it and may ask for
            administrator permission.
          </p>
        </Dialog>
      )}
    </div>
  );
}
