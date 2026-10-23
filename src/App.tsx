import { ArrowLeft, Settings2 } from "lucide-react";
import { useEffect, useState } from "react";
import { Home } from "./components/Home";
import { Onboarding } from "./components/Onboarding";
import { SettingsView } from "./components/SettingsView";
import { IconButton } from "./components/ui";
import { LocaleProvider, useT } from "./i18n/context";
import { api } from "./lib/api";
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
  const { t, locale } = useT();
  const { settings, info } = useParler();
  const [view, setView] = useState<"home" | "settings">("home");

  useEffect(() => {
    document.documentElement.lang = locale;
    api.setLocale(locale);
  }, [locale]);

  const mac = info.platform === "macos";
  const onboarding = !settings.onboarded;

  return (
    <div className="flex h-full flex-col">
      {/* The title bar: macOS draws its window buttons over the left side. */}
      <header
        data-tauri-drag-region
        className={`flex h-[52px] shrink-0 items-center justify-between pr-3 ${
          mac ? "pl-[84px]" : "pl-5"
        }`}
      >
        <span
          data-tauri-drag-region
          className="font-display text-[21px] text-ink italic select-none"
        >
          {view === "settings" && !onboarding ? t("settings.title") : "Parler"}
        </span>
        {!onboarding &&
          (view === "home" ? (
            <IconButton label={t("common.settings")} onClick={() => setView("settings")}>
              <Settings2 className="size-[18px]" />
            </IconButton>
          ) : (
            <IconButton label={t("common.back")} onClick={() => setView("home")}>
              <ArrowLeft className="size-[18px]" />
            </IconButton>
          ))}
      </header>
      <main className="flex-1 overflow-y-auto">
        {onboarding ? (
          <Onboarding />
        ) : view === "home" ? (
          <Home openSettings={() => setView("settings")} />
        ) : (
          <SettingsView />
        )}
      </main>
    </div>
  );
}
