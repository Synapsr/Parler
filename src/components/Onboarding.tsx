import { ArrowLeft, Check } from "lucide-react";
import { useEffect, useState } from "react";
import {
  checkAccessibilityPermission,
  checkMicrophonePermission,
  requestAccessibilityPermission,
  requestMicrophonePermission,
} from "tauri-plugin-macos-permissions-api";
import { useT } from "../i18n/context";
import { describeProblem } from "../lib/problems";
import { keycaps } from "../lib/shortcut";
import { useParler } from "../state";
import { ProviderSetup } from "./ProviderSetup";
import { ShortcutField } from "./ShortcutField";
import { Button, Keycaps } from "./ui";

type Step = "provider" | "key" | "permissions" | "try";

export function Onboarding() {
  const { t } = useT();
  const { info } = useParler();
  const steps: Step[] =
    info.platform === "macos"
      ? ["provider", "key", "permissions", "try"]
      : ["provider", "key", "try"];
  const [index, setIndex] = useState(0);
  const step = steps[index];
  const next = () => setIndex((i) => Math.min(i + 1, steps.length - 1));
  const back = () => setIndex((i) => Math.max(i - 1, 0));

  return (
    <div className="flex min-h-full flex-col px-7 pt-2 pb-8">
      <div className="mb-8 flex items-center gap-3">
        {index > 0 ? (
          <button
            type="button"
            onClick={back}
            aria-label={t("common.back")}
            className="-ml-2 flex size-8 items-center justify-center rounded-full text-muted hover:bg-ink/5 hover:text-ink"
          >
            <ArrowLeft className="size-4" />
          </button>
        ) : null}
        <div className="flex flex-1 gap-1.5">
          {steps.map((s, i) => (
            <span
              key={s}
              className={`h-1 flex-1 rounded-full transition-colors duration-500 ${
                i <= index ? "bg-accent" : "bg-ink/10"
              }`}
            />
          ))}
        </div>
        <span className="font-mono text-[11px] text-muted">
          {t("onboarding.step", { n: index + 1, total: steps.length })}
        </span>
      </div>

      <div key={step} className="flex flex-1 flex-col">
        {step === "provider" && <ProviderStep onNext={next} />}
        {step === "key" && <KeyStep onNext={next} />}
        {step === "permissions" && <PermissionsStep onNext={next} />}
        {step === "try" && <TryStep />}
      </div>
    </div>
  );
}

function Title({ title, subtitle }: { title: string; subtitle: string }) {
  return (
    <div className="rise mb-6">
      <h1 className="font-display text-[38px] leading-[1.05] text-ink">{title}</h1>
      <p className="mt-2.5 max-w-[38ch] text-[14px] text-muted">{subtitle}</p>
    </div>
  );
}

function ProviderStep({ onNext }: { onNext: () => void }) {
  const { t } = useT();
  const { settings, providers, update } = useParler();

  return (
    <>
      <p className="rise mb-1 font-display text-[19px] text-accent italic">
        {t("app.tagline")}
      </p>
      <Title
        title={t("onboarding.provider.title")}
        subtitle={t("onboarding.provider.subtitle")}
      />
      <div className="flex flex-col gap-2" role="radiogroup">
        {providers.map((p, i) => {
          const selected = p.id === settings.provider;
          return (
            <button
              key={p.id}
              type="button"
              role="radio"
              aria-checked={selected}
              onClick={() => update({ provider: p.id })}
              style={{ animationDelay: `${80 + i * 50}ms` }}
              className={`rise flex items-center justify-between rounded-2xl border px-4 py-3.5 text-left transition-all duration-200 ${
                selected
                  ? "border-ink/50 bg-raised shadow-[0_1px_0_var(--line-strong),0_8px_24px_-12px_rgb(0_0_0/0.25)]"
                  : "border-line bg-raised/50 hover:border-line-strong"
              }`}
            >
              <span>
                <span className="block font-display text-[22px] leading-tight text-ink">
                  {p.id === "custom" ? t("onboarding.key.customTitle") : p.label}
                </span>
                <span className="text-[13px] text-muted">{t(`providers.${p.id}`)}</span>
              </span>
              <span
                className={`flex size-5 items-center justify-center rounded-full border transition-colors ${
                  selected ? "border-accent bg-accent text-accent-ink" : "border-line-strong"
                }`}
              >
                {selected && <Check className="size-3" strokeWidth={3} />}
              </span>
            </button>
          );
        })}
      </div>
      <div className="mt-auto pt-8">
        <Button variant="primary" size="lg" onClick={onNext} className="w-full">
          {t("common.continue")}
        </Button>
      </div>
    </>
  );
}

function KeyStep({ onNext }: { onNext: () => void }) {
  const { t } = useT();
  const { provider } = useParler();
  if (!provider) return null;
  const custom = provider.id === "custom";

  return (
    <>
      <Title
        title={
          custom
            ? t("onboarding.key.customTitle")
            : t("onboarding.key.title", { provider: provider.label })
        }
        subtitle={
          custom
            ? t("onboarding.key.customSubtitle")
            : t("onboarding.key.subtitle", { provider: provider.label })
        }
      />
      <div className="rise" style={{ animationDelay: "80ms" }}>
        <ProviderSetup verifyLabel={t("onboarding.key.verify")} onVerified={onNext} />
      </div>
    </>
  );
}

type Permission = "microphone" | "accessibility";

function PermissionsStep({ onNext }: { onNext: () => void }) {
  const { t } = useT();
  const [granted, setGranted] = useState<Record<Permission, boolean>>({
    microphone: false,
    accessibility: false,
  });

  // The user flips these in System Settings; keep checking.
  useEffect(() => {
    const check = async () => {
      const [microphone, accessibility] = await Promise.all([
        checkMicrophonePermission(),
        checkAccessibilityPermission(),
      ]);
      setGranted({ microphone, accessibility });
    };
    check();
    const timer = setInterval(check, 1000);
    return () => clearInterval(timer);
  }, []);

  const request: Record<Permission, () => Promise<unknown>> = {
    microphone: requestMicrophonePermission,
    accessibility: requestAccessibilityPermission,
  };
  const all = granted.microphone && granted.accessibility;

  return (
    <>
      <Title
        title={t("onboarding.permissions.title")}
        subtitle={t("onboarding.permissions.subtitle")}
      />
      <div className="flex flex-col gap-2">
        {(["microphone", "accessibility"] as Permission[]).map((p, i) => (
          <div
            key={p}
            style={{ animationDelay: `${80 + i * 60}ms` }}
            className="rise flex items-center justify-between gap-4 rounded-2xl border border-line bg-raised/70 px-4 py-3.5"
          >
            <div>
              <div className="font-display text-[21px] leading-tight text-ink">
                {t(`onboarding.permissions.${p}`)}
              </div>
              <div className="text-[13px] text-muted">
                {t(`onboarding.permissions.${p}Hint`)}
              </div>
            </div>
            {granted[p] ? (
              <span className="inline-flex items-center gap-1 text-[13px] font-medium text-ok">
                <Check className="size-4" />
                {t("onboarding.permissions.allowed")}
              </span>
            ) : (
              <Button size="sm" variant="accent" onClick={() => request[p]()}>
                {t("onboarding.permissions.allow")}
              </Button>
            )}
          </div>
        ))}
      </div>
      <div className="mt-auto flex flex-col gap-2 pt-8">
        <Button variant="primary" size="lg" onClick={onNext} className="w-full">
          {all ? t("common.continue") : t("onboarding.permissions.later")}
        </Button>
      </div>
    </>
  );
}

function TryStep() {
  const { t } = useT();
  const { settings, info, status, update } = useParler();
  const [text, setText] = useState("");
  const done = text.trim().length > 0;
  const problem =
    status.problem && status.state === "idle" ? describeProblem(status.problem) : null;
  const verb = settings.shortcutMode === "toggle" ? "home.press" : "home.hold";

  return (
    <>
      <div className="rise mb-6">
        <h1 className="font-display text-[38px] leading-[1.05] text-ink">
          {t("onboarding.try.title")}
        </h1>
        <p className="mt-3 text-[15px] leading-relaxed text-ink-soft">
          {t("onboarding.try.lead")} {t(verb).toLowerCase()}{" "}
          <Keycaps keys={keycaps(settings.shortcut, info.platform, t("common.space"))} />{" "}
          {t("home.andSpeak")}
        </p>
      </div>
      <textarea
        autoFocus
        value={text}
        onChange={(e) => setText(e.target.value)}
        placeholder={t("onboarding.try.placeholder")}
        className={`rise min-h-36 w-full resize-none rounded-2xl border bg-raised px-4 py-3.5 font-display text-[22px] leading-snug text-ink placeholder:text-muted/60 focus:outline-none ${
          status.state === "recording"
            ? "border-accent shadow-[0_0_0_4px_var(--accent-soft)]"
            : "border-line-strong focus:border-ink/40"
        }`}
        style={{ animationDelay: "80ms" }}
      />
      <div className="mt-3 flex items-center justify-between">
        <span className="text-[12.5px] text-muted">{t("settings.shortcut.label")}</span>
        <ShortcutField />
      </div>
      {problem && (
        <p className="rise mt-4 rounded-xl bg-accent-soft px-3.5 py-2.5 text-[13px] text-ink">
          {t(problem.key, problem.params)}
        </p>
      )}
      <div className="mt-auto flex flex-col items-center gap-3 pt-8">
        {done && (
          <p className="rise flex items-center gap-1.5 text-[14px] font-medium text-ok">
            <Check className="size-4" />
            {t("onboarding.try.done")}
          </p>
        )}
        <Button
          variant={done ? "accent" : "primary"}
          size="lg"
          className="w-full"
          onClick={() => update({ onboarded: true })}
        >
          {done ? t("onboarding.try.finish") : t("onboarding.try.skip")}
        </Button>
      </div>
    </>
  );
}
