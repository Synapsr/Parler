import en from "./locales/en.json";
import fr from "./locales/fr.json";

export type Locale = "en" | "fr";
export type LanguagePref = "auto" | Locale;
export type Params = Record<string, string | number>;

const dictionaries: Record<Locale, unknown> = { en, fr };

export function detectLocale(language?: string): Locale {
  const lang = (
    language ?? (typeof navigator !== "undefined" ? navigator.language : "en")
  ).toLowerCase();
  return lang.startsWith("fr") ? "fr" : "en";
}

export function resolveLocale(pref: LanguagePref): Locale {
  return pref === "auto" ? detectLocale() : pref;
}

function lookup(dictionary: unknown, key: string): string | undefined {
  const value = key
    .split(".")
    .reduce<unknown>(
      (node, part) =>
        node && typeof node === "object"
          ? (node as Record<string, unknown>)[part]
          : undefined,
      dictionary,
    );
  return typeof value === "string" ? value : undefined;
}

/**
 * The text for `key`, falling back to English, then to the key itself so a
 * missing string is visible rather than blank.
 */
export function translate(locale: Locale, key: string, params?: Params): string {
  let text = lookup(dictionaries[locale], key) ?? lookup(dictionaries.en, key);
  if (text === undefined) return key;
  for (const [name, value] of Object.entries(params ?? {})) {
    text = text.replaceAll(`{${name}}`, String(value));
  }
  return text;
}
