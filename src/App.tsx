import { useEffect, useState } from "react";
import { Sidebar, type Page } from "./components/Sidebar";
import { LocaleProvider, useT } from "./i18n/context";
import { api } from "./lib/api";
import { Onboarding } from "./onboarding/Onboarding";
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

function Localized() {
  const { settings } = useParler();
  return (
    <LocaleProvider pref={settings.uiLanguage}>
      <Shell />
    </LocaleProvider>
  );
}

function Shell() {
  const { locale } = useT();
  const { settings } = useParler();
  const [page, setPage] = useState<Page>("home");

  useEffect(() => {
    document.documentElement.lang = locale;
    api.setLocale(locale);
  }, [locale]);

  if (!settings.onboarded) return <Onboarding />;

  return (
    <div className="grain relative flex h-full">
      <Sidebar page={page} onNavigate={setPage} />
      <main className="relative flex-1 overflow-y-auto">
        <div data-tauri-drag-region className="sticky top-0 z-10 h-11" />
        <div key={page} className="mx-auto max-w-[700px] px-10 pb-14">
          {page === "home" && <Home openSettings={() => setPage("settings")} />}
          {page === "vocabulary" && <Vocabulary />}
          {page === "settings" && <SettingsPage />}
        </div>
      </main>
    </div>
  );
}
