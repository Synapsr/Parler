import { describe, expect, it } from "vitest";
import { detectLocale, dictionaries, LOCALES, translate } from "./index";
import en from "./locales/en.json";

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

function strings(node: unknown, prefix = ""): [string, string][] {
  if (typeof node === "string") return [[prefix, node]];
  if (typeof node !== "object" || node === null) return [];
  return Object.entries(node).flatMap(([key, value]) =>
    strings(value, prefix ? `${prefix}.${key}` : key),
  );
}

const placeholders = (text: string) => (text.match(/\{\w+\}/g) ?? []).sort();

describe("locales", () => {
  for (const { code } of LOCALES) {
    it(`${code} has every English string, with the same placeholders`, () => {
      const dictionary = dictionaries[code];
      expect(keys(dictionary).sort()).toEqual(keys(en).sort());
      const translated = new Map(strings(dictionary));
      for (const [key, english] of strings(en)) {
        expect(placeholders(translated.get(key) ?? ""), `${code}: ${key}`).toEqual(
          placeholders(english),
        );
      }
    });
  }
});

describe("detectLocale", () => {
  it("matches a language and its regional variants", () => {
    expect(detectLocale("fr-CA")).toBe("fr");
    expect(detectLocale("en-GB")).toBe("en");
    expect(detectLocale("de-AT")).toBe("de");
    expect(detectLocale("pt-BR")).toBe("pt");
    expect(detectLocale("zh-CN")).toBe("zh");
  });

  it("falls back to English", () => {
    expect(detectLocale("sv-SE")).toBe("en");
  });
});
