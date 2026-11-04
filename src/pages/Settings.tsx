import {
  ArrowUpRight,
  AudioLines,
  Check,
  Download,
  Eye,
  EyeOff,
  Info,
  Keyboard,
  Mic,
  SlidersHorizontal,
  Type,
} from "lucide-react";
import { disable, enable, isEnabled } from "@tauri-apps/plugin-autostart";
import { openUrl } from "@tauri-apps/plugin-opener";
import { useEffect, useState, type ReactNode } from "react";
import { CloudAccountPanel } from "../components/Cloud";
import { LocalModelCard, LocalModelSelect } from "../components/LocalModel";
import { useLocalModels, useSize, type LocalModels } from "../components/useLocalModels";
import { SendLog } from "../components/SendLog";
import { ShortcutField } from "../components/ShortcutField";
import { usePercent, useUpdate, useUpdateError } from "../components/useUpdate";
import { useProviderForm } from "../components/useProviderForm";
import { useT } from "../i18n/context";
import {
  api,
  CLOUD,
  LOCAL,
  type Microphone,
  type Output,
  type ShortcutMode,
  type Theme,
  type TranscribeError,
  type UiLanguage,
} from "../lib/api";
import { LOCALES } from "../i18n";
import { LANGUAGES } from "../lib/languages";
import { providerName } from "../lib/look";
import { describeProblem } from "../lib/problems";
import type { Check as TestCheck } from "../components/useProviderForm";
import { useParler } from "../state";
import {
  Banner,
  Button,
  Card,
  Disclosure,
  Field,
  Input,
  PageHeader,
  SectionHeader,
  Segmented,
  Select,
  Toggle,
  type Tint,
} from "../ui";
import type { LucideIcon } from "lucide-react";

const REPOSITORY = "https://github.com/Synapsr/Parler";

function Section({
  icon,
  tint,
  title,
  description,
  children,
  delay,
}: {
  icon: LucideIcon;
  tint: Tint;
  title: string;
  description: string;
  children: ReactNode;
  delay: number;
}) {
  return (
    <div className="rise" style={{ animationDelay: `${delay}ms` }}>
      <Card>
        <SectionHeader icon={icon} tint={tint} title={title} description={description} />
        <div className="flex flex-col divide-y divide-border">{children}</div>
      </Card>
    </div>
  );
}

export function SettingsPage() {
  const { t } = useT();
  return (
    <div className="flex flex-col gap-4">
      <PageHeader title={t("settings.title")} subtitle={t("settings.subtitle")} />
      <Transcription />
      <Shortcut />
      <MicrophoneSection />
      <TextSection />
      <General />
      <About />
    </div>
  );
}

function Transcription() {
  const { t } = useT();
  const { settings, providers, update } = useParler();
  const form = useProviderForm();
  const local = useLocalModels();
  const [revealed, setRevealed] = useState(false);
  const provider = form.provider;
  if (!provider) return null;
  const custom = provider.id === "custom";
  // Parler Cloud has an account instead of a key, and picks its own model.
  const cloud = provider.id === CLOUD;
  // On this computer: a model to download, no key at all.
  const onLocal = provider.id === LOCAL;
  // Parler's own two ways first, then the services that take a key.
  const ordered = [
    ...providers.filter((p) => p.id === CLOUD || p.id === LOCAL),
    ...providers.filter((p) => p.id !== CLOUD && p.id !== LOCAL),
  ];

  return (
    <Section
      icon={AudioLines}
      tint="violet"
      title={t("settings.transcription.title")}
      description={t("settings.transcription.what")}
      delay={0}
    >
      <Field label={t("settings.transcription.provider")}>
        <Select value={settings.provider} onChange={(e) => update({ provider: e.target.value })}>
          {ordered.map((p) => (
            <option key={p.id} value={p.id}>
              {providerName(p, t)}
            </option>
          ))}
        </Select>
      </Field>

      {custom && (
        <Field label={t("settings.transcription.baseUrl")} hint={t("settings.transcription.baseUrlHint")}>
          <Input
            value={form.baseUrl}
            onChange={(e) => form.setBaseUrl(e.target.value)}
            onBlur={() => form.save()}
            placeholder="http://localhost:8000/v1"
            className="font-mono text-[13px]"
          />
        </Field>
      )}

      {onLocal ? (
        <LocalModelField local={local} />
      ) : cloud ? (
        <Field label={t("cloud.account")}>
          <CloudAccountPanel />
        </Field>
      ) : (
        <Field
          label={t("settings.transcription.apiKey")}
          hint={
            provider.apiKeyUrl && (
              <button
                type="button"
                onClick={() => openUrl(provider.apiKeyUrl!)}
                className="inline-flex items-center gap-0.5 font-medium text-accent-text hover:underline"
              >
                {t("settings.transcription.getKey")}
                <ArrowUpRight className="size-3.5" />
              </button>
            )
          }
        >
          <div className="flex gap-2">
            <Input
              type={revealed ? "text" : "password"}
              value={form.key}
              onChange={(e) => form.setKey(e.target.value)}
              onBlur={() => form.save()}
              placeholder={
                custom
                  ? t("settings.transcription.apiKeyOptional")
                  : t("settings.transcription.apiKeyPlaceholder")
              }
              className="font-mono text-[13px]"
              trailing={
                <button
                  type="button"
                  onClick={() => setRevealed((r) => !r)}
                  aria-label={revealed ? t("common.hide") : t("common.show")}
                  className="flex size-8 items-center justify-center rounded-lg text-text-3 hover:text-text"
                >
                  {revealed ? <EyeOff className="size-4" /> : <Eye className="size-4" />}
                </button>
              }
            />
            <Button
              className="h-11"
              onClick={() => form.verify()}
              busy={form.check.state === "checking"}
              disabled={form.needsKey}
            >
              {form.check.state === "checking" ? t("common.testing") : t("common.test")}
            </Button>
          </div>
          {form.check.state === "ok" && <Banner tone="success">{t("common.works")}</Banner>}
          {form.check.state === "failed" && <Banner tone="error">{form.check.message}</Banner>}
        </Field>
      )}

      <Field label={t("settings.transcription.language")} hint={t("settings.transcription.languageHint")}>
        <Select value={settings.language} onChange={(e) => update({ language: e.target.value })}>
          <option value="auto">{t("settings.transcription.languageAuto")}</option>
          {LANGUAGES.map((l) => (
            <option key={l.code} value={l.code}>
              {l.name}
            </option>
          ))}
        </Select>
        {onLocal && <LanguageMismatch local={local} />}
      </Field>

      {onLocal && local.chosen && (
        <div className="py-3.5 last:pb-0">
          <Disclosure label={t("settings.transcription.advanced")}>
            <LocalAdvanced local={local} />
          </Disclosure>
        </div>
      )}

      {!cloud && !onLocal && (
        <div className="py-3.5 last:pb-0">
          <Disclosure label={t("settings.transcription.advanced")}>
            <Field label={t("settings.transcription.model")} hint={t("settings.transcription.modelHint")}>
              <Input
                value={form.model}
                list={`models-${provider.id}`}
                onChange={(e) => form.setModel(e.target.value)}
                onBlur={() => form.save()}
                placeholder={provider.defaultModel}
                className="font-mono text-[13px]"
              />
              <datalist id={`models-${provider.id}`}>
                {provider.models.map((m) => (
                  <option key={m} value={m} />
                ))}
              </datalist>
            </Field>
          </Disclosure>
        </div>
      )}
    </Section>
  );
}

/** The model on this computer: its state, the honest trade-off, and a test once it's here. */
function LocalModelField({ local }: { local: LocalModels }) {
  const { t, locale } = useT();
  const { settings } = useParler();
  const [check, setCheck] = useState<TestCheck>({ state: "idle" });
  const [confirming, setConfirming] = useState(false);
  const model = local.chosen;
  const ready = model?.state.state === "ready";

  // A new model, or one gone, needs testing again.
  useEffect(() => {
    setCheck({ state: "idle" });
    setConfirming(false);
  }, [model?.id, ready]);

  if (!model) return null;

  /** Loads the model and runs half a second of silence through it. */
  const test = async () => {
    setCheck({ state: "checking" });
    try {
      await api.testProvider(settings);
      setCheck({ state: "ok" });
    } catch (error) {
      const text = describeProblem(
        { kind: "transcription", error: error as TranscribeError },
        t("providers.customName"),
        locale,
      );
      setCheck({ state: "failed", message: t(text.key, text.params) });
    }
  };

  const remove = () => {
    if (!confirming) {
      setConfirming(true);
      return;
    }
    setConfirming(false);
    local.remove(model.id);
  };

  return (
    <Field label={t("settings.transcription.model")} hint={t("local.tradeoff")}>
      <LocalModelCard local={local} model={model} inset showBackend />
      {ready && (
        <div className="flex items-center gap-2">
          <Button size="sm" onClick={test} busy={check.state === "checking"}>
            {check.state === "checking" ? t("common.testing") : t("common.test")}
          </Button>
          <div className="flex-1" />
          <Button
            size="sm"
            variant="ghost"
            onClick={remove}
            onBlur={() => setConfirming(false)}
            className={confirming ? "text-accent-text hover:text-accent-text" : ""}
          >
            {confirming ? t("local.confirmDelete") : t("common.delete")}
          </Button>
        </div>
      )}
      {check.state === "ok" && <Banner tone="success">{t("common.works")}</Banner>}
      {check.state === "failed" && <Banner tone="error">{check.message}</Banner>}
    </Field>
  );
}

/** The other model, and any model kept on disk that isn't in use. */
function LocalAdvanced({ local }: { local: LocalModels }) {
  const { t } = useT();
  const size = useSize();
  const idle = local.models.filter((m) => m.id !== local.chosen?.id && m.state.state === "ready");
  return (
    <Field label={t("settings.transcription.model")} hint={t("local.pickHint")}>
      <LocalModelSelect local={local} />
      {idle.map((m) => (
        <p key={m.id} className="flex flex-wrap items-center gap-x-2 text-[12.5px] text-text-3">
          {t("local.alsoHere", { model: m.name, size: size(m.sizeBytes) })}
          <button
            type="button"
            onClick={() => local.remove(m.id)}
            className="font-medium text-text-2 hover:text-text"
          >
            {t("common.delete")}
          </button>
        </p>
      ))}
    </Field>
  );
}

/** The chosen model doesn't know the spoken language: say which one does. */
function LanguageMismatch({ local }: { local: LocalModels }) {
  const { t } = useT();
  const { settings } = useParler();
  const model = local.chosen;
  if (!model?.languages || settings.language === "auto" || model.languages.includes(settings.language)) {
    return null;
  }
  const other = local.models.find((m) => !m.languages || m.languages.includes(settings.language));
  if (!other) return null;
  return (
    <p className="rise rounded-xl bg-warning-soft px-3.5 py-2 text-[12.5px] leading-snug text-text">
      {t("local.languageMismatch", { model: model.name, other: other.name })}{" "}
      <button
        type="button"
        onClick={() => local.choose(other.id)}
        className="font-medium whitespace-nowrap text-accent-text hover:underline"
      >
        {t("local.use", { model: other.name })}
      </button>
    </p>
  );
}

function Shortcut() {
  const { t } = useT();
  const { settings, info, update } = useParler();
  const modes: ShortcutMode[] = ["smart", "hold", "toggle"];
  return (
    <Section
      icon={Keyboard}
      tint="amber"
      title={t("settings.shortcut.title")}
      description={t("settings.shortcut.description")}
      delay={40}
    >
      <Field label={t("settings.shortcut.label")}>
        <ShortcutField />
      </Field>
      <Field label={t("settings.shortcut.mode")} hint={t(`settings.shortcut.${settings.shortcutMode}Hint`)}>
        <Segmented
          options={modes.map((m) => ({ value: m, label: t(`settings.shortcut.${m}`) }))}
          value={settings.shortcutMode}
          onChange={(shortcutMode) => update({ shortcutMode })}
        />
      </Field>
      {info.platform === "linux" && (
        <p className="py-3.5 text-[12.5px] text-text-3 last:pb-0">
          {t("settings.shortcut.wayland", { command: "parler --toggle" })}
        </p>
      )}
    </Section>
  );
}

function MicrophoneSection() {
  const { t } = useT();
  const { settings, update } = useParler();
  const [microphones, setMicrophones] = useState<Microphone[]>([]);
  useEffect(() => {
    api.listMicrophones().then(setMicrophones);
  }, []);

  return (
    <Section
      icon={Mic}
      tint="red"
      title={t("settings.microphone.title")}
      description={t("settings.microphone.description")}
      delay={80}
    >
      <div>
        <Select
          value={settings.microphone ?? ""}
          onChange={(e) => update({ microphone: e.target.value || null })}
        >
          <option value="">{t("settings.microphone.default")}</option>
          {microphones.map((m) => (
            <option key={m.id} value={m.id}>
              {m.name}
            </option>
          ))}
        </Select>
      </div>
    </Section>
  );
}

function TextSection() {
  const { t } = useT();
  const { settings, update } = useParler();
  const outputs: Output[] = ["paste", "copy"];
  return (
    <Section
      icon={Type}
      tint="blue"
      title={t("settings.output.title")}
      description={t("settings.output.description")}
      delay={120}
    >
      <div className="flex flex-col gap-2">
        <Segmented
          options={outputs.map((o) => ({ value: o, label: t(`settings.output.${o}`) }))}
          value={settings.output}
          onChange={(output) => update({ output })}
        />
        <p className="text-[12.5px] text-text-3">{t(`settings.output.${settings.output}Hint`)}</p>
      </div>
    </Section>
  );
}

function General() {
  const { t } = useT();
  const { settings, update } = useParler();
  const [autostart, setAutostart] = useState(false);

  useEffect(() => {
    isEnabled().then(setAutostart).catch(() => {});
  }, []);

  const toggleAutostart = async (on: boolean) => {
    setAutostart(on);
    try {
      await (on ? enable() : disable());
    } catch {
      setAutostart(!on);
    }
  };

  return (
    <Section
      icon={SlidersHorizontal}
      tint="stone"
      title={t("settings.general.title")}
      description={t("settings.general.description")}
      delay={160}
    >
      <Field inline label={t("settings.general.sounds")} hint={t("settings.general.soundsHint")}>
        <Toggle
          checked={settings.sounds}
          onChange={(sounds) => update({ sounds })}
          label={t("settings.general.sounds")}
        />
      </Field>
      <Field inline label={t("settings.general.muteAudio")} hint={t("settings.general.muteAudioHint")}>
        <Toggle
          checked={settings.muteAudio}
          onChange={(muteAudio) => update({ muteAudio })}
          label={t("settings.general.muteAudio")}
        />
      </Field>
      <Field inline label={t("settings.general.autostart")} hint={t("settings.general.autostartHint")}>
        <Toggle checked={autostart} onChange={toggleAutostart} label={t("settings.general.autostart")} />
      </Field>
      <Field inline label={t("settings.general.theme")}>
        <Segmented<Theme>
          options={(["system", "light", "dark"] as const).map((theme) => ({
            value: theme,
            label: t(`settings.general.theme_${theme}`),
          }))}
          value={settings.theme}
          onChange={(theme) => update({ theme })}
        />
      </Field>
      <Field inline label={t("settings.general.interfaceLanguage")}>
        <Select
          value={settings.uiLanguage}
          onChange={(e) => update({ uiLanguage: e.target.value as UiLanguage })}
          className="w-40"
        >
          <option value="auto">{t("settings.general.system")}</option>
          {LOCALES.map((l) => (
            <option key={l.code} value={l.code}>
              {l.name}
            </option>
          ))}
        </Select>
      </Field>
    </Section>
  );
}

function About() {
  const { t } = useT();
  const { info } = useParler();
  return (
    <Section icon={Info} tint="stone" title={t("settings.about.title")} description={t("settings.about.version", { version: info.version })} delay={200}>
      <div className="flex flex-col items-start gap-3">
        <Updates />
        <p className="text-[13px] leading-relaxed text-text-2">{t("settings.about.thanks")}</p>
        <Button size="sm" onClick={() => openUrl(REPOSITORY)}>
          {t("settings.about.source")}
          <ArrowUpRight className="size-3.5" />
        </Button>
        <SendLog />
      </div>
    </Section>
  );
}

/** "Check for updates", and what came of it. */
function Updates() {
  const { t } = useT();
  const { status, check, install } = useUpdate();
  const explain = useUpdateError();
  const percent = usePercent();

  if (status.state === "disabled") {
    return <p className="text-[13px] text-text-3">{t("updates.disabled")}</p>;
  }
  const busy = ["checking", "downloading", "restarting"].includes(status.state);
  let outcome: ReactNode = null;
  switch (status.state) {
    case "upToDate":
      outcome = (
        <span className="inline-flex items-center gap-1.5 text-success">
          <Check className="size-4" />
          {t("updates.upToDate")}
        </span>
      );
      break;
    case "available":
      outcome = (
        <>
          <span className="text-text">{t("updates.available", { version: status.version })}</span>
          <Button size="sm" variant="primary" onClick={install}>
            <Download className="size-3.5" />
            {t("updates.update")}
          </Button>
        </>
      );
      break;
    case "downloading":
      outcome = (
        <span className="text-text-2 tabular-nums">
          {t("updates.downloading", { version: status.version })}
          {status.progress !== null && ` ${percent(status.progress)}`}
        </span>
      );
      break;
    case "restarting":
      outcome = (
        <span className="text-text-2">
          {status.waiting ? t("updates.waiting", { version: status.version }) : t("updates.restarting")}
        </span>
      );
      break;
    case "failed":
      outcome = <span className="text-accent-text">{explain(status.error)}</span>;
      break;
  }

  return (
    <div className="flex flex-wrap items-center gap-x-3 gap-y-2 text-[13px]">
      <Button size="sm" onClick={check} busy={status.state === "checking"} disabled={busy}>
        {status.state === "checking" ? t("updates.checking") : t("updates.check")}
      </Button>
      {outcome}
    </div>
  );
}
