import { ArrowUpRight, Eye, EyeOff } from "lucide-react";
import { openUrl } from "@tauri-apps/plugin-opener";
import { useEffect, useState } from "react";
import { useT } from "../i18n/context";
import { api, modelFor, type Settings, type TranscribeError } from "../lib/api";
import { describeProblem } from "../lib/problems";
import { useParler } from "../state";
import { Button, Success, TextInput } from "./ui";

type Test =
  | { state: "idle" | "testing" | "ok" }
  | { state: "failed"; message: string };

/**
 * Key, model and server address for the selected provider. Fields save when
 * they lose focus; `verifyLabel` turns the test into the step's main action.
 */
export function ProviderSetup({
  verifyLabel,
  onVerified,
}: {
  verifyLabel?: string;
  onVerified?: () => void;
}) {
  const { t } = useT();
  const { settings, provider, update } = useParler();
  const [key, setKey] = useState("");
  const [model, setModel] = useState("");
  const [baseUrl, setBaseUrl] = useState(settings.customBaseUrl);
  const [revealed, setRevealed] = useState(false);
  const [test, setTest] = useState<Test>({ state: "idle" });

  const providerId = provider?.id ?? "";
  // Switching provider shows that provider's own key and model.
  useEffect(() => {
    setKey(settings.apiKeys[providerId] ?? "");
    setModel(provider ? modelFor(settings, provider) : "");
    setTest({ state: "idle" });
  }, [providerId]);

  if (!provider) return null;
  const custom = provider.id === "custom";

  // What the fields hold now, saved or not.
  const draft = (): Settings => ({
    ...settings,
    apiKeys: { ...settings.apiKeys, [provider.id]: key.trim() },
    models: { ...settings.models, [provider.id]: model.trim() },
    customBaseUrl: baseUrl.trim(),
  });

  const save = () => {
    const next = draft();
    setTest({ state: "idle" });
    return update({
      apiKeys: next.apiKeys,
      models: next.models,
      customBaseUrl: next.customBaseUrl,
    });
  };

  const verify = async () => {
    setTest({ state: "testing" });
    await save();
    try {
      await api.testProvider(draft());
      setTest({ state: "ok" });
      onVerified?.();
    } catch (error) {
      const text = describeProblem({
        kind: "transcription",
        error: error as TranscribeError,
      });
      setTest({ state: "failed", message: t(text.key, text.params) });
    }
  };

  return (
    <div className="space-y-4">
      {custom && (
        <label className="block">
          <span className="mb-1.5 block text-[13px] font-medium text-ink-soft">
            {t("settings.transcription.baseUrl")}
          </span>
          <TextInput
            value={baseUrl}
            onChange={(e) => setBaseUrl(e.target.value)}
            onBlur={save}
            placeholder="http://localhost:8000/v1"
            className="font-mono text-[13px]"
          />
          <span className="mt-1 block text-[12px] text-muted">
            {t("settings.transcription.baseUrlHint")}
          </span>
        </label>
      )}

      <label className="block">
        <span className="mb-1.5 flex items-center justify-between text-[13px] font-medium text-ink-soft">
          {t("settings.transcription.apiKey")}
          {provider.apiKeyUrl && (
            <button
              type="button"
              onClick={() => openUrl(provider.apiKeyUrl!)}
              className="inline-flex items-center gap-0.5 text-accent hover:underline"
            >
              {t("common.getKey")}
              <ArrowUpRight className="size-3.5" />
            </button>
          )}
        </span>
        <div className="relative">
          <TextInput
            type={revealed ? "text" : "password"}
            value={key}
            onChange={(e) => setKey(e.target.value)}
            onBlur={save}
            placeholder={
              custom
                ? t("settings.transcription.apiKeyOptional")
                : t("settings.transcription.apiKeyPlaceholder")
            }
            className="pr-10 font-mono text-[13px]"
          />
          <button
            type="button"
            onClick={() => setRevealed((r) => !r)}
            aria-label={revealed ? t("common.hide") : t("common.show")}
            className="absolute top-1/2 right-2 flex size-7 -translate-y-1/2 items-center justify-center rounded-lg text-muted hover:text-ink"
          >
            {revealed ? <EyeOff className="size-4" /> : <Eye className="size-4" />}
          </button>
        </div>
      </label>

      <label className="block">
        <span className="mb-1.5 block text-[13px] font-medium text-ink-soft">
          {t("settings.transcription.model")}
        </span>
        <TextInput
          value={model}
          list={`models-${provider.id}`}
          onChange={(e) => setModel(e.target.value)}
          onBlur={save}
          placeholder={provider.defaultModel}
          className="font-mono text-[13px]"
        />
        <datalist id={`models-${provider.id}`}>
          {provider.models.map((m) => (
            <option key={m} value={m} />
          ))}
        </datalist>
        {provider.models.length > 1 && (
          <div className="mt-2 flex flex-wrap gap-1.5">
            {provider.models.map((m) => (
              <button
                key={m}
                type="button"
                onClick={() => {
                  setModel(m);
                  setTest({ state: "idle" });
                  update({ models: { ...settings.models, [provider.id]: m } });
                }}
                className={`rounded-full border px-2.5 py-0.5 font-mono text-[11.5px] transition-colors ${
                  model === m
                    ? "border-ink/40 bg-ink/5 text-ink"
                    : "border-line text-muted hover:text-ink"
                }`}
              >
                {m}
              </button>
            ))}
          </div>
        )}
      </label>

      <div className="flex min-h-9 items-center gap-3">
        <Button
          variant={verifyLabel ? "primary" : "outline"}
          size={verifyLabel ? "lg" : "sm"}
          onClick={verify}
          busy={test.state === "testing"}
          disabled={provider.requiresApiKey && !key.trim()}
        >
          {test.state === "testing"
            ? t("common.testing")
            : (verifyLabel ?? t("common.test"))}
        </Button>
        {test.state === "ok" && !verifyLabel && (
          <Success>{t("common.works")}</Success>
        )}
      </div>
      {test.state === "failed" && (
        <p className="rise rounded-xl bg-accent-soft px-3.5 py-2.5 text-[13px] text-ink">
          {test.message}
        </p>
      )}
    </div>
  );
}
