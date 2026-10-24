import type { Params } from "../i18n";
import type { Problem } from "./api";
import { formatDay, formatTime, nextUtcDay, nextUtcMonth } from "./cloud";
import { FN } from "./shortcut";

export interface ProblemText {
  /** Full sentence for the main window, under `problems.`. */
  key: string;
  /** A few words for the dictation pill, under `overlay.`. */
  short: string;
  params: Params;
  /** Fixing it means visiting the settings. */
  needsSettings: boolean;
  needsAccessibility: boolean;
  /** Fixing it means signing in to Parler Cloud again. */
  needsSignIn: boolean;
  /** Fixing it means choosing a plan on this page. */
  manageUrl?: string;
}

/** The backend's label for the user's own server, shown as `customName` instead. */
const CUSTOM_LABEL = "Custom";

export function describeProblem(
  problem: Problem,
  customName?: string,
  locale = "en",
): ProblemText {
  const text = (
    key: string,
    params: Params = {},
    flags: Partial<ProblemText> = {},
  ): ProblemText => ({
    key: `problems.${key}`,
    short: `overlay.${key in SHORT ? key : "error"}`,
    params,
    needsSettings: false,
    needsAccessibility: false,
    needsSignIn: false,
    ...flags,
  });

  switch (problem.kind) {
    case "transcription": {
      const error = problem.error;
      const label = "provider" in error ? error.provider : "";
      const provider = label === CUSTOM_LABEL && customName ? customName : label;
      switch (error.kind) {
        case "unknownProvider":
        case "missingBaseUrl":
        case "missingApiKey":
        case "unauthorized":
          return text(error.kind, { provider }, { needsSettings: true });
        case "signedOut":
          return text("signedOut", { provider }, { needsSignIn: true });
        case "subscriptionRequired":
          return text("subscriptionRequired", { provider }, { manageUrl: error.manageUrl });
        // Fair use counts by UTC day and month: say when, in local time.
        case "fairUseDay":
          return text("fairUseDay", { provider, time: formatTime(nextUtcDay(), locale) });
        case "fairUseMonth":
          return text("fairUseMonth", { provider, date: formatDay(nextUtcMonth(), locale) });
        case "rejected":
          return text("rejected", {
            provider,
            status: error.status,
            message: error.message,
          });
        default:
          return text(error.kind, { provider });
      }
    }
    case "microphone":
      return text("microphone", { detail: problem.detail });
    case "paste":
      switch (problem.error.kind) {
        case "needsAccessibility":
          return text("needsAccessibility", {}, { needsAccessibility: true });
        case "noClipboard":
          return text("noClipboard");
        case "failed":
          return text("pasteFailed", { detail: problem.error.detail });
      }
      break;
    case "shortcut":
      // The fn key can't be taken by another app; it waits for Accessibility.
      if (problem.shortcut === FN) {
        return text("fnKey", {}, { needsAccessibility: true });
      }
      return text(
        "shortcut",
        { shortcut: problem.shortcut },
        { needsSettings: true },
      );
  }
  return text("error");
}

// Problems with their own short wording on the pill.
const SHORT: Record<string, true> = {
  unknownProvider: true,
  missingApiKey: true,
  missingBaseUrl: true,
  unauthorized: true,
  rateLimited: true,
  offline: true,
  timeout: true,
  rejected: true,
  invalidResponse: true,
  signedOut: true,
  subscriptionRequired: true,
  fairUseDay: true,
  fairUseMonth: true,
  microphone: true,
};
