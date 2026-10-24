import { getCurrentWindow } from "@tauri-apps/api/window";
import { Globe } from "lucide-react";
import { useEffect, useState } from "react";
import { requestAccessibilityPermission } from "tauri-plugin-macos-permissions-api";
import { useT } from "../i18n/context";
import { api } from "../lib/api";
import { acceleratorFromEvent, FN, isReserved, keycaps } from "../lib/shortcut";
import { useKeycaps } from "./useKeycaps";
import { useParler } from "../state";
import { Button, Keycaps } from "../ui";

/** The dictation shortcut, with a way to record a new one (or, on macOS, to
 * pick the fn key, which can't be recorded). */
export function ShortcutField({ size = "md" }: { size?: "md" | "lg" }) {
  const { t } = useT();
  const { settings, status, info, update } = useParler();
  const toKeys = useKeycaps();
  const [recording, setRecording] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const mac = info.platform === "macos";
  const onFn = settings.shortcut === FN;

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

  const chooseFn = async () => {
    setRecording(false);
    setError(null);
    const refused = await update({ shortcut: FN });
    if (refused) setError(refused);
  };

  return (
    <div className="flex flex-col items-start gap-2">
      <div className="flex flex-wrap items-center gap-x-3 gap-y-2">
        {recording ? (
          <span className="flex h-10 items-center rounded-xl bg-accent-soft px-3.5 text-[13px] font-medium text-accent-text">
            {t("settings.shortcut.recording")}
          </span>
        ) : (
          <Keycaps keys={toKeys(settings.shortcut)} size={size} />
        )}
        <span className="flex items-center gap-1">
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
          {/* fn never reaches the window, so it is offered rather than recorded. */}
          {mac && !onFn && (
            <Button size="sm" variant="ghost" onClick={chooseFn}>
              <Globe className="size-3.5" />
              {t("settings.shortcut.useFn")}
            </Button>
          )}
        </span>
      </div>
      {error && <p className="text-[12.5px] text-accent-text">{error}</p>}
      {mac && onFn && (
        <FnHints
          needsAccess={status.problem?.kind === "shortcut" && status.problem.shortcut === FN}
        />
      )}
    </div>
  );
}

/**
 * What stands between the fn key and Parler: the Accessibility permission,
 * and macOS's own use of 🌐 (emoji, input source, Dictation) unless set to
 * do nothing.
 */
function FnHints({ needsAccess }: { needsAccess: boolean }) {
  const { t } = useT();
  const [globeBusy, setGlobeBusy] = useState(false);

  // Checked again when coming back from System Settings.
  useEffect(() => {
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
  }, []);

  if (!needsAccess && !globeBusy) return null;
  return (
    <div className="flex max-w-[52ch] flex-col gap-2.5">
      {needsAccess && (
        <Hint
          text={t("problems.fnKey")}
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
