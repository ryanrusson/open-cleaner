import { useEffect, useState } from "react";
import { cleanerClean, cleanerScan } from "../api";
import { errorText, formatSize } from "../format";
import type { CleanResult } from "../types";
import { Dialog, Failed, Loading, PageHeader, useLoad } from "../ui";

export default function CleanerPage() {
  const { state, reload } = useLoad(cleanerScan);
  const [selected, setSelected] = useState<Set<string>>(new Set());
  const [confirming, setConfirming] = useState(false);
  const [cleaning, setCleaning] = useState(false);
  const [outcome, setOutcome] = useState<CleanResult | string | null>(null);

  const categories = state.step === "ready" ? state.data : [];

  // Each fresh scan starts from the recommended set that has something in it.
  useEffect(() => {
    if (state.step === "ready") {
      setSelected(new Set(state.data.filter((c) => c.recommended && c.sizeBytes > 0).map((c) => c.id)));
    }
  }, [state]);

  const chosen = categories.filter((c) => selected.has(c.id));
  const chosenBytes = chosen.reduce((n, c) => n + c.sizeBytes, 0);
  const totalBytes = categories.reduce((n, c) => n + c.sizeBytes, 0);

  const toggle = (id: string) =>
    setSelected((set) => {
      const next = new Set(set);
      if (!next.delete(id)) next.add(id);
      return next;
    });

  const clean = async () => {
    setConfirming(false);
    setCleaning(true);
    try {
      setOutcome(await cleanerClean([...selected]));
    } catch (e) {
      setOutcome(errorText(e));
    }
    setCleaning(false);
    void reload();
  };

  return (
    <div className="page">
      <PageHeader title="Cleaner" subtitle="Free up space by removing files you do not need">
        <button className="btn" onClick={() => void reload()} disabled={state.step === "loading" || cleaning}>
          Scan again
        </button>
      </PageHeader>

      {(state.step === "loading" || cleaning) && (
        <Loading label={cleaning ? "Cleaning…" : "Measuring what can be removed…"} />
      )}
      {state.step === "failed" && <Failed message={state.message} onRetry={() => void reload()} />}

      {state.step === "ready" && !cleaning && (
        <div className="list">
          {outcome !== null && (
            <div className={typeof outcome === "string" ? "banner error" : "banner good"}>
              <span>
                {typeof outcome === "string"
                  ? outcome
                  : `Freed ${formatSize(outcome.freedBytes) || "0 KB"}.` +
                    (outcome.skipped > 0
                      ? ` ${outcome.skipped.toLocaleString()} items were in use and left alone.`
                      : "")}
              </span>
              <button className="icon-btn" onClick={() => setOutcome(null)} aria-label="Dismiss">
                ✕
              </button>
            </div>
          )}

          <div className="table-wrap">
            {categories.map((c) => (
              <label key={c.id} className={c.sizeBytes === 0 ? "check-row empty-row" : "check-row"}>
                <input
                  type="checkbox"
                  checked={selected.has(c.id)}
                  disabled={c.sizeBytes === 0}
                  onChange={() => toggle(c.id)}
                />
                <span className="check-text">
                  <span className="device">{c.label}</span>
                  <span className="muted small">{c.description}</span>
                </span>
                <span className="check-size">
                  {c.sizeBytes === 0 ? <span className="muted">Nothing to clean</span> : formatSize(c.sizeBytes)}
                  {c.itemCount > 0 && <span className="muted small">{c.itemCount.toLocaleString()} items</span>}
                </span>
              </label>
            ))}
          </div>

          <div className="footer-bar">
            <span className="muted">
              {formatSize(totalBytes) || "Nothing"} found · {formatSize(chosenBytes) || "nothing"} selected
            </span>
            <button className="btn primary" disabled={chosenBytes === 0} onClick={() => setConfirming(true)}>
              Clean {formatSize(chosenBytes)}
            </button>
          </div>
        </div>
      )}

      {confirming && (
        <Dialog
          title={`Remove ${formatSize(chosenBytes)}?`}
          actions={
            <>
              <button className="btn" onClick={() => setConfirming(false)}>
                Cancel
              </button>
              <button className="btn primary" onClick={() => void clean()}>
                Clean
              </button>
            </>
          }
        >
          <ul className="plain-list">
            {chosen.map((c) => (
              <li key={c.id}>
                <span>{c.label}</span>
                <span className="muted">{formatSize(c.sizeBytes)}</span>
              </li>
            ))}
          </ul>
          <p className="muted">
            {selected.has("recycle-bin")
              ? "These files are deleted permanently, including everything in the Recycle Bin."
              : "These files are deleted permanently. Files in use are left alone."}
          </p>
        </Dialog>
      )}
    </div>
  );
}
