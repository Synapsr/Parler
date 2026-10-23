import { Check, Copy, RotateCw, Trash2 } from "lucide-react";
import { useState } from "react";
import { requestAccessibilityPermission } from "tauri-plugin-macos-permissions-api";
import { useT } from "../i18n/context";
import { api, modelFor, type HistoryEntry } from "../lib/api";
import { describeProblem } from "../lib/problems";
import { keycaps } from "../lib/shortcut";
import { useParler } from "../state";
import { Button, IconButton, Keycaps } from "./ui";

export function Home({ openSettings }: { openSettings: () => void }) {
  return (
    <div className="flex flex-col gap-7 px-7 pt-2 pb-8">
      <Hero />
      <ProblemBanner openSettings={openSettings} />
      <Recent />
    </div>
  );
}

function Hero() {
  const { t } = useT();
  const { settings, status, info, provider } = useParler();
  const keys = keycaps(settings.shortcut, info.platform, t("common.space"));

  if (status.state === "recording") {
    return (
      <div className="rise flex min-h-[176px] flex-col justify-center gap-3">
        <div className="flex items-center gap-3">
          <span className="recording-light size-3.5 rounded-full bg-accent" />
          <h1 className="font-display text-[44px] leading-none text-ink">
            {t("home.listening")}
          </h1>
        </div>
        <p className="text-muted">{t("home.listeningHint")}</p>
      </div>
    );
  }

  if (status.state === "transcribing") {
    return (
      <div className="rise flex min-h-[176px] flex-col justify-center">
        <h1 className="font-display text-[44px] leading-none text-ink italic">
          {t("home.writing")}
        </h1>
      </div>
    );
  }

  const verb = settings.shortcutMode === "toggle" ? "home.press" : "home.hold";
  return (
    <div className="rise flex min-h-[176px] flex-col justify-center gap-4">
      <h1 className="font-display text-[44px] leading-[1.08] tracking-[-0.01em] text-ink">
        {t(verb)} <Keycaps keys={keys} size="lg" />
        <br />
        <span className="italic">{t("home.andSpeak")}</span>
      </h1>
      <p className="max-w-[34ch] text-[13.5px] text-muted">
        {t(`home.hint.${settings.shortcutMode}`)}{" "}
        {t(`home.hint.${settings.output}`)}
      </p>
      {provider && (
        <p className="font-mono text-[11.5px] tracking-tight text-muted/80">
          {provider.label} · {modelFor(settings, provider)}
        </p>
      )}
    </div>
  );
}

function ProblemBanner({ openSettings }: { openSettings: () => void }) {
  const { t } = useT();
  const { status } = useParler();
  if (!status.problem || status.state !== "idle") return null;
  const problem = describeProblem(status.problem);

  return (
    <div className="rise flex flex-col gap-3 rounded-2xl border border-accent/25 bg-accent-soft px-4 py-3.5">
      <p className="text-[13.5px] leading-snug text-ink">
        {t(problem.key, problem.params)}
      </p>
      <div className="flex flex-wrap gap-2">
        {status.canRetry && (
          <Button size="sm" variant="primary" onClick={() => api.retryDictation()}>
            <RotateCw className="size-3.5" />
            {t("problems.retry")}
          </Button>
        )}
        {problem.needsSettings && (
          <Button size="sm" variant="outline" onClick={openSettings}>
            {t("problems.openSettings")}
          </Button>
        )}
        {problem.needsAccessibility && (
          <Button
            size="sm"
            variant="outline"
            onClick={() => requestAccessibilityPermission()}
          >
            {t("problems.openAccessibility")}
          </Button>
        )}
      </div>
    </div>
  );
}

function Recent() {
  const { t, locale } = useT();
  const { history } = useParler();

  return (
    <section className="flex flex-col gap-2">
      <div className="flex items-baseline justify-between px-1">
        <h2 className="font-display text-[22px] text-ink">{t("home.recent")}</h2>
        {history.length > 0 && (
          <button
            type="button"
            onClick={() => api.clearHistory()}
            className="text-[12.5px] text-muted hover:text-ink"
          >
            {t("home.clear")}
          </button>
        )}
      </div>
      {history.length === 0 ? (
        <p className="rounded-2xl border border-dashed border-line-strong px-4 py-8 text-center text-[13px] text-muted">
          {t("home.empty")}
        </p>
      ) : (
        <ul className="flex flex-col gap-2">
          {history.map((entry, i) => (
            <Note key={entry.id} entry={entry} locale={locale} delay={i} />
          ))}
        </ul>
      )}
    </section>
  );
}

const relative = (locale: string, ms: number) => {
  const format = new Intl.RelativeTimeFormat(locale, { numeric: "auto" });
  const seconds = Math.round((ms - Date.now()) / 1000);
  const units: [Intl.RelativeTimeFormatUnit, number][] = [
    ["day", 86400],
    ["hour", 3600],
    ["minute", 60],
  ];
  for (const [unit, size] of units) {
    if (Math.abs(seconds) >= size) return format.format(Math.round(seconds / size), unit);
  }
  return format.format(0, "second");
};

function Note({
  entry,
  locale,
  delay,
}: {
  entry: HistoryEntry;
  locale: string;
  delay: number;
}) {
  const { t } = useT();
  const [copied, setCopied] = useState(false);

  const copy = async () => {
    await api.copyText(entry.text);
    setCopied(true);
    setTimeout(() => setCopied(false), 1400);
  };

  return (
    <li
      className="rise group relative rounded-2xl border border-line bg-raised/80 px-4 py-3 transition-colors hover:border-line-strong"
      style={{ animationDelay: `${Math.min(delay, 6) * 40}ms` }}
    >
      <p data-selectable className="line-clamp-4 pr-16 text-[14px] leading-relaxed text-ink">
        {entry.text}
      </p>
      <p className="mt-1.5 font-mono text-[11px] text-muted">
        {relative(locale, entry.id)} ·{" "}
        {t("home.seconds", {
          s: (entry.durationMs / 1000).toLocaleString(locale, {
            maximumFractionDigits: 1,
          }),
        })}
      </p>
      <div className="absolute top-2 right-2 flex gap-0.5 opacity-60 transition-opacity group-hover:opacity-100">
        <IconButton label={copied ? t("common.copied") : t("common.copy")} onClick={copy}>
          {copied ? <Check className="size-4 text-ok" /> : <Copy className="size-4" />}
        </IconButton>
        <IconButton
          label={t("common.delete")}
          onClick={() => api.deleteHistoryEntry(entry.id)}
        >
          <Trash2 className="size-4" />
        </IconButton>
      </div>
    </li>
  );
}
