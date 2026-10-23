import { describe, expect, it } from "vitest";
import { detectLocale, translate } from "./index";
import en from "./locales/en.json";
import fr from "./locales/fr.json";

function keys(node: unknown, prefix = ""): string[] {
  if (typeof node !== "object" || node === null) return [prefix];
  return Object.entries(node).flatMap(([key, value]) =>
    keys(value, prefix ? `${prefix}.${key}` : key),
  );
}

describe("translate", () => {
  it("reads nested keys in each language", () => {
    expect(translate("en", "settings.title")).toBe("Settings");
    expect(translate("fr", "settings.title")).toBe("Réglages");
  });

  it("fills in parameters", () => {
    expect(
      translate("fr", "problems.unauthorized", { provider: "Groq" }),
    ).toContain("Groq");
  });

  it("shows the key when a string is missing everywhere", () => {
    expect(translate("en", "no.such.key")).toBe("no.such.key");
  });
});

describe("locales", () => {
  it("translate every English string into French", () => {
    expect(keys(fr).sort()).toEqual(keys(en).sort());
  });
});

describe("detectLocale", () => {
  it("picks French for any French variant", () => {
    expect(detectLocale("fr-CA")).toBe("fr");
    expect(detectLocale("en-GB")).toBe("en");
    expect(detectLocale("de")).toBe("en");
  });
});
