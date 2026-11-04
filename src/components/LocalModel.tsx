import { Check, Cpu, Download, LoaderCircle, RotateCw } from "lucide-react";
import { useT } from "../i18n/context";
import { useEffect, useState } from "react";
import { api, LOCAL, type LocalBackend, type LocalModel, type LocalModelId } from "../lib/api";
import { useParler } from "../state";
import { Banner, Button, IconTile, Select } from "../ui";
import { usePercent } from "./useUpdate";
import {
  downloadShare,
  useDownloadError,
  useModelLanguages,
  useSize,
  type LocalModels,
} from "./useLocalModels";

/** A thin bar that fills as the model arrives. */
export function ProgressBar({ share, className = "" }: { share: number; className?: string }) {
  return (
    <span
      role="progressbar"
      aria-valuemin={0}
      aria-valuemax={100}
      aria-valuenow={Math.round(share * 100)}
      className={`block h-1.5 overflow-hidden rounded-full bg-text/10 ${className}`}
    >
      <span
        className="block h-full rounded-full bg-text transition-[width] duration-200"
        style={{ width: `${Math.round(share * 100)}%` }}
      />
    </span>
  );
}

/** "312 MB of 740 MB". */
export function useProgressText(): (model: LocalModel) => string {
  const { t } = useT();
  const size = useSize();
  return (model) => {
    const state = model.state;
    const total = state.state === "downloading" && state.total > 0 ? state.total : model.sizeBytes;
    const done = state.state === "downloading" ? state.downloaded : 0;
    return t("local.progress", { done: size(done), total: size(total) });
  };
}

type BackendKind = LocalBackend["kind"];

/** "Metal · Apple M4" → metal, "Vulkan · AMD Radeon 780M" → vulkan, "CPU" → cpu. */
export function backendKind(backend: string): BackendKind {
  if (/^\s*metal/i.test(backend)) return "metal";
  if (/^\s*vulkan/i.test(backend)) return "vulkan";
  return "cpu";
}

const RUNS_ON: Record<BackendKind, string> = {
  metal: "local.runsOnGpu",
  vulkan: "local.runsOnCard",
  cpu: "local.runsOnCpu",
};

/**
 * Where the model runs: what the loaded one actually uses (a graphics card
 * that failed falls back to the processor), else where it will run.
 */
function useRunsOn(model: LocalModel, enabled: boolean): { kind: BackendKind; title: string } | null {
  const [planned, setPlanned] = useState<LocalBackend | null>(null);
  useEffect(() => {
    if (!enabled) return;
    api
      .localBackend()
      .then(setPlanned)
      .catch(() => {});
  }, [enabled]);
  if (!enabled || model.state.state !== "ready") return null;
  if (model.backend) return { kind: backendKind(model.backend), title: model.backend };
  if (planned) return { kind: planned.kind, title: planned.device };
  return null;
}

/**
 * The model in use and where it stands: its name, languages and size, then
 * the download as it goes, until it is ready.
 */
export function LocalModelCard({
  local,
  model,
  actions = true,
  inset = false,
  showBackend = false,
}: {
  local: LocalModels;
  model: LocalModel;
  /** Its own Download and Try again buttons; off where the screen's main button does it. */
  actions?: boolean;
  /** Inside another card. */
  inset?: boolean;
  /** Says what the engine runs on, once it is loaded. */
  showBackend?: boolean;
}) {
  const { t } = useT();
  const size = useSize();
  const languages = useModelLanguages();
  const percent = usePercent();
  const progress = useProgressText();
  const explain = useDownloadError();
  const state = model.state;
  const runsOn = useRunsOn(model, showBackend);

  let side = null;
  switch (state.state) {
    case "missing":
      side = actions && (
        <Button size="sm" variant="primary" onClick={() => local.download(model.id)}>
          <Download className="size-3.5" />
          {t("local.download")}
        </Button>
      );
      break;
    case "downloading":
      side = (
        <span className="text-[13px] font-medium text-text-2 tabular-nums">
          {percent(downloadShare(state))}
        </span>
      );
      break;
    case "verifying":
      side = (
        <span className="inline-flex items-center gap-1.5 text-[12.5px] text-text-3" role="status">
          <LoaderCircle className="size-3.5 animate-spin" />
          {t("local.verifying")}
        </span>
      );
      break;
    case "ready":
      // A round tick, as in the scene: the name and languages keep the room.
      side = (
        <span
          className="rise flex size-7 shrink-0 items-center justify-center rounded-full bg-success-soft text-success"
          title={t("local.ready")}
        >
          <Check className="size-4" strokeWidth={2.5} />
          <span className="sr-only">{t("local.ready")}</span>
        </span>
      );
      break;
    case "failed":
      side = actions && (
        <Button size="sm" onClick={() => local.download(model.id)}>
          <RotateCw className="size-3.5" />
          {t("problems.retry")}
        </Button>
      );
      break;
  }

  return (
    <div
      className={
        inset
          ? "rounded-xl bg-surface-2 px-4 py-3.5 shadow-[inset_0_0_0_1px_var(--border)]"
          : "rounded-card bg-surface px-4 py-3.5 shadow-card"
      }
    >
      <div className="flex items-center gap-3.5">
        <IconTile icon={Cpu} tint="stone" />
        <div className="min-w-0 flex-1">
          <div className="flex flex-wrap items-center gap-x-2 gap-y-0.5 text-[14.5px] font-semibold text-text">
            {model.name}
            {model.recommended && (
              <span className="rounded-full bg-success-soft px-2 py-0.5 text-[11px] font-medium whitespace-nowrap text-success">
                {t("providers.recommended")}
              </span>
            )}
          </div>
          <div className="text-[12.5px] text-text-3">
            {languages(model)} · {size(model.sizeBytes)}
          </div>
          {runsOn && (
            <div className="rise mt-0.5 text-[12px] text-text-3" title={runsOn.title}>
              {t(RUNS_ON[runsOn.kind])}
            </div>
          )}
        </div>
        {side}
      </div>
      {state.state === "downloading" && (
        <div className="rise mt-3.5 flex flex-col gap-2">
          <ProgressBar share={downloadShare(state)} />
          <div className="flex items-center justify-between gap-3">
            <span className="text-[12.5px] text-text-3 tabular-nums">{progress(model)}</span>
            <button
              type="button"
              onClick={() => local.cancel(model.id)}
              className="text-[12.5px] font-medium text-text-2 hover:text-text"
            >
              {t("common.cancel")}
            </button>
          </div>
        </div>
      )}
      {state.state === "failed" && (
        <p className="rise mt-3 text-[12.5px] leading-snug text-accent-text">{explain(state.error)}</p>
      )}
    </div>
  );
}

/** The other model, for those who need it: a plain list of the two. */
export function LocalModelSelect({ local }: { local: LocalModels }) {
  const { t } = useT();
  const size = useSize();
  const languages = useModelLanguages();
  if (!local.chosen) return null;
  return (
    <Select value={local.chosen.id} onChange={(e) => local.choose(e.target.value as LocalModelId)}>
      {local.models.map((m) => (
        <option key={m.id} value={m.id}>
          {`${m.name} · ${languages(m)} · ${size(m.sizeBytes)}${m.recommended ? ` · ${t("providers.recommended")}` : ""}`}
        </option>
      ))}
    </Select>
  );
}

/** The model that transcribes on this computer, while it isn't here yet. */
export function LocalBanner({ local, trying = false }: { local: LocalModels; trying?: boolean }) {
  const { t } = useT();
  const { settings, status } = useParler();
  const size = useSize();
  const percent = usePercent();
  const progress = useProgressText();
  const explain = useDownloadError();
  const model = local.chosen;
  if (settings.provider !== LOCAL || !model) return null;
  const state = model.state;

  const figures = state.state === "downloading" ? `${progress(model)} · ${percent(downloadShare(state))}` : "";

  switch (state.state) {
    case "downloading":
    case "verifying":
      return (
        <Banner tone="info">
          {/* Wide on Home: the figures beside the words; narrow in onboarding: under the bar. */}
          <span className="flex flex-col gap-2">
            <span className={trying ? "" : "flex items-baseline justify-between gap-3"}>
              {state.state === "verifying"
                ? t("local.verifying")
                : t(trying ? "local.tryDownloading" : "local.homeDownloading", { model: model.name })}
              {!trying && figures && <span className="text-[12.5px] text-text-2 tabular-nums">{figures}</span>}
            </span>
            <ProgressBar share={downloadShare(state)} />
            {trying && figures && <span className="text-[12.5px] text-text-3 tabular-nums">{figures}</span>}
          </span>
        </Banner>
      );
    case "missing": {
      // After a dictation ran into it, the problem banner says the same.
      const problem = status.problem;
      if (!trying && problem?.kind === "transcription" && problem.error.kind === "modelMissing") return null;
      return (
        <Banner
          tone="warning"
          actions={
            <Button size="sm" variant="primary" onClick={() => local.download(model.id)}>
              <Download className="size-3.5" />
              {t("local.downloadSize", { size: size(model.sizeBytes) })}
            </Button>
          }
        >
          {t("local.homeMissing", { model: model.name, size: size(model.sizeBytes) })}
        </Banner>
      );
    }
    case "failed":
      return (
        <Banner
          tone="error"
          actions={
            <Button size="sm" onClick={() => local.download(model.id)}>
              <RotateCw className="size-3.5" />
              {t("problems.retry")}
            </Button>
          }
        >
          {explain(state.error)}
        </Banner>
      );
    default:
      return null;
  }
}
