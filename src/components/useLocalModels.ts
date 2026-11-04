import { useCallback, useEffect, useState } from "react";
import { useT } from "../i18n/context";
import { api, events, LOCAL, type LocalModel, type LocalModelError, type LocalModelId } from "../lib/api";
import { useParler } from "../state";

/**
 * The models that transcribe on this computer, kept current by the backend,
 * and the one in use: the person's choice, or the one recommended for their
 * language.
 */
export function useLocalModels() {
  const { settings, update } = useParler();
  const [models, setModels] = useState<LocalModel[]>([]);

  useEffect(() => {
    const unlisten = events.localModels(setModels);
    return () => {
      unlisten.then((stop) => stop()).catch(() => {});
    };
  }, []);
  // Which one is recommended follows the language settings.
  useEffect(() => {
    api.localModels().then(setModels).catch(() => {});
  }, [settings.language, settings.uiLanguage]);

  const picked = settings.models[LOCAL]?.trim();
  const chosen = models.find((m) => m.id === picked) ?? models.find((m) => m.recommended) ?? models[0];

  /** Changes a model's state at once, before the backend's event. */
  const patch = (id: LocalModelId, state: LocalModel["state"]) =>
    setModels((current) => current.map((m) => (m.id === id ? { ...m, state } : m)));

  /** A refused request (unknown model, file in use): back to what the backend says. */
  const resync = () => {
    api.localModels().then(setModels).catch(() => {});
  };

  const pin = (id: LocalModelId) => {
    if (settings.models[LOCAL] !== id) update({ models: { ...settings.models, [LOCAL]: id } });
  };

  const download = (id: LocalModelId) => {
    const model = models.find((m) => m.id === id);
    // The model being fetched is the one to use, whatever the language later.
    pin(id);
    if (model && (model.state.state === "missing" || model.state.state === "failed")) {
      patch(id, { state: "downloading", downloaded: 0, total: model.sizeBytes });
    }
    api.downloadLocalModel(id).catch(resync);
  };

  const cancel = (id: LocalModelId) => {
    patch(id, { state: "missing" });
    api.cancelLocalModelDownload(id).catch(resync);
  };

  const remove = (id: LocalModelId) => {
    patch(id, { state: "missing" });
    api.deleteLocalModel(id).catch(resync);
  };

  /** Uses another model; a download of the one left behind stops (and can resume later). */
  const choose = (id: LocalModelId) => {
    if (chosen && chosen.id !== id && chosen.state.state === "downloading") cancel(chosen.id);
    pin(id);
  };

  return { models, chosen, download, cancel, remove, choose };
}

export type LocalModels = ReturnType<typeof useLocalModels>;

/** "740 MB", "1.2 GB": sizes in the interface language. */
export function useSize(): (bytes: number) => string {
  const { locale } = useT();
  return useCallback(
    (bytes: number) => {
      const giga = bytes >= 1e9;
      return new Intl.NumberFormat(locale, {
        style: "unit",
        unit: giga ? "gigabyte" : "megabyte",
        maximumFractionDigits: giga ? 1 : 0,
      }).format(giga ? bytes / 1e9 : bytes / 1e6);
    },
    [locale],
  );
}

/** The languages a model knows, in plain words. */
export function useModelLanguages(): (model: LocalModel) => string {
  const { t } = useT();
  return (model) => {
    if (!model.languages) return t("local.languagesAll");
    // Parakeet's are all European; another list would just be counted.
    return model.id === "parakeet-ultra"
      ? t("local.languagesEuropean", { n: model.languages.length })
      : t("local.languagesCount", { n: model.languages.length });
  };
}

/** Why a download stopped, in a sentence. */
export function useDownloadError(): (error: LocalModelError) => string {
  const { t } = useT();
  return (error) =>
    error.kind === "other"
      ? t("local.error.other", { detail: error.detail ?? "" })
      : t(`local.error.${error.kind}`);
}

/** How far a download has come, from 0 to 1. */
export function downloadShare(state: LocalModel["state"]): number {
  if (state.state === "verifying" || state.state === "ready") return 1;
  if (state.state !== "downloading" || state.total <= 0) return 0;
  return Math.min(1, state.downloaded / state.total);
}
