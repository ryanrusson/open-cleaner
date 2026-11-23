import { useState } from "react";
import { duplicatesCancel, duplicatesFind, duplicatesRemove, inApp, pickFolder, revealInExplorer } from "../api";
import { errorText, formatDate, formatSize } from "../format";
import type { DuplicateGroup, DuplicateReport } from "../types";
import { Dialog, Failed, PageHeader } from "../ui";

type Phase =
  | { step: "idle" }
  | { step: "scanning" }
  | { step: "failed"; message: string }
  | { step: "ready"; report: DuplicateReport };

export default function DuplicatesPage() {
  const [phase, setPhase] = useState<Phase>({ step: "idle" });
  const [folder, setFolder] = useState<string | undefined>(undefined);
  const [selected, setSelected] = useState<Set<string>>(new Set());
  const [confirming, setConfirming] = useState(false);
  const [notice, setNotice] = useState<{ error: boolean; text: string } | null>(null);

  const scan = async (path?: string) => {
    setFolder(path);
    setSelected(new Set());
    setNotice(null);
    setPhase({ step: "scanning" });
    try {
      setPhase({ step: "ready", report: await duplicatesFind(path) });
    } catch (e) {
      // Cancelling is not a failure worth an error screen.
      setPhase(errorText(e) === "Cancelled." ? { step: "idle" } : { step: "failed", message: errorText(e) });
    }
  };

  const choose = async () => {
    const picked = await pickFolder();
    if (picked) void scan(picked);
  };

  const groups = phase.step === "ready" ? phase.report.groups : [];
  const selectedBytes = groups.reduce(
    (n, g) => n + g.sizeBytes * g.files.filter((f) => selected.has(f.path)).length,
    0,
  );
  // The last unticked copy in a group cannot be ticked: one must stay.
  const isLastCopy = (g: DuplicateGroup, path: string) =>
    !selected.has(path) && g.files.filter((f) => !selected.has(f.path)).length === 1;

  const toggle = (path: string) =>
    setSelected((set) => {
      const next = new Set(set);
      if (!next.delete(path)) next.add(path);
      return next;
    });

  const remove = async () => {
    setConfirming(false);
    if (phase.step !== "ready") return;
    try {
      const result = await duplicatesRemove([...selected]);
      setNotice({
        error: result.failed > 0,
        text:
          `Moved ${result.removed} file${result.removed === 1 ? "" : "s"} to the Recycle Bin, freeing ${formatSize(result.freedBytes) || "0 KB"}.` +
          (result.failed > 0 ? ` ${result.failed} could not be moved; scan again to see what is left.` : ""),
      });
      const remaining = phase.report.groups
        .map((g) => ({ ...g, files: g.files.filter((f) => !selected.has(f.path)) }))
        .filter((g) => g.files.length > 1);
      setPhase({
        step: "ready",
        report: {
          ...phase.report,
          groups: remaining,
          wastedBytes: remaining.reduce((n, g) => n + g.sizeBytes * (g.files.length - 1), 0),
        },
      });
      setSelected(new Set());
    } catch (e) {
      setNotice({ error: true, text: errorText(e) });
    }
  };

  const busy = phase.step === "scanning";

  return (
    <div className="page">
      <PageHeader
        title="Duplicates"
        subtitle={phase.step === "ready" ? phase.report.root : "Find identical files that are taking up space twice"}
      >
        <button className="btn" onClick={() => void choose()} disabled={busy || !inApp}>
          Choose folder
        </button>
        {phase.step !== "idle" && (
          <button className="btn" onClick={() => void scan(folder)} disabled={busy}>
            Scan again
          </button>
        )}
      </PageHeader>

      {phase.step === "idle" && (
        <div className="center">
          <h2>Find duplicate files</h2>
          <p className="muted narrow">
            Looks through your user folder for files of 1 MB or more that are exact copies of each other. App
            data and other hidden folders are skipped. Nothing is removed until you choose.
          </p>
          <button className="btn primary" onClick={() => void scan()}>
            Scan my user folder
          </button>
        </div>
      )}

      {busy && (
        <div className="center">
          <span className="spinner large" />
          <p>Comparing files. Large folders can take several minutes…</p>
          <button className="btn" onClick={() => void duplicatesCancel()}>
            Cancel
          </button>
        </div>
      )}

      {phase.step === "failed" && <Failed message={phase.message} onRetry={() => void scan(folder)} />}
      {phase.step === "ready" && groups.length === 0 && !notice && (
        <div className="center">
          <h2>No duplicates found</h2>
          <p className="muted">Checked {phase.report.filesChecked.toLocaleString()} files of 1 MB or more.</p>
        </div>
      )}

      {phase.step === "ready" && (groups.length > 0 || notice) && (
        <div className="list">
          {notice && (
            <div className={notice.error ? "banner error" : "banner good"}>
              <span>{notice.text}</span>
              <button className="icon-btn" onClick={() => setNotice(null)} aria-label="Dismiss">
                ✕
              </button>
            </div>
          )}
          <div className="footer-bar">
            <span className="muted">
              {formatSize(phase.report.wastedBytes) || "Nothing"} taken up by extra copies in {groups.length} set
              {groups.length === 1 ? "" : "s"}.
            </span>
            <span className="row">
              <button
                className="btn small"
                onClick={() => setSelected(new Set(groups.flatMap((g) => g.files.slice(1).map((f) => f.path))))}
              >
                Select all but the oldest
              </button>
              <button className="btn small" onClick={() => setSelected(new Set())} disabled={selected.size === 0}>
                Clear selection
              </button>
            </span>
          </div>

          <div className="table-wrap">
            {groups.map((g) => (
              <div key={g.files[0].path} className="dupe-group">
                <div className="dupe-head">
                  <span className="device">
                    {g.files.length} copies · {formatSize(g.sizeBytes)} each
                  </span>
                </div>
                {g.files.map((f) => (
                  <label key={f.path} className="check-row">
                    <input
                      type="checkbox"
                      checked={selected.has(f.path)}
                      disabled={isLastCopy(g, f.path)}
                      title={isLastCopy(g, f.path) ? "One copy is always kept" : undefined}
                      onChange={() => toggle(f.path)}
                    />
                    <span className="check-text">
                      <span className="wrap">{f.path}</span>
                    </span>
                    <span className="muted small nowrap">{formatDate(f.modified)}</span>
                    <button
                      className="btn small"
                      onClick={(e) => {
                        e.preventDefault();
                        void revealInExplorer(f.path);
                      }}
                    >
                      Show
                    </button>
                  </label>
                ))}
              </div>
            ))}
          </div>

          <div className="footer-bar">
            <span className="muted">
              {selected.size} file{selected.size === 1 ? "" : "s"} selected
            </span>
            <button className="btn primary" disabled={selected.size === 0} onClick={() => setConfirming(true)}>
              Move to Recycle Bin {formatSize(selectedBytes)}
            </button>
          </div>
        </div>
      )}

      {confirming && (
        <Dialog
          title={`Move ${selected.size} file${selected.size === 1 ? "" : "s"} to the Recycle Bin?`}
          actions={
            <>
              <button className="btn" onClick={() => setConfirming(false)}>
                Cancel
              </button>
              <button className="btn primary" onClick={() => void remove()}>
                Move to Recycle Bin
              </button>
            </>
          }
        >
          <p className="muted">
            This frees {formatSize(selectedBytes)} once the Recycle Bin is emptied. One copy of every file stays
            where it is, and you can restore the others from the Recycle Bin.
          </p>
        </Dialog>
      )}
    </div>
  );
}
