import { BookA, House, Settings2, type LucideIcon } from "lucide-react";
import { useT } from "../i18n/context";
import { modelFor } from "../lib/api";
import { lookOf } from "../lib/look";
import { keycaps } from "../lib/shortcut";
import { useParler } from "../state";
import { IconTile, Keycaps, LogoMark, StatusDot, Wordmark, type DotTone } from "../ui";

export type Page = "home" | "vocabulary" | "settings";

const NAV: { page: Page; icon: LucideIcon; label: string }[] = [
  { page: "home", icon: House, label: "nav.home" },
  { page: "vocabulary", icon: BookA, label: "nav.vocabulary" },
  { page: "settings", icon: Settings2, label: "nav.settings" },
];

export function Sidebar({
  page,
  onNavigate,
}: {
  page: Page;
  onNavigate: (page: Page) => void;
}) {
  const { t } = useT();
  const { info, settings, status, provider } = useParler();
  const mac = info.platform === "macos";

  const missingKey = (provider?.requiresApiKey ?? false) && !settings.apiKeys[settings.provider]?.trim();
  const [tone, label]: [DotTone, string] =
    status.state === "recording"
      ? ["listening", t("status.listening")]
      : status.state === "transcribing"
        ? ["busy", t("status.writing")]
        : status.problem || missingKey
          ? ["attention", t("status.attention")]
          : ["ready", t("status.ready")];
  const look = lookOf(settings.provider);

  return (
    <aside className="relative z-10 flex w-[236px] shrink-0 flex-col border-r border-border bg-sidebar/80">
      {/* macOS draws its window buttons here. */}
      <div data-tauri-drag-region className={mac ? "h-[52px]" : "h-5"} />
      <div data-tauri-drag-region className="flex items-center gap-2.5 px-5">
        <LogoMark size={28} live={status.state === "recording"} />
        <Wordmark />
      </div>

      <nav className="mt-7 flex flex-col gap-0.5 px-3">
        {NAV.map(({ page: target, icon: Icon, label: key }) => {
          const active = target === page;
          return (
            <button
              key={target}
              type="button"
              onClick={() => onNavigate(target)}
              aria-current={active ? "page" : undefined}
              className={`flex h-9 items-center gap-2.5 rounded-xl px-3 text-[13.5px] font-medium transition-colors ${
                active
                  ? "bg-surface text-text shadow-card"
                  : "text-text-2 hover:bg-text/5 hover:text-text"
              }`}
            >
              <Icon className="size-[17px]" strokeWidth={active ? 2.1 : 1.8} />
              {t(key)}
            </button>
          );
        })}
      </nav>

      <div className="flex-1" />

      <button
        type="button"
        onClick={() => onNavigate("settings")}
        className="mx-3 mb-3 flex flex-col gap-3 rounded-card bg-surface p-3.5 text-left shadow-card transition-shadow hover:shadow-[0_0_0_1px_var(--border-strong)]"
      >
        <span className="flex items-center gap-2 text-[12.5px] font-medium text-text">
          <StatusDot tone={tone} />
          {label}
        </span>
        <span className="flex items-center gap-2.5">
          <IconTile icon={look.icon} tint={look.tint} size="sm" />
          <span className="min-w-0">
            <span className="block text-[12.5px] font-medium text-text">{provider?.label}</span>
            <span className="block truncate font-mono text-[10.5px] text-text-3">
              {provider ? modelFor(settings, provider) : ""}
            </span>
          </span>
        </span>
        <Keycaps keys={keycaps(settings.shortcut, info.platform, t("common.space"))} size="sm" />
      </button>
      <p className="px-5 pb-4 font-mono text-[10.5px] text-text-3">Parler {info.version}</p>
    </aside>
  );
}
