import { ArrowLeft, ArrowRight, ArrowUpRight, Check, Clock, LoaderCircle, Mic, PersonStanding } from "lucide-react";
import { openUrl } from "@tauri-apps/plugin-opener";
import { useEffect, useState, type ReactNode } from "react";
import {
  checkAccessibilityPermission,
  checkMicrophonePermission,
  requestAccessibilityPermission,
  requestMicrophonePermission,
} from "tauri-plugin-macos-permissions-api";
import { AccountStatus, PlanButton, SignInWaiting, signInTrouble } from "../components/Cloud";
import { KeysSentence } from "../components/KeysSentence";
import { ShortcutField } from "../components/ShortcutField";
import { useKeycaps } from "../components/useKeycaps";
import { useCloudSignIn } from "../components/useCloudSignIn";
import { useProviderForm } from "../components/useProviderForm";
import { useT } from "../i18n/context";
import { CLOUD } from "../lib/api";
import { needsPlan } from "../lib/cloud";
import { lookOf } from "../lib/look";
import { describeProblem } from "../lib/problems";
import { useParler } from "../state";
import {
  Banner,
  Button,
  ChoiceCard,
  IconTile,
  Input,
  Keycaps,
  LogoMark,
  StepDots,
  TextArea,
  Wordmark,
} from "../ui";
import {
  FlowScene,
  IdeasScene,
  KeyScene,
  PermissionsScene,
  ShortcutScene,
  SignInScene,
  WelcomeScene,
} from "./visuals";

type Step = "welcome" | "provider" | "key" | "permissions" | "shortcut" | "try";

interface Nav {
  index: number;
  count: number;
  next: () => void;
  back?: () => void;
}

export function Onboarding() {
  const { info, settings } = useParler();
  const steps: Step[] = [
    "welcome",
    "provider",
    "key",
    ...(info.platform === "macos" ? (["permissions"] as Step[]) : []),
    "shortcut",
    "try",
  ];
  const [index, setIndex] = useState(0);
  const nav: Nav = {
    index,
    count: steps.length,
    next: () => setIndex((i) => Math.min(i + 1, steps.length - 1)),
    back: index > 0 ? () => setIndex((i) => i - 1) : undefined,
  };

  switch (steps[index]) {
    case "welcome":
      return <WelcomeStep nav={nav} />;
    case "provider":
      return <ProviderStep nav={nav} />;
    case "key":
      // Parler Cloud needs no key: the person signs in instead.
      return settings.provider === CLOUD ? <SignInStep nav={nav} /> : <KeyStep nav={nav} />;
    case "permissions":
      return <PermissionsStep nav={nav} />;
    case "shortcut":
      return <ShortcutStep nav={nav} />;
    case "try":
      return <TryStep nav={nav} />;
  }
}

/** Words on the left, the scene that shows them on the right. */
function StepLayout({
  nav,
  scene,
  footer,
  children,
}: {
  nav: Nav;
  scene: ReactNode;
  footer: ReactNode;
  children: ReactNode;
}) {
  const { t } = useT();
  const { info } = useParler();
  return (
    <div className="grain relative grid h-full grid-cols-[minmax(400px,46%)_1fr]">
      <section className="relative z-10 flex min-h-0 flex-col px-11 pb-9">
        <div data-tauri-drag-region className={info.platform === "macos" ? "h-[52px]" : "h-7"} />
        <div data-tauri-drag-region className="flex items-center justify-between">
          <span className="flex items-center gap-2.5">
            <LogoMark size={28} />
            <Wordmark />
          </span>
          <StepDots count={nav.count} current={nav.index} />
        </div>
        {/* Centred while it fits, scrolled from the top when it doesn't. */}
        <div key={nav.index} className="flex min-h-0 flex-1 flex-col overflow-y-auto py-6">
          <div className="my-auto">{children}</div>
        </div>
        <div className="flex items-center gap-3">
          {nav.back && (
            <Button variant="ghost" onClick={nav.back}>
              <ArrowLeft className="size-4" />
              {t("common.back")}
            </Button>
          )}
          <div className="flex-1" />
          {footer}
        </div>
      </section>
      <section className="relative py-3 pr-3">
        <div
          data-tauri-drag-region
          className="stage relative flex h-full items-center justify-center overflow-hidden rounded-[26px]"
        >
          <div key={nav.index} className="rise">
            {scene}
          </div>
        </div>
      </section>
    </div>
  );
}

function Title({ title, subtitle }: { title: ReactNode; subtitle: ReactNode }) {
  return (
    <div className="rise mb-7">
      <h1 className="font-display text-title text-text">{title}</h1>
      <p className="mt-3 max-w-[40ch] text-[14.5px] leading-relaxed text-text-2">{subtitle}</p>
    </div>
  );
}

/** "Your voice, typed anywhere." with the second half in italics. */
function emphasize(text: string): ReactNode {
  const cut = text.indexOf(", ");
  if (cut < 0) return text;
  return (
    <>
      {text.slice(0, cut + 1)} <em>{text.slice(cut + 2)}</em>
    </>
  );
}

function WelcomeStep({ nav }: { nav: Nav }) {
  const { t } = useT();
  return (
    <StepLayout
      nav={nav}
      scene={<WelcomeScene />}
      footer={
        <Button variant="primary" size="lg" onClick={nav.next}>
          {t("onboarding.welcome.start")}
          <ArrowRight className="size-4" />
        </Button>
      }
    >
      <h1 className="rise font-display text-hero text-text">{emphasize(t("onboarding.welcome.title"))}</h1>
      <p className="rise mt-5 max-w-[38ch] text-[15px] leading-relaxed text-text-2" style={{ animationDelay: "80ms" }}>
        {t("onboarding.welcome.subtitle")}
      </p>
      <p className="rise mt-7 flex items-center gap-2 text-[13px] text-text-3" style={{ animationDelay: "160ms" }}>
        <Clock className="size-4" />
        {t("onboarding.welcome.time")}
      </p>
    </StepLayout>
  );
}

function ProviderStep({ nav }: { nav: Nav }) {
  const { t } = useT();
  const { settings, providers, provider, update } = useParler();
  // Parler Cloud is the suggested path; the services that take your own key
  // wait behind a link, as they were.
  const [own, setOwn] = useState(settings.provider !== CLOUD);
  const name = (id: string, label: string) => (id === "custom" ? t("providers.customName") : label);
  const others = providers.filter((p) => p.id !== CLOUD);
  const cloud = providers.find((p) => p.id === CLOUD);

  const chooseOwn = () => {
    setOwn(true);
    if (settings.provider === CLOUD && others[0]) update({ provider: others[0].id });
  };
  const chooseCloud = () => {
    setOwn(false);
    update({ provider: CLOUD });
  };
  const link = "inline-flex items-center gap-1.5 text-[13px] font-medium text-text-2 hover:text-text";

  return (
    <StepLayout
      nav={nav}
      scene={<FlowScene providerId={settings.provider} providerLabel={name(settings.provider, provider?.label ?? "")} />}
      footer={
        <Button variant="primary" size="lg" onClick={nav.next}>
          {own || !cloud ? t("common.continue") : t("onboarding.provider.startTrial")}
          <ArrowRight className="size-4" />
        </Button>
      }
    >
      {own || !cloud ? (
        <div key="own">
          <Title title={t("onboarding.provider.title")} subtitle={t("onboarding.provider.subtitle")} />
          <div className="flex flex-col gap-1.5" role="radiogroup">
            {others.map((p, i) => {
              const look = lookOf(p.id);
              return (
                <ChoiceCard
                  key={p.id}
                  icon={look.icon}
                  tint={look.tint}
                  title={name(p.id, p.label)}
                  description={t(`providers.${p.id}`)}
                  badge={p.id === "groq" ? t("providers.recommended") : undefined}
                  selected={p.id === settings.provider}
                  onSelect={() => update({ provider: p.id })}
                  delay={60 + i * 50}
                />
              );
            })}
          </div>
          {cloud && (
            <button type="button" onClick={chooseCloud} className={`rise mt-4 ${link}`} style={{ animationDelay: "260ms" }}>
              <ArrowLeft className="size-3.5" />
              {t("onboarding.provider.useCloud")}
            </button>
          )}
        </div>
      ) : (
        <div key="cloud">
          <Title title={t("onboarding.provider.cloudTitle")} subtitle={t("onboarding.provider.cloudSubtitle")} />
          <CloudOffer label={cloud.label} />
          <button type="button" onClick={chooseOwn} className={`rise mt-5 ${link}`} style={{ animationDelay: "160ms" }}>
            {t("onboarding.provider.ownKey")}
            <ArrowRight className="size-3.5" />
          </button>
        </div>
      )}
    </StepLayout>
  );
}

/** Parler Cloud, chosen for you: what it is, and the trial first. */
function CloudOffer({ label }: { label: string }) {
  const { t } = useT();
  const look = lookOf(CLOUD);
  return (
    <div
      className="rise rounded-card bg-surface p-5 shadow-[0_0_0_2px_var(--text),0_10px_24px_-14px_rgb(0_0_0/0.35)]"
      style={{ animationDelay: "60ms" }}
    >
      <div className="flex items-center gap-3.5">
        <IconTile icon={look.icon} tint={look.tint} />
        <div className="min-w-0 flex-1">
          <div className="flex items-center gap-2 text-[14.5px] font-semibold text-text">
            {label}
            <span className="rounded-full bg-accent-soft px-2 py-0.5 text-[11px] font-medium text-accent-text">
              {t("providers.recommended")}
            </span>
          </div>
          <div className="text-[12.5px] text-text-3">{t("providers.cloud")}</div>
        </div>
      </div>
      <div className="mt-4 border-t border-border pt-4">
        <div className="font-display text-[30px] leading-none text-text">{t("onboarding.provider.trial")}</div>
        <p className="mt-2 text-[13.5px] text-text-2">{t("onboarding.provider.trialThen")}</p>
        <p className="mt-1 text-[12.5px] text-text-3">{t("onboarding.provider.trialTerms")}</p>
      </div>
    </div>
  );
}

function KeyStep({ nav }: { nav: Nav }) {
  const { t } = useT();
  const form = useProviderForm();
  const provider = form.provider;
  if (!provider) return null;
  const custom = provider.id === "custom";
  const label = custom ? t("providers.customName") : provider.label;

  const proceed = async () => {
    if (form.check.state === "ok" || (await form.verify())) nav.next();
  };

  return (
    <StepLayout
      nav={nav}
      scene={<KeyScene providerId={provider.id} providerLabel={label} apiKey={form.key} check={form.check} />}
      footer={
        <Button
          variant="primary"
          size="lg"
          onClick={proceed}
          busy={form.check.state === "checking"}
          disabled={form.needsKey}
        >
          {t("common.continue")}
          <ArrowRight className="size-4" />
        </Button>
      }
    >
      <Title
        title={custom ? t("onboarding.key.customTitle") : t("onboarding.key.title", { provider: label })}
        subtitle={custom ? t("onboarding.key.customSubtitle") : t("onboarding.key.subtitle", { provider: label })}
      />

      {custom ? (
        <div className="mb-3">
          <Input
            value={form.baseUrl}
            onChange={(e) => form.setBaseUrl(e.target.value)}
            onBlur={() => form.save()}
            placeholder="http://localhost:8000/v1"
            className="font-mono text-[13px]"
          />
        </div>
      ) : (
        <ol className="rise mb-5 flex flex-col gap-3" style={{ animationDelay: "60ms" }}>
          {(["step1", "step2", "step3"] as const).map((step, i) => (
            <li key={step} className="flex items-center gap-3 text-[14px] text-text">
              <span className="flex size-6 shrink-0 items-center justify-center rounded-full bg-text text-[12px] font-semibold text-surface">
                {i + 1}
              </span>
              <span className="flex-1">{t(`onboarding.key.${step}`, { provider: label })}</span>
              {i === 0 && provider.apiKeyUrl && (
                <Button size="sm" onClick={() => openUrl(provider.apiKeyUrl!)}>
                  {t("onboarding.key.open", { provider: label })}
                  <ArrowUpRight className="size-3.5" />
                </Button>
              )}
            </li>
          ))}
        </ol>
      )}

      <div className="rise flex flex-col gap-3" style={{ animationDelay: "120ms" }}>
        <Input
          type="password"
          value={form.key}
          onChange={(e) => form.setKey(e.target.value)}
          onPaste={(e) => {
            // A pasted key is checked right away.
            const pasted = e.clipboardData.getData("text").trim();
            if (pasted) {
              e.preventDefault();
              form.setKey(pasted);
              form.verify({ key: pasted });
            }
          }}
          onBlur={() => form.save()}
          placeholder={custom ? t("settings.transcription.apiKeyOptional") : t("onboarding.key.placeholder")}
          className="h-12 font-mono text-[14px]"
          autoFocus={!custom}
        />
        {form.check.state === "ok" && <Banner tone="success">{t("onboarding.key.valid")}</Banner>}
        {form.check.state === "failed" && <Banner tone="error">{form.check.message}</Banner>}
      </div>
    </StepLayout>
  );
}

/** Parler Cloud: sign in with the browser instead of pasting a key. */
function SignInStep({ nav }: { nav: Nav }) {
  const { t } = useT();
  const { cloud, providers } = useParler();
  const signIn = useCloudSignIn();
  const { state } = signIn;
  const account = state.phase === "approved" ? state.account : cloud.account;
  const connected = cloud.signedIn || state.phase === "approved";
  const trouble = signInTrouble(signIn, t);

  // Signed in with a plan: carry on once the person has seen it worked.
  useEffect(() => {
    if (state.phase !== "approved" || needsPlan(state.account)) return;
    const timer = setTimeout(nav.next, 2600);
    return () => clearTimeout(timer);
  }, [state]);

  const cloudUrl = providers.find((p) => p.id === CLOUD)?.baseUrl ?? "https://cloud.parler.synapsr.io";
  const host = (url: string) => {
    try {
      return new URL(url).host;
    } catch {
      return url;
    }
  };
  const begun = state.phase === "waiting" || state.phase === "approved" ? state.start : null;
  const scene = (
    <SignInScene
      stage={connected ? "linked" : begun ? "waiting" : "idle"}
      code={begun?.userCode}
      deviceName={begun?.deviceName}
      host={host(begun?.verificationUrl ?? cloudUrl)}
    />
  );

  let footer;
  if (connected) {
    footer = (
      <Button variant="primary" size="lg" onClick={nav.next}>
        {t("common.continue")}
        <ArrowRight className="size-4" />
      </Button>
    );
  } else if (state.phase === "waiting") {
    footer = (
      <Button variant="primary" size="lg" disabled>
        {t("common.continue")}
        <ArrowRight className="size-4" />
      </Button>
    );
  } else {
    footer = (
      <Button variant="primary" size="lg" onClick={signIn.start} busy={state.phase === "starting"}>
        {trouble ? t("problems.retry") : t("cloud.start")}
        <ArrowUpRight className="size-4" />
      </Button>
    );
  }

  return (
    <StepLayout nav={nav} scene={scene} footer={footer}>
      <Title title={t("onboarding.signIn.title")} subtitle={t("onboarding.signIn.subtitle")} />

      {connected ? (
        account ? (
          <div className="rise">
            <Banner
              tone={needsPlan(account) ? "warning" : "success"}
              actions={needsPlan(account) ? <PlanButton account={account} /> : undefined}
            >
              <span className="flex flex-col gap-1">
                <span className="font-medium">{t("cloud.connected", { email: account.email })}</span>
                <AccountStatus account={account} />
              </span>
            </Banner>
          </div>
        ) : (
          // Offline, the account can wait: being signed in is what counts here.
          cloud.loading && (
            <p className="flex items-center gap-2 text-[13.5px] text-text-3" role="status">
              <LoaderCircle className="size-4 animate-spin" />
              {t("cloud.loading")}
            </p>
          )
        )
      ) : state.phase === "waiting" ? (
        <SignInWaiting signIn={signIn} />
      ) : (
        <div className="flex flex-col gap-5">
          <ol className="rise flex flex-col gap-3" style={{ animationDelay: "60ms" }}>
            {(["step1", "step2", "step3"] as const).map((step, i) => (
              <li key={step} className="flex items-center gap-3 text-[14px] text-text">
                <span className="flex size-6 shrink-0 items-center justify-center rounded-full bg-text text-[12px] font-semibold text-surface">
                  {i + 1}
                </span>
                {t(`cloud.${step}`)}
              </li>
            ))}
          </ol>
          <p className="rise text-[13px] text-text-3" style={{ animationDelay: "120ms" }}>
            {t("cloud.price")}
          </p>
          {trouble && <Banner tone={state.phase === "expired" ? "warning" : "error"}>{trouble}</Banner>}
        </div>
      )}
    </StepLayout>
  );
}

type Permission = "microphone" | "accessibility";

function PermissionsStep({ nav }: { nav: Nav }) {
  const { t } = useT();
  const [granted, setGranted] = useState({ microphone: false, accessibility: false });

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

  const rows: { id: Permission; icon: typeof Mic; request: () => Promise<unknown> }[] = [
    { id: "microphone", icon: Mic, request: requestMicrophonePermission },
    { id: "accessibility", icon: PersonStanding, request: requestAccessibilityPermission },
  ];
  const all = granted.microphone && granted.accessibility;

  return (
    <StepLayout
      nav={nav}
      scene={<PermissionsScene {...granted} />}
      footer={
        <Button variant={all ? "primary" : "ghost"} size="lg" onClick={nav.next}>
          {all ? t("common.continue") : t("onboarding.permissions.later")}
          <ArrowRight className="size-4" />
        </Button>
      }
    >
      <Title title={t("onboarding.permissions.title")} subtitle={t("onboarding.permissions.subtitle")} />
      <div className="flex flex-col gap-2">
        {rows.map(({ id, icon, request }, i) => (
          <div
            key={id}
            className="rise flex items-center gap-3.5 rounded-card bg-surface px-4 py-3.5 shadow-card"
            style={{ animationDelay: `${60 + i * 60}ms` }}
          >
            <IconTile icon={icon} tint={id === "microphone" ? "red" : "blue"} />
            <div className="min-w-0 flex-1">
              <div className="text-[14.5px] font-semibold text-text">{t(`onboarding.permissions.${id}`)}</div>
              <div className="text-[12.5px] text-text-3">{t(`onboarding.permissions.${id}Hint`)}</div>
            </div>
            {granted[id] ? (
              <span className="inline-flex items-center gap-1 text-[13px] font-medium text-success">
                <Check className="size-4" />
                {t("onboarding.permissions.allowed")}
              </span>
            ) : (
              <Button size="sm" variant="primary" onClick={() => request()}>
                {t("onboarding.permissions.allow")}
              </Button>
            )}
          </div>
        ))}
      </div>
    </StepLayout>
  );
}

function ShortcutStep({ nav }: { nav: Nav }) {
  const { t } = useT();
  const { settings } = useParler();
  const toKeys = useKeycaps();
  return (
    <StepLayout
      nav={nav}
      scene={<ShortcutScene keys={toKeys(settings.shortcut)} />}
      footer={
        <Button variant="primary" size="lg" onClick={nav.next}>
          {t("common.continue")}
          <ArrowRight className="size-4" />
        </Button>
      }
    >
      <Title title={t("onboarding.shortcut.title")} subtitle={t("onboarding.shortcut.subtitle")} />
      <div className="rise" style={{ animationDelay: "80ms" }}>
        <ShortcutField size="lg" />
      </div>
    </StepLayout>
  );
}

function TryStep({ nav }: { nav: Nav }) {
  const { t, locale } = useT();
  const { settings, status, update, cloud } = useParler();
  const toKeys = useKeycaps();
  const [text, setText] = useState("");
  const done = text.trim().length > 0;
  const problem =
    status.problem && status.state === "idle"
      ? describeProblem(status.problem, t("providers.customName"), locale)
      : null;
  const finish = () => update({ onboarded: true });
  const manageUrl = cloud.account?.manageUrl ?? problem?.manageUrl;

  return (
    <StepLayout
      nav={nav}
      scene={<IdeasScene />}
      footer={
        done ? (
          <Button variant="primary" size="lg" onClick={finish}>
            {t("onboarding.try.finish")}
            <ArrowRight className="size-4" />
          </Button>
        ) : (
          <Button variant="ghost" size="lg" onClick={finish}>
            {t("onboarding.try.skip")}
          </Button>
        )
      }
    >
      <div className="rise mb-5">
        <h1 className="font-display text-title text-text">{t("onboarding.try.title")}</h1>
        <p className="mt-3 text-[14.5px] leading-[2] text-text-2">
          <KeysSentence
            text={t(
              settings.shortcutMode === "toggle"
                ? "onboarding.try.pressToSpeak"
                : "onboarding.try.holdToSpeak",
            )}
            keys={<Keycaps keys={toKeys(settings.shortcut)} />}
          />
        </p>
      </div>
      <TextArea
        autoFocus
        value={text}
        onChange={(e) => setText(e.target.value)}
        placeholder={t("onboarding.try.placeholder")}
        className={`rise min-h-36 font-display text-[21px] leading-snug ${
          status.state === "recording" ? "shadow-[inset_0_0_0_1.5px_var(--accent),0_0_0_4px_var(--accent-soft)]" : ""
        }`}
        style={{ animationDelay: "80ms" }}
      />
      <div className="mt-3 min-h-6">
        {done && (
          <p className="rise flex items-center gap-1.5 text-[14px] font-medium text-success">
            <Check className="size-4" />
            {t("onboarding.try.done")}
          </p>
        )}
        {problem && !done && (
          <Banner
            tone="error"
            actions={
              problem.manageUrl && manageUrl ? (
                <PlanButton account={{ status: cloud.account?.status ?? "none", manageUrl }} />
              ) : undefined
            }
          >
            {t(problem.key, problem.params)}
          </Banner>
        )}
      </div>
    </StepLayout>
  );
}
