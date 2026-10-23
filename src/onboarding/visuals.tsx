// The right-hand scenes of the first-run flow: each shows, rather than tells.
import { Check, KeyRound, Lightbulb, LoaderCircle, Mic, PersonStanding, Type, X } from "lucide-react";
import { useEffect, useState } from "react";
import { useT } from "../i18n/context";
import { lookOf } from "../lib/look";
import type { Check as KeyCheck } from "../components/useProviderForm";
import { FlowPill, IconTile, Keycaps, LogoMark } from "../ui";

/** Steps through `durations` (ms) in a loop; returns the current index. */
function useCycle(durations: number[]): number {
  const [index, setIndex] = useState(0);
  useEffect(() => {
    const timer = setTimeout(() => setIndex((i) => (i + 1) % durations.length), durations[index]);
    return () => clearTimeout(timer);
  }, [index, durations]);
  return index;
}

const WELCOME_CYCLE = [2600, 900, 3400];

/** A message written by voice: listen, write, done, again. */
export function WelcomeScene() {
  const { t } = useT();
  const phase = useCycle(WELCOME_CYCLE);
  const text = t("onboarding.welcome.demo");

  return (
    <div className="relative flex w-[360px] flex-col items-center">
      <div className="w-full rounded-[20px] bg-surface p-5 shadow-raised">
        <div className="mb-4 flex items-center gap-1.5">
          {["#ff5f57", "#febc2e", "#28c840"].map((c) => (
            <span key={c} className="size-2.5 rounded-full" style={{ background: c }} />
          ))}
          <span className="ml-2 text-[12px] font-medium text-text-3">
            {t("onboarding.welcome.document")}
          </span>
        </div>
        <div className="mb-3 h-2 w-24 rounded-full bg-text/8" />
        <p className="min-h-[72px] text-[16px] leading-relaxed text-text">
          {phase === 2 && <span className="rise inline">{text}</span>}
          <span className="caret ml-px inline-block h-[18px] w-[2px] translate-y-[3px] bg-accent" />
        </p>
        <div className="mt-2 flex flex-col gap-2">
          <div className="h-2 w-full rounded-full bg-text/6" />
          <div className="h-2 w-2/3 rounded-full bg-text/6" />
        </div>
      </div>
      <div className="mt-7 transition-all duration-300" style={{ opacity: phase === 2 ? 0 : 1 }}>
        <FlowPill state={phase === 0 ? "listening" : "writing"} />
      </div>
    </div>
  );
}

/** Voice → service → text, following the selection. */
export function FlowScene({ providerId, providerLabel }: { providerId: string; providerLabel: string }) {
  const { t } = useT();
  const look = lookOf(providerId);
  const node = "flex w-[250px] items-center gap-3 rounded-2xl bg-surface px-4 py-3.5 shadow-raised";
  const connector = (
    <svg width="2" height="54" className="overflow-visible" aria-hidden="true">
      <line x1="1" y1="0" x2="1" y2="54" stroke="var(--text-3)" strokeWidth="2" strokeDasharray="4 8" strokeLinecap="round" className="flow" />
    </svg>
  );

  return (
    <div className="flex flex-col items-center">
      <div className={node}>
        <IconTile icon={Mic} tint="red" />
        <span className="text-[14px] font-medium text-text">{t("onboarding.provider.voice")}</span>
        <span className="ml-auto flex h-5 items-center gap-[2px]">
          {[6, 12, 18, 10, 14].map((h, i) => (
            <span key={i} className="dance w-[3px] rounded-full bg-accent" style={{ height: h, animationDelay: `${i * 120}ms` }} />
          ))}
        </span>
      </div>
      {connector}
      <div key={providerId} className={`${node} rise`}>
        <IconTile icon={look.icon} tint={look.tint} />
        <span className="text-[14px] font-semibold text-text">{providerLabel}</span>
        <LoaderCircle className="ml-auto size-4 animate-spin text-text-3" />
      </div>
      {connector}
      <div className={node}>
        <IconTile icon={Type} tint="violet" />
        <span className="text-[14px] font-medium text-text">{t("onboarding.provider.text")}</span>
        <span className="ml-auto flex flex-col gap-1">
          <span className="h-1.5 w-12 rounded-full bg-text/12" />
          <span className="h-1.5 w-8 rounded-full bg-text/12" />
        </span>
      </div>
    </div>
  );
}

/** The key as a card, filling in as it is pasted and checked. */
export function KeyScene({
  providerId,
  providerLabel,
  apiKey,
  check,
}: {
  providerId: string;
  providerLabel: string;
  apiKey: string;
  check: KeyCheck;
}) {
  const { t } = useT();
  const look = lookOf(providerId);
  const trimmed = apiKey.trim();
  const masked =
    trimmed.length > 10 ? `${trimmed.slice(0, 4)}••••••••${trimmed.slice(-4)}` : "•••• •••• •••• ••••";

  return (
    <div className="grain relative h-[210px] w-[340px] overflow-hidden rounded-[22px] bg-ink p-6 text-on-ink shadow-raised">
      <div aria-hidden="true" className="pointer-events-none absolute -right-20 -bottom-24 size-64 rounded-full bg-[radial-gradient(circle,rgb(255_90_60/0.3),transparent_65%)]" />
      <div className="relative flex items-center justify-between">
        <span className="flex items-center gap-2.5">
          <IconTile icon={look.icon} tint={look.tint} size="sm" />
          <span className="text-[14px] font-semibold">{providerLabel}</span>
        </span>
        <KeyRound className="size-5 text-on-ink-2" />
      </div>
      <div className="relative mt-9">
        <div className="text-[11px] tracking-wide text-on-ink-2 uppercase">{t("onboarding.key.card")}</div>
        <div className={`mt-1.5 font-mono text-[19px] tracking-wider ${trimmed ? "text-on-ink" : "text-on-ink-2"}`}>
          {masked}
        </div>
      </div>
      <div className="relative mt-5 flex items-center gap-2 text-[12.5px] font-medium">
        {check.state === "checking" && (
          <><LoaderCircle className="size-4 animate-spin" />{t("onboarding.key.checking")}</>
        )}
        {check.state === "ok" && (
          <span className="rise flex items-center gap-1.5 text-[#6fd39a]"><Check className="size-4" />{t("onboarding.key.valid")}</span>
        )}
        {check.state === "failed" && (
          <span className="rise flex items-center gap-1.5 text-[#ff8a72]"><X className="size-4" />{t("overlay.unauthorized")}</span>
        )}
        {check.state === "idle" && <span className="text-on-ink-2">{t("onboarding.key.waiting")}</span>}
      </div>
    </div>
  );
}

/** A miniature of System Settings, switches mirroring the real state. */
export function PermissionsScene({ microphone, accessibility }: { microphone: boolean; accessibility: boolean }) {
  const { t } = useT();
  const rows = [
    { icon: Mic, label: t("onboarding.permissions.microphone"), on: microphone },
    { icon: PersonStanding, label: t("onboarding.permissions.accessibility"), on: accessibility },
  ];
  return (
    <div className="w-[360px] overflow-hidden rounded-[16px] bg-surface shadow-raised">
      <div className="flex items-center gap-1.5 border-b border-border px-4 py-3">
        {["#ff5f57", "#febc2e", "#28c840"].map((c) => (
          <span key={c} className="size-2.5 rounded-full" style={{ background: c }} />
        ))}
        <span className="ml-2 text-[12.5px] font-semibold text-text">{t("onboarding.permissions.panel")}</span>
      </div>
      <div className="flex flex-col gap-4 p-4">
        {rows.map(({ icon: Icon, label, on }) => (
          <div key={label}>
            <div className="mb-1.5 flex items-center gap-1.5 text-[11.5px] font-medium text-text-3">
              <Icon className="size-3.5" />
              {label}
            </div>
            <div
              className={`flex items-center gap-3 rounded-xl bg-surface-2 px-3 py-2.5 transition-shadow ${
                on ? "" : "shadow-[0_0_0_2px_var(--accent)]"
              }`}
            >
              <LogoMark size={22} />
              <span className="text-[13px] font-medium text-text">Parler</span>
              <span className={`relative ml-auto h-5 w-8 rounded-full transition-colors duration-300 ${on ? "bg-success" : "bg-text/15"}`}>
                <span className={`absolute top-0.5 left-0.5 size-4 rounded-full bg-white shadow transition-transform duration-300 ${on ? "translate-x-3" : ""}`} />
              </span>
            </div>
          </div>
        ))}
      </div>
    </div>
  );
}

const SHORTCUT_CYCLE = [1100, 2400, 1300];

/** The keys press down, the pill listens, the text arrives. */
export function ShortcutScene({ keys }: { keys: string[] }) {
  const { t } = useT();
  const phase = useCycle(SHORTCUT_CYCLE);
  const captions = [t("onboarding.shortcut.hold"), t("onboarding.shortcut.speak"), t("onboarding.shortcut.release")];
  return (
    <div className="flex flex-col items-center gap-9">
      <Keycaps keys={keys} size="xl" pressed={phase === 1} />
      <div className="h-11">
        {phase === 1 && <span className="rise inline-block"><FlowPill state="listening" /></span>}
        {phase === 2 && <span className="rise inline-block"><FlowPill state="writing" /></span>}
      </div>
      <div className="flex gap-3 font-display text-[22px]">
        {captions.map((caption, i) => (
          <span key={caption} className={`transition-colors duration-300 ${i === phase ? "text-text" : "text-text/20"}`}>
            {caption}
          </span>
        ))}
      </div>
    </div>
  );
}

/** Things worth saying the first time. */
export function IdeasScene() {
  const { t, locale } = useT();
  const [open, close] = locale === "fr" ? ["« ", " »"] : ["“", "”"];
  return (
    <div className="flex w-[360px] flex-col gap-3">
      <div className="mb-1 text-[11.5px] font-semibold tracking-wide text-text-3 uppercase">
        {t("onboarding.try.ideas")}
      </div>
      {["idea1", "idea2", "idea3"].map((idea, i) => (
        <div
          key={idea}
          className="rise rounded-2xl bg-surface px-5 py-4 font-display text-[19px] leading-snug text-text italic shadow-raised"
          style={{ animationDelay: `${150 + i * 120}ms` }}
        >
          {open}
          {t(`onboarding.try.${idea}`)}
          {close}
        </div>
      ))}
      <div className="mt-2 flex items-center gap-2 text-[13px] text-text-2">
        <Lightbulb className="size-4 text-warning" />
        {t("onboarding.try.tip")}
      </div>
    </div>
  );
}
