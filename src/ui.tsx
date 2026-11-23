import { type ReactNode, useCallback, useEffect, useState } from "react";
import { errorText } from "./format";
import type { Load } from "./store";

/** Runs `fetcher` on mount and again on `reload()`. */
export function useLoad<T>(fetcher: () => Promise<T>) {
  const [state, setState] = useState<Load<T>>({ step: "loading" });
  const reload = useCallback(async () => {
    setState({ step: "loading" });
    try {
      setState({ step: "ready", data: await fetcher() });
    } catch (e) {
      setState({ step: "failed", message: errorText(e) });
    }
    // The fetchers passed in are module-level functions and never change.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);
  useEffect(() => {
    void reload();
  }, [reload]);
  return { state, reload, setState };
}

export function PageHeader({ title, subtitle, children }: { title: string; subtitle: string; children?: ReactNode }) {
  return (
    <header className="topbar">
      <div className="brand">
        <div>
          <h1>{title}</h1>
          <p className="muted">{subtitle}</p>
        </div>
      </div>
      <div className="actions">{children}</div>
    </header>
  );
}

export function Loading({ label }: { label: string }) {
  return (
    <div className="center">
      <span className="spinner large" />
      <p>{label}</p>
    </div>
  );
}

export function Failed({ message, onRetry }: { message: string; onRetry: () => void }) {
  return (
    <div className="center">
      <h2>That did not work</h2>
      <p className="muted">{message}</p>
      <button className="btn primary" onClick={onRetry}>
        Try again
      </button>
    </div>
  );
}

export function Dialog({ title, children, actions }: { title: string; children: ReactNode; actions: ReactNode }) {
  return (
    <div className="overlay">
      <div className="dialog" role="dialog" aria-modal="true" aria-label={title}>
        <h2>{title}</h2>
        {children}
        <div className="dialog-actions">{actions}</div>
      </div>
    </div>
  );
}
