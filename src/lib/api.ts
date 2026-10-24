// Typed access to the Rust backend. The shapes mirror src-tauri/src.
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

export type ShortcutMode = "smart" | "hold" | "toggle";
export type Output = "paste" | "copy";
export type UiLanguage = "auto" | "en" | "fr";
export type Theme = "system" | "light" | "dark";

export interface Settings {
  onboarded: boolean;
  provider: string;
  apiKeys: Record<string, string>;
  models: Record<string, string>;
  customBaseUrl: string;
  language: string;
  vocabulary: string;
  shortcut: string;
  shortcutMode: ShortcutMode;
  microphone: string | null;
  output: Output;
  sounds: boolean;
  uiLanguage: UiLanguage;
  theme: Theme;
}

export interface Provider {
  id: string;
  label: string;
  baseUrl: string;
  defaultModel: string;
  models: string[];
  requiresApiKey: boolean;
  apiKeyUrl: string | null;
}

export type TranscribeError =
  | { kind: "unknownProvider" }
  | { kind: "missingApiKey"; provider: string }
  | { kind: "missingBaseUrl" }
  | { kind: "unauthorized"; provider: string }
  | { kind: "rateLimited"; provider: string }
  | { kind: "offline"; provider: string }
  | { kind: "timeout"; provider: string }
  | { kind: "rejected"; provider: string; status: number; message: string }
  | { kind: "invalidResponse"; provider: string };

export type PasteError =
  | { kind: "noClipboard" }
  | { kind: "needsAccessibility" }
  | { kind: "failed"; detail: string };

export type Problem =
  | { kind: "transcription"; error: TranscribeError }
  | { kind: "microphone"; detail: string }
  | { kind: "paste"; error: PasteError }
  | { kind: "shortcut"; shortcut: string; detail: string };

export type DictationState = "idle" | "recording" | "transcribing";

export interface Status {
  state: DictationState;
  problem: Problem | null;
  canRetry: boolean;
}

export interface HistoryEntry {
  id: number;
  text: string;
  durationMs: number;
  provider: string;
}

export interface Stats {
  dictations: number;
  words: number;
  speechMs: number;
}

export interface Microphone {
  id: string;
  name: string;
  isDefault: boolean;
}

export interface TrayLabels {
  open: string;
  toggle: string;
  copyLast: string;
  history: string;
  quit: string;
}

export interface AppInfo {
  platform: "macos" | "windows" | "linux" | string;
  version: string;
}

export type OverlayPhase =
  | "connecting"
  | "recording"
  | "transcribing"
  | "empty"
  | "error"
  | "hidden";

export interface OverlayEvent {
  phase: OverlayPhase;
  sounds: boolean;
  problem: Problem | null;
}

export const api = {
  appInfo: () => invoke<AppInfo>("app_info"),
  getSettings: () => invoke<Settings>("get_settings"),
  saveSettings: (settings: Settings) =>
    invoke<Settings>("save_settings", { settings }),
  getProviders: () => invoke<Provider[]>("get_providers"),
  testProvider: (settings: Settings) =>
    invoke<null>("test_provider", { settings }),
  listMicrophones: () => invoke<Microphone[]>("list_microphones"),
  getHistory: () => invoke<HistoryEntry[]>("get_history"),
  getStats: () => invoke<Stats>("get_stats"),
  deleteHistoryEntry: (id: number) => invoke("delete_history_entry", { id }),
  clearHistory: () => invoke("clear_history"),
  copyText: (text: string) => invoke("copy_text", { text }),
  getStatus: () => invoke<Status>("get_status"),
  toggleDictation: () => invoke("toggle_dictation"),
  retryDictation: () => invoke("retry_dictation"),
  setTrayLabels: (labels: TrayLabels) => invoke("set_tray_labels", { labels }),
  suspendShortcut: () => invoke("suspend_shortcut"),
  resumeShortcut: () => invoke("resume_shortcut"),
};

export const events = {
  status: (handler: (status: Status) => void): Promise<UnlistenFn> =>
    listen<Status>("parler://status", (e) => handler(e.payload)),
  history: (handler: (entries: HistoryEntry[]) => void): Promise<UnlistenFn> =>
    listen<HistoryEntry[]>("parler://history", (e) => handler(e.payload)),
  /** The tray asks the window to show a page. */
  navigate: (handler: (page: string) => void): Promise<UnlistenFn> =>
    listen<string>("parler://navigate", (e) => handler(e.payload)),
  stats: (handler: (stats: Stats) => void): Promise<UnlistenFn> =>
    listen<Stats>("parler://stats", (e) => handler(e.payload)),
  overlay: (handler: (event: OverlayEvent) => void): Promise<UnlistenFn> =>
    listen<OverlayEvent>("parler://overlay", (e) => handler(e.payload)),
  level: (handler: (level: number) => void): Promise<UnlistenFn> =>
    listen<number>("parler://level", (e) => handler(e.payload)),
};

/** The model a provider will use with these settings. */
export function modelFor(settings: Settings, provider: Provider): string {
  return settings.models[provider.id]?.trim() || provider.defaultModel;
}
