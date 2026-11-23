import { useEffect } from "react";
import { formatSize } from "../format";
import { driverUpdates, drivers, ensureDrivers, junk, software, startup, useResource, useWatch } from "../store";
import type { Status } from "../types";
import { PageHeader } from "../ui";

type Tone = Status | "busy";

interface Card {
  page: string;
  title: string;
  /** The headline: a count, a size, or a short state. */
  value: string;
  detail: string;
  tone: Tone;
  action: string;
}

export default function OverviewPage({ onOpen }: { onOpen: (page: string) => void }) {
  const scan = useWatch(drivers);
  const updates = useWatch(driverUpdates);
  const junkFiles = useResource(junk).state;
  const startupItems = useResource(startup).state;
  const appUpdates = useResource(software).state;
  useEffect(() => ensureDrivers(), []);

  const problems =
    scan.step === "ready" ? scan.data.drivers.filter((d) => d.problem).length : 0;
  const driverCard: Card = (() => {
    const base = { page: "drivers", title: "Drivers", action: "Review drivers" };
    if (scan.step === "failed") return { ...base, value: "Not checked", detail: scan.message, tone: "check" };
    if (updates.step === "failed") return { ...base, value: "Not checked", detail: updates.message, tone: "check" };
    if (scan.step === "loading" || updates.step === "loading")
      return { ...base, value: "Checking…", detail: "Asking Windows Update. This can take a minute.", tone: "busy" };
    const count = new Set(updates.data.matched.map((m) => m.update.updateId)).size;
    const detail = problems > 0 ? `${problems} device${problems === 1 ? " has" : "s have"} a problem.` : "";
    if (count > 0)
      return { ...base, value: `${count} update${count === 1 ? "" : "s"}`, detail: detail || "Ready to install from Windows Update.", tone: "update" };
    if (problems > 0) return { ...base, value: "Needs attention", detail, tone: "problem" };
    return { ...base, value: "Up to date", detail: "Windows Update has nothing newer.", tone: "ok" };
  })();

  const softwareCard: Card = (() => {
    const base = { page: "software", title: "App updates", action: "Review app updates" };
    if (appUpdates.step === "loading") return { ...base, value: "Checking…", detail: "Asking winget.", tone: "busy" };
    if (appUpdates.step === "failed") return { ...base, value: "Not checked", detail: appUpdates.message, tone: "check" };
    const count = appUpdates.data.length;
    return count > 0
      ? { ...base, value: `${count} update${count === 1 ? "" : "s"}`, detail: appUpdates.data.slice(0, 3).map((u) => u.name).join(", ") + (count > 3 ? "…" : ""), tone: "update" }
      : { ...base, value: "Up to date", detail: "Every app winget knows about is current.", tone: "ok" };
  })();

  const cleanerCard: Card = (() => {
    const base = { page: "cleaner", title: "Cleaner", action: "Choose what to clean" };
    if (junkFiles.step === "loading") return { ...base, value: "Measuring…", detail: "", tone: "busy" };
    if (junkFiles.step === "failed") return { ...base, value: "Not checked", detail: junkFiles.message, tone: "check" };
    const safe = junkFiles.data.filter((c) => c.recommended).reduce((n, c) => n + c.sizeBytes, 0);
    const total = junkFiles.data.reduce((n, c) => n + c.sizeBytes, 0);
    // Under 100 MB is not worth anyone's attention.
    return safe > 100 * 1024 * 1024
      ? { ...base, value: formatSize(safe), detail: `Safe to remove. ${formatSize(total)} in total if you include the optional items.`, tone: "check" }
      : { ...base, value: "Tidy", detail: total > 0 ? `${formatSize(total)} in optional items.` : "Nothing to clean.", tone: "ok" };
  })();

  const startupCard: Card = (() => {
    const base = { page: "startup", title: "Startup", action: "Manage startup apps" };
    if (startupItems.step === "loading") return { ...base, value: "Checking…", detail: "", tone: "busy" };
    if (startupItems.step === "failed") return { ...base, value: "Not checked", detail: startupItems.message, tone: "check" };
    const on = startupItems.data.filter((i) => i.enabled).length;
    return { ...base, value: `${on} app${on === 1 ? "" : "s"}`, detail: "Start with Windows. Fewer means a faster sign-in.", tone: "ok" };
  })();

  const cards = [driverCard, softwareCard, cleanerCard, startupCard];
  const busy = cards.some((c) => c.tone === "busy");
  const attention = cards.filter((c) => c.tone === "update" || c.tone === "problem" || c.tone === "check").length;
  const system = scan.step === "ready" ? scan.data.system : null;

  return (
    <div className="page">
      <PageHeader
        title="Overview"
        subtitle={
          system
            ? [system.manufacturer, system.model].filter(Boolean).join(" ") + (system.os ? ` · ${system.os}` : "")
            : "Checking this PC"
        }
      />
      <div className="list scroll">
        <h2 className="headline">
          {busy
            ? "Checking your PC…"
            : attention === 0
              ? "Everything looks good."
              : `${attention} thing${attention === 1 ? "" : "s"} worth a look.`}
        </h2>
        <div className="cards">
          {cards.map((card) => (
            <section key={card.page} className={`card health ${card.tone}`}>
              <span className="muted">{card.title}</span>
              <span className="health-value">
                {card.tone === "busy" && <span className="spinner" />}
                {card.value}
              </span>
              <p className="muted small">{card.detail}</p>
              <button className="btn small" onClick={() => onOpen(card.page)}>
                {card.action}
              </button>
            </section>
          ))}
        </div>
        <div className="row">
          <button className="btn" onClick={() => onOpen("duplicates")}>
            Find duplicate files
          </button>
          <button className="btn" onClick={() => onOpen("apps")}>
            Uninstall apps
          </button>
          <button className="btn" onClick={() => onOpen("disk")}>
            See what fills the disk
          </button>
        </div>
      </div>
    </div>
  );
}
