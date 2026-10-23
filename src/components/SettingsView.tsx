import { ArrowUpRight } from "lucide-react";
import { disable, enable, isEnabled } from "@tauri-apps/plugin-autostart";
import { openUrl } from "@tauri-apps/plugin-opener";
import { useEffect, useState } from "react";
import { useT } from "../i18n/context";
import { api, type Microphone, type Output, type ShortcutMode, type UiLanguage } from "../lib/api";
import { LANGUAGES } from "../lib/languages";
import { useParler } from "../state";
import { ProviderSetup } from "./ProviderSetup";
import { ShortcutField } from "./ShortcutField";
import { Row, Section, Segmented, Select, Toggle } from "./ui";

const REPOSITORY = "https://github.com/Synapsr/Parler";

export function SettingsView() {
  const { t } = useT();
  return (
    <div className="flex flex-col gap-7 px-6 pt-1 pb-10">
      <Transcription />
      <Shortcut />
      <Section title={t("settings.microphone.title")}>
        <MicrophonePicker />
      </Section>
      <Text />
      <General />
      <About />
    </div>
  );
}

function Transcription() {
  const { t } = useT();
  const { settings, providers, update } = useParler();
  const [vocabulary, setVocabulary] = useState(settings.vocabulary);

  return (
    <Section title={t("settings.transcription.title")}>
      <Row label={t("settings.transcription.provider")}>
        <Select
          value={settings.provider}
          onChange={(e) => update({ provider: e.target.value })}
          className="w-44"
        >
          {providers.map((p) => (
            <option key={p.id} value={p.id}>
              {p.label}
            </option>
          ))}
        </Select>
      </Row>
      <div className="px-4 py-4">
        <ProviderSetup />
      </div>
      <Row
        label={t("settings.transcription.language")}
        hint={t("settings.transcription.languageHint")}
        stacked
      >
        <Select value={settings.language} onChange={(e) => update({ language: e.target.value })}>
          <option value="auto">{t("settings.transcription.languageAuto")}</option>
          {LANGUAGES.map((l) => (
            <option key={l.code} value={l.code}>
              {l.name}
            </option>
          ))}
        </Select>
      </Row>
      <Row
        label={t("settings.transcription.vocabulary")}
        hint={t("settings.transcription.vocabularyHint")}
        stacked
      >
        <textarea
          value={vocabulary}
          onChange={(e) => setVocabulary(e.target.value)}
          onBlur={() => update({ vocabulary })}
          placeholder={t("settings.transcription.vocabularyPlaceholder")}
          rows={2}
          className="w-full resize-none rounded-xl border border-line-strong bg-raised px-3.5 py-2.5 text-sm text-ink placeholder:text-muted/70 focus:border-ink/40 focus:outline-none"
        />
      </Row>
    </Section>
  );
}

function Shortcut() {
  const { t } = useT();
  const { settings, info, update } = useParler();
  const modes: ShortcutMode[] = ["smart", "hold", "toggle"];

  return (
    <Section title={t("settings.shortcut.title")}>
      <Row label={t("settings.shortcut.label")}>
        <ShortcutField />
      </Row>
      <Row
        label={t("settings.shortcut.mode")}
        hint={t(`settings.shortcut.${settings.shortcutMode}Hint`)}
        stacked
      >
        <Segmented
          options={modes.map((m) => ({ value: m, label: t(`settings.shortcut.${m}`) }))}
          value={settings.shortcutMode}
          onChange={(shortcutMode) => update({ shortcutMode })}
        />
      </Row>
      {info.platform === "linux" && (
        <p className="px-4 py-3 text-[12.5px] text-muted">
          {t("settings.shortcut.wayland", { command: "parler --toggle" })}
        </p>
      )}
    </Section>
  );
}

function MicrophonePicker() {
  const { t } = useT();
  const { settings, update } = useParler();
  const [microphones, setMicrophones] = useState<Microphone[]>([]);

  useEffect(() => {
    api.listMicrophones().then(setMicrophones);
  }, []);

  return (
    <div className="px-4 py-3.5">
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
  );
}

function Text() {
  const { t } = useT();
  const { settings, update } = useParler();
  const outputs: Output[] = ["paste", "copy"];
  return (
    <Section title={t("settings.output.title")}>
      <Row label={t(`settings.output.${settings.output}Hint`)} stacked>
        <Segmented
          options={outputs.map((o) => ({ value: o, label: t(`settings.output.${o}`) }))}
          value={settings.output}
          onChange={(output) => update({ output })}
        />
      </Row>
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
    <Section title={t("settings.general.title")}>
      <Row label={t("settings.general.sounds")} hint={t("settings.general.soundsHint")}>
        <Toggle
          checked={settings.sounds}
          onChange={(sounds) => update({ sounds })}
          label={t("settings.general.sounds")}
        />
      </Row>
      <Row label={t("settings.general.autostart")} hint={t("settings.general.autostartHint")}>
        <Toggle
          checked={autostart}
          onChange={toggleAutostart}
          label={t("settings.general.autostart")}
        />
      </Row>
      <Row label={t("settings.general.interfaceLanguage")}>
        <Select
          value={settings.uiLanguage}
          onChange={(e) => update({ uiLanguage: e.target.value as UiLanguage })}
          className="w-36"
        >
          <option value="auto">{t("settings.general.system")}</option>
          <option value="en">English</option>
          <option value="fr">Français</option>
        </Select>
      </Row>
    </Section>
  );
}

function About() {
  const { t } = useT();
  const { info } = useParler();
  return (
    <Section title={t("settings.about.title")}>
      <div className="flex flex-col gap-2 px-4 py-3.5">
        <div className="flex items-center justify-between">
          <span className="font-mono text-[12px] text-muted">
            {t("settings.about.version", { version: info.version })}
          </span>
          <button
            type="button"
            onClick={() => openUrl(REPOSITORY)}
            className="inline-flex items-center gap-0.5 text-[13px] font-medium text-accent hover:underline"
          >
            {t("settings.about.source")}
            <ArrowUpRight className="size-3.5" />
          </button>
        </div>
        <p className="text-[12.5px] text-muted">{t("settings.about.credits")}</p>
      </div>
    </Section>
  );
}
