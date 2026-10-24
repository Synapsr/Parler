import { ArrowRight, Gauge, RotateCw, Timer, Type } from "lucide-react";
import { requestAccessibilityPermission } from "tauri-plugin-macos-permissions-api";
import { joinSentences } from "../i18n";
import { useT } from "../i18n/context";
import { EmptyHistory, HistoryDays } from "../components/HistoryList";
import { KeysSentence } from "../components/KeysSentence";
import { useKeycaps } from "../components/useKeycaps";
import { api } from "../lib/api";
import { formatDuration, minutesSaved, wordsPerMinute } from "../lib/insights";
import { describeProblem } from "../lib/problems";
import { useParler } from "../state";
import { Banner, Button, FlowPill, Keycaps, PageHeader, Stat } from "../ui";

/** Dictations shown on the home page. */
const RECENT = 5;

export function Home({
  openSettings,
  openHistory,
}: {
  openSettings: () => void;
  openHistory: () => void;
}) {
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
      <Recent openHistory={openHistory} />
    </div>
  );
}

/** The one thing to remember, on an ink card. */
function Hero() {
  const { t, locale } = useT();
  const { settings, status } = useParler();
  const keys = useKeycaps()(settings.shortcut);

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
      <KeysSentence
        text={t(settings.shortcutMode === "toggle" ? "home.pressToSpeak" : "home.holdToSpeak")}
        keys={<Keycaps keys={keys} size="lg" dark />}
      />
    );
    hint = joinSentences(locale, t(`home.hint.${settings.shortcutMode}`), t(`home.hint.${settings.output}`));
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
  const problem = describeProblem(status.problem, t("providers.customName"));

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
      <Stat icon={Timer} tint="green" value={formatDuration(minutesSaved(stats), locale)} label={t("home.stats.saved")} />
      <Stat icon={Gauge} tint="amber" value={speed ? number.format(speed) : "—"} label={t("home.stats.speed")} />
    </div>
  );
}

/** The last few dictations; the History page has them all. */
function Recent({ openHistory }: { openHistory: () => void }) {
  const { t } = useT();
  const { history } = useParler();

  return (
    <section className="rise mt-3 flex flex-col gap-3" style={{ animationDelay: "180ms" }}>
      <div className="flex items-baseline justify-between">
        <h2 className="font-display text-heading text-text">{t("home.recent")}</h2>
        {history.length > 0 && (
          <Button size="sm" variant="ghost" onClick={openHistory}>
            {t("home.seeAll")}
            <ArrowRight className="size-3.5" />
          </Button>
        )}
      </div>
      {history.length === 0 ? (
        <EmptyHistory>
          <p className="text-[14px] font-medium text-text">{t("home.empty")}</p>
          <p className="mt-1 text-[13px] text-text-3">{t("home.emptyHint")}</p>
        </EmptyHistory>
      ) : (
        <HistoryDays entries={history.slice(0, RECENT)} />
      )}
    </section>
  );
}
