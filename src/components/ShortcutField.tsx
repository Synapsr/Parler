import { useEffect, useState } from "react";
import { useT } from "../i18n/context";
import { api } from "../lib/api";
import { acceleratorFromEvent, keycaps } from "../lib/shortcut";
import { useParler } from "../state";
import { Button, Keycaps } from "./ui";

/** The dictation shortcut, with a way to record a new one. */
export function ShortcutField() {
  const { t } = useT();
  const { settings, info, update } = useParler();
  const [recording, setRecording] = useState(false);
  const [error, setError] = useState<string | null>(null);

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
          const modifiers = info.platform === "macos" ? "⌘ ⌥ ⌃" : "Ctrl / Alt";
          setError(t("settings.shortcut.invalid", { modifiers }));
        }
        return;
      }
      setRecording(false);
      const refused = await update({ shortcut: accelerator });
      setError(
        refused
          ? t("problems.shortcut", {
              shortcut: keycaps(accelerator, info.platform, t("common.space")).join(
                " ",
              ),
            })
          : null,
      );
    };

    window.addEventListener("keydown", onKeyDown, true);
    return () => {
      window.removeEventListener("keydown", onKeyDown, true);
      api.resumeShortcut();
    };
  }, [recording, info.platform, t, update]);

  return (
    <div className="flex flex-col items-end gap-1.5">
      <div className="flex items-center gap-2">
        {recording ? (
          <span className="animate-pulse text-[13px] font-medium text-accent">
            {t("settings.shortcut.recording")}
          </span>
        ) : (
          <Keycaps keys={keycaps(settings.shortcut, info.platform, t("common.space"))} />
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
      </div>
      {error && <p className="max-w-64 text-right text-[12px] text-accent">{error}</p>}
    </div>
  );
}
