import { type ReactNode, useEffect, useState } from "react";
import { type AppUpdate, checkForAppUpdate, openLink } from "./api";
import AppsPage from "./pages/Apps";
import CleanerPage from "./pages/Cleaner";
import DiskPage from "./pages/Disk";
import DriversPage from "./pages/Drivers";
import StartupPage from "./pages/Startup";

const icon = (path: ReactNode) => (
  <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
    {path}
  </svg>
);

const PAGES = [
  { id: "drivers", label: "Drivers", Page: DriversPage, icon: icon(<path d="M12 4v11m0 0-4-4m4 4 4-4M5 20h14" />) },
  { id: "cleaner", label: "Cleaner", Page: CleanerPage, icon: icon(<path d="M4 7h16M9 7V4h6v3m-9 0 1 13h10l1-13M10 11v5m4-5v5" />) },
  { id: "startup", label: "Startup", Page: StartupPage, icon: icon(<path d="M12 3v9m5.7-5.7a8 8 0 1 1-11.4 0" />) },
  { id: "apps", label: "Apps", Page: AppsPage, icon: icon(<path d="M4 4h7v7H4zM13 4h7v7h-7zM4 13h7v7H4zM13 13h7v7h-7z" />) },
  { id: "disk", label: "Disk", Page: DiskPage, icon: icon(<path d="M12 3a9 9 0 1 0 9 9h-9z M15 3.5A9 9 0 0 1 20.5 9H15z" />) },
] as const;

type PageId = (typeof PAGES)[number]["id"];

export default function App() {
  const [active, setActive] = useState<PageId>("drivers");
  // Pages load their data when first opened and keep it while you look elsewhere.
  const [opened, setOpened] = useState<PageId[]>(["drivers"]);
  const [appUpdate, setAppUpdate] = useState<AppUpdate | null>(null);

  useEffect(() => {
    void checkForAppUpdate().then(setAppUpdate);
  }, []);

  const go = (id: PageId) => {
    setActive(id);
    setOpened((list) => (list.includes(id) ? list : [...list, id]));
  };

  return (
    <div className="shell">
      <nav className="sidebar" aria-label="Tools">
        <div className="sidebar-brand">
          <svg className="logo" viewBox="0 0 32 32" aria-hidden="true">
            <rect width="32" height="32" rx="8" fill="var(--accent)" />
            <path d="M16 7v11m0 0-4.5-4.5M16 18l4.5-4.5M9 22.5h14" fill="none" stroke="#fff" strokeWidth="2.4" strokeLinecap="round" strokeLinejoin="round" />
          </svg>
          <span>Driver Updater</span>
        </div>
        {PAGES.map((page) => (
          <button
            key={page.id}
            className={page.id === active ? "nav-item active" : "nav-item"}
            onClick={() => go(page.id)}
            aria-current={page.id === active ? "page" : undefined}
          >
            {page.icon}
            {page.label}
          </button>
        ))}
      </nav>

      <div className="main">
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
        {PAGES.filter((page) => opened.includes(page.id)).map(({ id, Page }) => (
          <div key={id} className="page-slot" hidden={id !== active}>
            <Page />
          </div>
        ))}
      </div>
    </div>
  );
}
