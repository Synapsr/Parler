import { Check, Copy, Gauge, RotateCw, Timer, Trash2, Type } from "lucide-react";
import { useState } from "react";
import { requestAccessibilityPermission } from "tauri-plugin-macos-permissions-api";
import { useT } from "../i18n/context";
import { api, type HistoryEntry } from "../lib/api";
import {
  dayLabel,
  formatDuration,
  groupByDay,
  minutesSaved,
  wordsPerMinute,
} from "../lib/insights";
import { describeProblem } from "../lib/problems";
import { keycaps } from "../lib/shortcut";
import { useParler } from "../state";
import {
  Banner,
  Button,
  Card,
  FlowPill,
  IconButton,
  Keycaps,
  PageHeader,
  Stat,
  VoiceBars,
} from "../ui";

export function Home({ openSettings }: { openSettings: () => void }) {
  const { t } = useT();
  const hour = new Date().getHours();
  const greeting =
    hour < 12 ? t("home.morning") : hour < 18 ? t("home.afternoon") : t("home.evening");

  return (
    <div className="flex flex-col gap-5">
      <PageHeader title={greeting} subtitle={t("home.subtitle")} />
      <Hero />
      <Problem openSettings={openSettings} />
      <Stats />
      <History />
    </div>
  );
}

/** The one thing to remember, on an ink card. */
function Hero() {
  const { t } = useT();
  const { settings, status, info } = useParler();
  const keys = keycaps(settings.shortcut, info.platform, t("common.space"));
  const verb = settings.shortcutMode === "toggle" ? t("home.press") : t("home.hold");

  let title;
  let hint;
  let pill: "idle" | "listening" | "writing" = "idle";
  if (status.state === "recording") {
    title = t("home.listening");
    hint = t("home.listeningHint");
    pill = "listening";
  } else if (status.state === "transcribing") {
    title = t("home.writing");
    hint = " ";
    pill = "writing";
  } else {
    title = (
      <>
        {verb} <Keycaps keys={keys} size="lg" dark /> <span className="italic">{t("home.andSpeak")}</span>
      </>
    );
    hint = `${t(`home.hint.${settings.shortcutMode}`)} ${t(`home.hint.${settings.output}`)}`;
  }

  return (
    <section
      className="rise grain relative overflow-hidden rounded-[22px] bg-ink px-7 pt-7 pb-6 text-on-ink shadow-raised"
      style={{ animationDelay: "60ms" }}
    >
      <div
        aria-hidden="true"
        className="pointer-events-none absolute -top-24 -right-16 size-72 rounded-full bg-[radial-gradient(circle,rgb(255_90_60/0.28),transparent_65%)]"
      />
      <h2 className="relative font-display text-[34px] leading-[1.15] text-on-ink">{title}</h2>
      <div className="relative mt-5 flex items-end justify-between gap-4">
        <p className="max-w-[40ch] text-[13px] leading-relaxed text-on-ink-2">{hint}</p>
        <FlowPill state={pill} />
      </div>
    </section>
  );
}

function Problem({ openSettings }: { openSettings: () => void }) {
  const { t } = useT();
  const { status } = useParler();
  if (!status.problem || status.state !== "idle") return null;
  const problem = describeProblem(status.problem);

  return (
    <Banner
      tone="error"
      actions={
        <>
          {status.canRetry && (
            <Button size="sm" variant="primary" onClick={() => api.retryDictation()}>
              <RotateCw className="size-3.5" />
              {t("problems.retry")}
            </Button>
          )}
          {problem.needsSettings && (
            <Button size="sm" onClick={openSettings}>
              {t("problems.openSettings")}
            </Button>
          )}
          {problem.needsAccessibility && (
            <Button size="sm" onClick={() => requestAccessibilityPermission()}>
              {t("problems.openAccessibility")}
            </Button>
          )}
        </>
      }
    >
      {t(problem.key, problem.params)}
    </Banner>
  );
}

function Stats() {
  const { t, locale } = useT();
  const { stats } = useParler();
  const number = new Intl.NumberFormat(locale);
  const speed = wordsPerMinute(stats);
  return (
    <div className="rise grid grid-cols-3 gap-3" style={{ animationDelay: "120ms" }}>
      <Stat icon={Type} tint="violet" value={number.format(stats.words)} label={t("home.stats.words")} />
      <Stat icon={Timer} tint="green" value={formatDuration(minutesSaved(stats))} label={t("home.stats.saved")} />
      <Stat icon={Gauge} tint="amber" value={speed ? number.format(speed) : "—"} label={t("home.stats.speed")} />
    </div>
  );
}

function History() {
  const { t, locale } = useT();
  const { history } = useParler();
  const now = Date.now();

  return (
    <section className="rise mt-3 flex flex-col gap-3" style={{ animationDelay: "180ms" }}>
      <div className="flex items-baseline justify-between">
        <h2 className="font-display text-heading text-text">{t("home.history")}</h2>
        {history.length > 0 && (
          <Button size="sm" variant="ghost" onClick={() => api.clearHistory()}>
            {t("home.clear")}
          </Button>
        )}
      </div>

      {history.length === 0 ? (
        <Card className="flex flex-col items-center gap-3 py-10 text-center">
          <VoiceBars className="text-text-3" />
          <div>
            <p className="text-[14px] font-medium text-text">{t("home.empty")}</p>
            <p className="mt-1 text-[13px] text-text-3">{t("home.emptyHint")}</p>
          </div>
        </Card>
      ) : (
        groupByDay(history).map((day) => (
          <div key={day.start} className="flex flex-col gap-2">
            <h3 className="px-1 text-[11.5px] font-semibold tracking-wide text-text-3 uppercase">
              {dayLabel(day.start, now, locale)}
            </h3>
            <Card padded={false} className="divide-y divide-border overflow-hidden">
              {day.entries.map((entry) => (
                <Row key={entry.id} entry={entry} locale={locale} />
              ))}
            </Card>
          </div>
        ))
      )}
    </section>
  );
}

function Row({ entry, locale }: { entry: HistoryEntry; locale: string }) {
  const { t } = useT();
  const [copied, setCopied] = useState(false);
  const time = new Intl.DateTimeFormat(locale, { hour: "2-digit", minute: "2-digit" }).format(entry.id);

  const copy = async () => {
    await api.copyText(entry.text);
    setCopied(true);
    setTimeout(() => setCopied(false), 1400);
  };

  return (
    <div className="group flex items-start gap-4 px-4 py-3.5 transition-colors hover:bg-surface-2">
      <span className="w-16 shrink-0 pt-px font-mono text-[11.5px] whitespace-nowrap text-text-3">{time}</span>
      <p data-selectable className="min-w-0 flex-1 text-[14px] leading-relaxed text-text">
        {entry.text}
      </p>
      <div className="-my-1 flex shrink-0 gap-0.5 opacity-0 transition-opacity group-hover:opacity-100 focus-within:opacity-100">
        <IconButton label={copied ? t("common.copied") : t("common.copy")} onClick={copy}>
          {copied ? <Check className="size-4 text-success" /> : <Copy className="size-4" />}
        </IconButton>
        <IconButton label={t("common.delete")} onClick={() => api.deleteHistoryEntry(entry.id)}>
          <Trash2 className="size-4" />
        </IconButton>
      </div>
    </div>
  );
}
