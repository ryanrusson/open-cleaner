import { diskScan, inApp, pickFolder, revealInExplorer } from "../api";
import { formatDate, formatSize } from "../format";
import { Failed, Loading, PageHeader, useLoad } from "../ui";
import { useCallback, useRef } from "react";

export default function DiskPage() {
  // The folder being analysed; undefined means the user's profile folder.
  const folder = useRef<string | undefined>(undefined);
  const fetcher = useCallback(() => diskScan(folder.current), []);
  const { state, reload } = useLoad(fetcher);

  const choose = async () => {
    const picked = await pickFolder();
    if (picked) {
      folder.current = picked;
      void reload();
    }
  };

  const report = state.step === "ready" ? state.data : null;
  const biggest = report?.folders[0]?.sizeBytes || 1;
  const fileName = (path: string) => path.slice(path.lastIndexOf("\\") + 1);
  const parent = (path: string) => path.slice(0, path.lastIndexOf("\\"));

  return (
    <div className="page">
      <PageHeader title="Disk" subtitle={report ? report.root : "See what is taking up space"}>
        <button className="btn" onClick={() => void choose()} disabled={state.step === "loading" || !inApp}>
          Choose folder
        </button>
        <button className="btn" onClick={() => void reload()} disabled={state.step === "loading"}>
          Scan again
        </button>
      </PageHeader>

      {state.step === "loading" && <Loading label="Measuring folders. Large ones can take a minute…" />}
      {state.step === "failed" && <Failed message={state.message} onRetry={() => void reload()} />}

      {report && (
        <div className="list scroll">
          <p className="muted">
            {formatSize(report.totalBytes) || "0 KB"} in {report.fileCount.toLocaleString()} files.
            {report.unreadable > 0 &&
              ` ${report.unreadable.toLocaleString()} protected folders could not be read and are not counted.`}
          </p>

          <section className="card">
            <h2>Folders</h2>
            <ul className="bars">
              {report.folders.map((f) => (
                <li key={f.name}>
                  <div className="bar-head">
                    <button className="link" onClick={() => void revealInExplorer(f.path)} title="Show in Explorer">
                      {f.name}
                    </button>
                    <span>
                      {formatSize(f.sizeBytes) || "0 KB"}
                      <span className="muted small"> · {f.fileCount.toLocaleString()} files</span>
                    </span>
                  </div>
                  <div className="bar">
                    <span style={{ width: `${Math.max(0.5, (f.sizeBytes / biggest) * 100)}%` }} />
                  </div>
                </li>
              ))}
            </ul>
            {report.folders.length === 0 && <p className="muted">This folder is empty.</p>}
          </section>

          <section className="card">
            <h2>Largest files</h2>
            <table>
              <tbody>
                {report.largestFiles.map((f) => (
                  <tr key={f.path} className="static">
                    <td>
                      <div className="device wrap">{fileName(f.path)}</div>
                      <div className="muted small wrap">{parent(f.path)}</div>
                    </td>
                    <td className="nowrap muted">{formatDate(f.modified)}</td>
                    <td className="nowrap">{formatSize(f.sizeBytes) || "0 KB"}</td>
                    <td className="row-action">
                      <button className="btn small" onClick={() => void revealInExplorer(f.path)}>
                        Show in Explorer
                      </button>
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          </section>
        </div>
      )}
    </div>
  );
}
