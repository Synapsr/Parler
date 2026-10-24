import { getCurrentWindow } from "@tauri-apps/api/window";
import { useEffect, useState } from "react";
import { requestAccessibilityPermission } from "tauri-plugin-macos-permissions-api";
import { useT } from "../i18n/context";
import { api } from "../lib/api";
import {
  acceleratorFromEvent,
  FN,
  isReserved,
  isSingleKey,
  keycaps,
  SINGLE_KEYS,
  type SingleKey,
} from "../lib/shortcut";
import { useKeycaps } from "./useKeycaps";
import { useParler } from "../state";
import { Button, Keycaps, Select } from "../ui";

/** The picker's value for a recorded combination. */
const CUSTOM = "custom";

/** "RightCommand" → "rightCommand", the key's name in the locale files. */
const keyName = (key: SingleKey) => key.charAt(0).toLowerCase() + key.slice(1);

/**
 * The dictation shortcut. On macOS a picker offers keys used alone (fn,
 * right ⌘, ⌥, ⌃), which can't be recorded, or a custom combination;
 * elsewhere the combination is recorded directly.
 */
export function ShortcutField({ size = "md" }: { size?: "md" | "lg" }) {
  const { t } = useT();
  const { settings, status, info, update } = useParler();
  const toKeys = useKeycaps();
  const [recording, setRecording] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const mac = info.platform === "macos";
  const single = isSingleKey(settings.shortcut);

  useEffect(() => {
    if (!recording) return;
    // The current shortcut must not start a dictation while a new one is
    // being typed.
    api.suspendShortcut();

    const onKeyDown = async (event: KeyboardEvent) => {
      event.preventDefault();
      event.stopPropagation();
      if (event.code === "Escape") {
        setRecording(false);
        return;
      }
      const accelerator = acceleratorFromEvent(event);
      if (!accelerator) {
        const modifierOnly = /^(Control|Alt|Shift|Meta|OS)/.test(event.code);
        if (!modifierOnly) {
          const modifiers = mac ? "⌘ ⌥ ⌃" : `${t("common.ctrl")} / Alt`;
          setError(t("settings.shortcut.invalid", { modifiers }));
        }
        return;
      }
      const label = keycaps(accelerator, info.platform, {
        space: t("common.space"),
        ctrl: t("common.ctrl"),
      }).join(" ");
      if (isReserved(accelerator, info.platform)) {
        setError(t("settings.shortcut.reserved", { shortcut: label }));
        return;
      }
      setRecording(false);
      const refused = await update({ shortcut: accelerator });
      setError(refused ? t("problems.shortcut", { shortcut: label }) : null);
    };

    window.addEventListener("keydown", onKeyDown, true);
    return () => {
      window.removeEventListener("keydown", onKeyDown, true);
      api.resumeShortcut();
    };
  }, [recording, mac, info.platform, t, update]);

  // A key is saved at once; "Custom shortcut…" records one, and leaving the
  // recorder puts the picker back on what is saved.
  const pick = async (value: string) => {
    setError(null);
    if (value === CUSTOM) {
      setRecording(true);
      return;
    }
    setRecording(false);
    const refused = await update({ shortcut: value });
    if (refused) setError(refused);
  };

  const recorder = (
    <>
      {recording ? (
        <span className="flex h-10 items-center rounded-xl bg-accent-soft px-3.5 text-[13px] font-medium text-accent-text">
          {t("settings.shortcut.recording")}
        </span>
      ) : (
        <Keycaps keys={toKeys(settings.shortcut)} size={size} />
      )}
      <Button
        size="sm"
        variant="ghost"
        onClick={() => {
          setError(null);
          setRecording((r) => !r);
        }}
      >
        {recording ? t("settings.shortcut.cancelHint") : t("common.change")}
      </Button>
    </>
  );

  return (
    <div className="flex flex-col items-start gap-2.5">
      <div className="flex flex-wrap items-center gap-x-3 gap-y-2">
        {mac && (
          <Select
            aria-label={t("settings.shortcut.label")}
            value={recording || !single ? CUSTOM : settings.shortcut}
            onChange={(e) => pick(e.target.value)}
            className="w-56"
          >
            {SINGLE_KEYS.map((key) => (
              <option key={key} value={key}>
                {t(`settings.shortcut.keys.${keyName(key)}`)}
              </option>
            ))}
            <option value={CUSTOM}>{t("settings.shortcut.keys.custom")}</option>
          </Select>
        )}
        {mac && single && !recording ? <Keycaps keys={toKeys(settings.shortcut)} size={size} /> : recorder}
      </div>
      {error && <p className="text-[12.5px] text-accent-text">{error}</p>}
      {mac && single && !recording && (
        <KeyHints
          fn={settings.shortcut === FN}
          needsAccess={status.problem?.kind === "shortcut" && isSingleKey(status.problem.shortcut)}
        />
      )}
    </div>
  );
}

/**
 * About a key used alone: chords don't start a dictation, the Accessibility
 * permission it needs, and, for fn, macOS's own use of 🌐 (emoji, input
 * source, Dictation) unless set to do nothing.
 */
function KeyHints({ fn, needsAccess }: { fn: boolean; needsAccess: boolean }) {
  const { t } = useT();
  const [globeBusy, setGlobeBusy] = useState(false);

  // Checked again when coming back from System Settings.
  useEffect(() => {
    if (!fn) {
      setGlobeBusy(false);
      return;
    }
    const check = () =>
      api
        .globeKey()
        .then(({ action }) => setGlobeBusy(action !== 0))
        .catch(() => setGlobeBusy(false));
    check();
    const unlisten = getCurrentWindow().onFocusChanged(({ payload: focused }) => {
      if (focused) check();
    });
    return () => {
      unlisten.then((stop) => stop()).catch(() => {});
    };
  }, [fn]);

  return (
    <div className="flex max-w-[52ch] flex-col gap-2.5">
      <p className="text-[12.5px] leading-snug text-text-3">{t("settings.shortcut.singleKeyHint")}</p>
      {needsAccess && (
        <Hint
          text={t("problems.singleKey")}
          action={t("problems.openAccessibility")}
          onAction={() => requestAccessibilityPermission()}
        />
      )}
      {globeBusy && (
        <Hint
          text={t("settings.shortcut.globeHint")}
          action={t("settings.shortcut.openKeyboard")}
          onAction={() => api.openKeyboardSettings()}
        />
      )}
    </div>
  );
}

function Hint({ text, action, onAction }: { text: string; action: string; onAction: () => void }) {
  return (
    <div className="flex flex-col items-start gap-1.5">
      <p className="text-[12.5px] leading-snug text-text-3">{text}</p>
      <Button size="sm" onClick={onAction}>
        {action}
      </Button>
    </div>
  );
}
