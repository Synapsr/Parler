import { useEffect, useState } from "react";
import { useT } from "../i18n/context";
import { api, modelFor, type Settings, type TranscribeError } from "../lib/api";
import { describeProblem } from "../lib/problems";
import { useParler } from "../state";

export type Check =
  | { state: "idle" | "checking" | "ok" }
  | { state: "failed"; message: string };

/**
 * The key, model and server address of the selected service, saved when a
 * field is left, and checked against the service on demand.
 */
export function useProviderForm() {
  const { t } = useT();
  const { settings, provider, update } = useParler();
  const [key, setKey] = useState("");
  const [model, setModel] = useState("");
  const [baseUrl, setBaseUrl] = useState(settings.customBaseUrl);
  const [check, setCheck] = useState<Check>({ state: "idle" });

  const providerId = provider?.id ?? "";
  // Switching service shows that service's own key and model.
  useEffect(() => {
    setKey(settings.apiKeys[providerId] ?? "");
    setModel(provider ? modelFor(settings, provider) : "");
    setCheck({ state: "idle" });
  }, [providerId]);

  const draft = (overrides: { key?: string; model?: string } = {}): Settings => ({
    ...settings,
    apiKeys: { ...settings.apiKeys, [providerId]: (overrides.key ?? key).trim() },
    models: { ...settings.models, [providerId]: (overrides.model ?? model).trim() },
    customBaseUrl: baseUrl.trim(),
  });

  const save = (overrides: { key?: string; model?: string } = {}) => {
    const next = draft(overrides);
    setCheck({ state: "idle" });
    return update({
      apiKeys: next.apiKeys,
      models: next.models,
      customBaseUrl: next.customBaseUrl,
    });
  };

  /** Saves, then sends the service half a second of silence. */
  const verify = async (overrides: { key?: string } = {}) => {
    setCheck({ state: "checking" });
    await save(overrides);
    try {
      await api.testProvider(draft(overrides));
      setCheck({ state: "ok" });
      return true;
    } catch (error) {
      const text = describeProblem({
        kind: "transcription",
        error: error as TranscribeError,
      });
      setCheck({ state: "failed", message: t(text.key, text.params) });
      return false;
    }
  };

  return {
    provider,
    key,
    setKey: (value: string) => {
      setKey(value);
      setCheck({ state: "idle" });
    },
    model,
    setModel,
    baseUrl,
    setBaseUrl,
    check,
    save,
    verify,
    needsKey: (provider?.requiresApiKey ?? false) && !key.trim(),
  };
}
