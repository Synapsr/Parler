import { describe, expect, it } from "vitest";
import { LOCALES, translate } from "../i18n";
import { describeProblem, isMicrophoneProblem } from "./problems";
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
  { kind: "transcription", error: { kind: "signedOut", provider: "Parler Cloud" } },
  {
    kind: "transcription",
    error: { kind: "subscriptionRequired", provider: "Parler Cloud", manageUrl: "https://cloud.parler.synapsr.io/account" },
  },
  { kind: "transcription", error: { kind: "fairUseDay", provider: "Parler Cloud" } },
  { kind: "transcription", error: { kind: "fairUseMonth", provider: "Parler Cloud" } },
  { kind: "microphone", detail: "gone" },
  { kind: "microphoneDenied" },
  { kind: "noSound" },
  { kind: "paste", error: { kind: "needsAccessibility" } },
  { kind: "paste", error: { kind: "noClipboard" } },
  { kind: "paste", error: { kind: "failed", detail: "x" } },
  { kind: "shortcut", shortcut: "Alt+Space", detail: "taken" },
  { kind: "shortcut", shortcut: "Fn", detail: "accessibility" },
  { kind: "shortcut", shortcut: "RightCommand", detail: "accessibility" },
];

describe("describeProblem", () => {
  it("has a translated sentence and pill text for every problem", () => {
    for (const problem of problems) {
      for (const { code: locale } of LOCALES) {
        const { key, short, params } = describeProblem(problem, undefined, locale);
        expect(translate(locale, key, params)).not.toBe(key);
        expect(translate(locale, short)).not.toBe(short);
        expect(translate(locale, key, params)).not.toMatch(/\{\w+\}/);
      }
    }
  });

  it("names the user's own server in the interface language", () => {
    const custom: Problem = { kind: "transcription", error: { kind: "unauthorized", provider: "Custom" } };
    expect(describeProblem(custom, "Votre serveur").params.provider).toBe("Votre serveur");
    expect(describeProblem(problems[3], "Votre serveur").params.provider).toBe("Groq");
  });

  it("sends key problems to the settings", () => {
    expect(describeProblem(problems[3]).needsSettings).toBe(true);
    expect(describeProblem(problems[5]).needsSettings).toBe(false);
  });

  it("offers the fix for Parler Cloud problems", () => {
    const signedOut = describeProblem(problems[9]);
    expect(signedOut.needsSignIn).toBe(true);
    expect(signedOut.short).toBe("overlay.signedOut");
    const plan = describeProblem(problems[10]);
    expect(plan.manageUrl).toBe("https://cloud.parler.synapsr.io/account");
    expect(plan.needsSettings).toBe(false);
    expect(describeProblem(problems[11]).params.time).toBeTruthy();
    expect(describeProblem(problems[12]).params.date).toBeTruthy();
  });

  it("sends a silent or forbidden microphone to its privacy settings", () => {
    for (const problem of [{ kind: "noSound" }, { kind: "microphoneDenied" }] as Problem[]) {
      const text = describeProblem(problem);
      expect(text.key).toBe(`problems.${problem.kind}`);
      expect(text.short).toBe(`overlay.${problem.kind}`);
      expect(text.needsMicrophone).toBe(true);
      expect(isMicrophoneProblem(problem)).toBe(true);
    }
    expect(describeProblem({ kind: "microphone", detail: "gone" }).needsMicrophone).toBe(false);
    expect(isMicrophoneProblem(problems[5])).toBe(false);
  });

  it("sends keys used alone to the Accessibility settings", () => {
    for (const problem of problems.slice(-2)) {
      const text = describeProblem(problem);
      expect(text.key).toBe("problems.singleKey");
      expect(text.needsAccessibility).toBe(true);
      expect(text.needsSettings).toBe(false);
    }
  });
});
