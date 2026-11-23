import { useEffect, useState } from "react";
import { cleanerClean } from "../api";
import { errorText, formatSize } from "../format";
import type { CleanCategory, CleanOutcome } from "../types";
import { Dialog, Failed, Loading, PageHeader } from "../ui";
import { junk, useResource } from "../store";

export default function CleanerPage() {
  const { state, reload } = useResource(junk);
  const [selected, setSelected] = useState<Set<string>>(new Set());
  const [confirming, setConfirming] = useState(false);
  const [cleaning, setCleaning] = useState(false);
  const [outcome, setOutcome] = useState<CleanOutcome | string | null>(null);

  const categories = state.step === "ready" ? state.data : [];

  // Each fresh scan starts from the recommended set that has something in it.
  useEffect(() => {
    if (state.step === "ready") {
      setSelected(new Set(state.data.filter((c) => c.recommended && c.sizeBytes > 0).map((c) => c.id)));
    }
  }, [state]);

  // A system folder that cannot be read yet may still hold plenty.
  const isEmpty = (c: CleanCategory) => c.sizeBytes === 0 && c.sizeKnown;
  const chosen = categories.filter((c) => selected.has(c.id));
  const needsAdmin = chosen.some((c) => c.needsAdmin);
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
                      : "") +
                    (outcome.note ? ` ${outcome.note}` : "")}
              </span>
              <button className="icon-btn" onClick={() => setOutcome(null)} aria-label="Dismiss">
                ✕
              </button>
            </div>
          )}

          <div className="table-wrap">
            {categories.map((c) => (
              <label key={c.id} className={isEmpty(c) ? "check-row empty-row" : "check-row"}>
                <input
                  type="checkbox"
                  checked={selected.has(c.id)}
                  disabled={isEmpty(c)}
                  onChange={() => toggle(c.id)}
                />
                <span className="check-text">
                  <span className="device">
                    {c.label}
                    {c.needsAdmin && <span className="tag">Needs administrator</span>}
                  </span>
                  <span className="muted small">{c.description}</span>
                </span>
                <span className="check-size">
                  {!c.sizeKnown ? (
                    <span className="muted">Size shown after cleaning</span>
                  ) : c.sizeBytes === 0 ? (
                    <span className="muted">Nothing to clean</span>
                  ) : (
                    formatSize(c.sizeBytes)
                  )}
                  {c.itemCount > 0 && <span className="muted small">{c.itemCount.toLocaleString()} items</span>}
                </span>
              </label>
            ))}
          </div>

          <div className="footer-bar">
            <span className="muted">
              {formatSize(totalBytes) || "Nothing"} found · {formatSize(chosenBytes) || "nothing"} selected
            </span>
            <button className="btn primary" disabled={chosen.length === 0} onClick={() => setConfirming(true)}>
              Clean {formatSize(chosenBytes)}
            </button>
          </div>
        </div>
      )}

      {confirming && (
        <Dialog
          title={chosenBytes > 0 ? `Remove ${formatSize(chosenBytes)}?` : "Clean the selected items?"}
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
                <span className="muted">{c.sizeKnown ? formatSize(c.sizeBytes) : "size unknown"}</span>
              </li>
            ))}
          </ul>
          <p className="muted">
            {selected.has("recycle-bin")
              ? "These files are deleted permanently, including everything in the Recycle Bin."
              : "These files are deleted permanently. Files in use are left alone."}
            {needsAdmin && " Windows will ask for administrator permission for the system files."}
          </p>
        </Dialog>
      )}
    </div>
  );
}
