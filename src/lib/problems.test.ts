import { describe, expect, it } from "vitest";
import { translate } from "../i18n";
import { describeProblem } from "./problems";
import type { Problem } from "./api";

const problems: Problem[] = [
  { kind: "transcription", error: { kind: "unknownProvider" } },
  { kind: "transcription", error: { kind: "missingApiKey", provider: "Groq" } },
  { kind: "transcription", error: { kind: "missingBaseUrl" } },
  { kind: "transcription", error: { kind: "unauthorized", provider: "Groq" } },
  { kind: "transcription", error: { kind: "rateLimited", provider: "Groq" } },
  { kind: "transcription", error: { kind: "offline", provider: "Groq" } },
  { kind: "transcription", error: { kind: "timeout", provider: "Groq" } },
  {
    kind: "transcription",
    error: { kind: "rejected", provider: "Groq", status: 400, message: "bad" },
  },
  { kind: "transcription", error: { kind: "invalidResponse", provider: "Groq" } },
  { kind: "microphone", detail: "gone" },
  { kind: "paste", error: { kind: "needsAccessibility" } },
  { kind: "paste", error: { kind: "noClipboard" } },
  { kind: "paste", error: { kind: "failed", detail: "x" } },
  { kind: "shortcut", shortcut: "Alt+Space", detail: "taken" },
];

describe("describeProblem", () => {
  it("has a translated sentence and pill text for every problem", () => {
    for (const problem of problems) {
      const { key, short, params } = describeProblem(problem);
      for (const locale of ["en", "fr"] as const) {
        expect(translate(locale, key, params)).not.toBe(key);
        expect(translate(locale, short)).not.toBe(short);
        expect(translate(locale, key, params)).not.toMatch(/\{\w+\}/);
      }
    }
  });

  it("sends key problems to the settings", () => {
    expect(describeProblem(problems[3]).needsSettings).toBe(true);
    expect(describeProblem(problems[5]).needsSettings).toBe(false);
  });
});
