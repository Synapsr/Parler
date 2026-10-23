import { useEffect, useState } from "react";
import { useT } from "../i18n/context";
import { api } from "../lib/api";
import { acceleratorFromEvent, isReserved, keycaps } from "../lib/shortcut";
import { useParler } from "../state";
import { Button, Keycaps } from "../ui";

/** The dictation shortcut, with a way to record a new one. */
export function ShortcutField({ size = "md" }: { size?: "md" | "lg" }) {
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
      const label = keycaps(accelerator, info.platform, t("common.space")).join(" ");
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
  }, [recording, info.platform, t, update]);

  return (
    <div className="flex flex-col items-start gap-2">
      <div className="flex items-center gap-3">
        {recording ? (
          <span className="flex h-10 items-center rounded-xl bg-accent-soft px-3.5 text-[13px] font-medium text-accent-text">
            {t("settings.shortcut.recording")}
          </span>
        ) : (
          <Keycaps
            keys={keycaps(settings.shortcut, info.platform, t("common.space"))}
            size={size}
          />
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
      {error && <p className="text-[12.5px] text-accent-text">{error}</p>}
    </div>
  );
}
