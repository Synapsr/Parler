import type { Params } from "../i18n";
import type { Problem } from "./api";

export interface ProblemText {
  /** Full sentence for the main window, under `problems.`. */
  key: string;
  /** A few words for the dictation pill, under `overlay.`. */
  short: string;
  params: Params;
  /** Fixing it means visiting the settings. */
  needsSettings: boolean;
  needsAccessibility: boolean;
}

export function describeProblem(problem: Problem): ProblemText {
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
    ...flags,
  });

  switch (problem.kind) {
    case "transcription": {
      const error = problem.error;
      const provider = "provider" in error ? error.provider : "";
      switch (error.kind) {
        case "unknownProvider":
        case "missingBaseUrl":
        case "missingApiKey":
        case "unauthorized":
          return text(error.kind, { provider }, { needsSettings: true });
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
  microphone: true,
};
