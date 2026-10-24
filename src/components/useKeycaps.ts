import { useT } from "../i18n/context";
import { keycaps } from "../lib/shortcut";
import { useParler } from "../state";

/** Turns a shortcut into keycaps for this system, named in the interface language. */
export function useKeycaps(): (accelerator: string) => string[] {
  const { t } = useT();
  const { info } = useParler();
  return (accelerator) =>
    keycaps(accelerator, info.platform, { space: t("common.space"), ctrl: t("common.ctrl") });
}
