// Typed access to the Rust backend. The shapes mirror src-tauri/src.
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type { LanguagePref } from "../i18n";

export type ShortcutMode = "smart" | "hold" | "toggle";
export type Output = "paste" | "copy";
export type UiLanguage = LanguagePref;
export type Theme = "system" | "light" | "dark";

export interface Settings {
  onboarded: boolean;
  provider: string;
  /** Parler Cloud's entry is the device token, written by the backend only. */
  apiKeys: Record<string, string>;
  /** Kept by the backend: the service to go back to on signing out. */
  providerBeforeCloud?: string | null;
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
  | { kind: "invalidResponse"; provider: string }
  | { kind: "signedOut"; provider: string }
  | { kind: "subscriptionRequired"; provider: string; manageUrl: string }
  | { kind: "fairUseDay"; provider: string }
  | { kind: "fairUseMonth"; provider: string };

/** The provider id of Parler Cloud, the plan that replaces an API key. */
export const CLOUD = "cloud";

export type CloudStatus = "none" | "trialing" | "active" | "past_due" | "canceled" | "unknown";

export interface CloudAccount {
  email: string;
  status: CloudStatus;
  plan: "monthly" | "yearly" | string | null;
  trialEndsAt: string | null;
  renewsAt: string | null;
  cancelAtPeriodEnd: boolean;
  usage: {
    daySeconds: number;
    monthSeconds: number;
    dayLimitSeconds: number;
    monthLimitSeconds: number;
  };
  manageUrl: string;
}

/** A sign-in in progress: the code to check and the page it was opened on. */
export interface DeviceStart {
  deviceCode: string;
  userCode: string;
  verificationUrl: string;
  /** Seconds. */
  expiresIn: number;
  /** Seconds between polls. */
  interval: number;
  deviceName: string;
}

export type SignInPoll =
  | { status: "pending" }
  | { status: "approved"; account: CloudAccount }
  | { status: "denied" }
  | { status: "expired" };

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
  cloudSignInStart: () => invoke<DeviceStart>("cloud_sign_in_start"),
  cloudSignInPoll: (deviceCode: string) =>
    invoke<SignInPoll>("cloud_sign_in_poll", { deviceCode }),
  /** `null` when this computer isn't signed in. */
  cloudAccount: () => invoke<CloudAccount | null>("cloud_account"),
  cloudSignOut: () => invoke<Settings>("cloud_sign_out"),
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
  /** macOS: what the system does with a 🌐 press (0 = nothing, null = its default). */
  globeKey: () => invoke<{ action: number | null }>("globe_key"),
  openKeyboardSettings: () => invoke("open_keyboard_settings"),
};

export const events = {
  /** The backend changed the settings (Parler Cloud sign-in or sign-out). */
  settings: (handler: (settings: Settings) => void): Promise<UnlistenFn> =>
    listen<Settings>("parler://settings", (e) => handler(e.payload)),
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
