import { getCurrentWindow } from "@tauri-apps/api/window";
import { useEffect, useState } from "react";
import { Sidebar, type Page } from "./components/Sidebar";
import { LocaleProvider, useT } from "./i18n/context";
import { api, events, type Theme } from "./lib/api";
import { Onboarding } from "./onboarding/Onboarding";
import { HistoryPage } from "./pages/History";
import { Home } from "./pages/Home";
import { SettingsPage } from "./pages/Settings";
import { Vocabulary } from "./pages/Vocabulary";
import { loadSnapshot, ParlerProvider, useParler, type Snapshot } from "./state";

export default function App() {
  const [snapshot, setSnapshot] = useState<Snapshot | null>(null);

  useEffect(() => {
    loadSnapshot().then((s) => {
      document.documentElement.dataset.platform = s.info.platform;
      setSnapshot(s);
    });
  }, []);

  if (!snapshot) return null;
  return (
    <ParlerProvider initial={snapshot}>
      <Localized />
    </ParlerProvider>
  );
}

/** The chosen theme on the page, and on the window's own controls. */
function useTheme(theme: Theme) {
  useEffect(() => {
    const root = document.documentElement;
    if (theme === "system") delete root.dataset.theme;
    else root.dataset.theme = theme;
    getCurrentWindow()
      .setTheme(theme === "system" ? null : theme)
      .catch(() => {});
  }, [theme]);
}

function Localized() {
  const { settings } = useParler();
  useTheme(settings.theme);
  return (
    <LocaleProvider pref={settings.uiLanguage}>
      <Shell />
    </LocaleProvider>
  );
}

function Shell() {
  const { t, locale } = useT();
  const { settings } = useParler();
  const [page, setPage] = useState<Page>("home");

  useEffect(() => {
    document.documentElement.lang = locale;
    api.setTrayLabels({
      open: t("tray.open"),
      toggle: t("tray.toggle"),
      copyLast: t("tray.copyLast"),
      history: t("tray.history"),
      quit: t("tray.quit"),
    });
  }, [locale, t]);

  // The tray's "History…" opens the window on that page.
  useEffect(() => {
    const unlisten = events.navigate((target) => {
      if (target === "history") setPage("history");
    });
    return () => {
      unlisten.then((stop) => stop());
    };
  }, []);

  if (!settings.onboarded) return <Onboarding />;

  return (
    <div className="grain relative flex h-full">
      <Sidebar page={page} onNavigate={setPage} />
      <main className="relative flex-1 overflow-y-auto">
        <div data-tauri-drag-region className="sticky top-0 z-10 h-11" />
        <div key={page} className="mx-auto max-w-[700px] px-10 pb-14">
          {page === "home" && (
            <Home
              openSettings={() => setPage("settings")}
              openHistory={() => setPage("history")}
            />
          )}
          {page === "history" && <HistoryPage />}
          {page === "vocabulary" && <Vocabulary />}
          {page === "settings" && <SettingsPage />}
        </div>
      </main>
    </div>
  );
}
