import de from "./locales/de.json";
import en from "./locales/en.json";
import es from "./locales/es.json";
import fr from "./locales/fr.json";
import it from "./locales/it.json";
import ja from "./locales/ja.json";
import ko from "./locales/ko.json";
import nl from "./locales/nl.json";
import pl from "./locales/pl.json";
import pt from "./locales/pt.json";
import tr from "./locales/tr.json";
import zh from "./locales/zh.json";
import { LOCALES, type Locale } from "./languages";

export { LOCALES, type Locale };
export type LanguagePref = "auto" | Locale;
export type Params = Record<string, string | number>;

export const dictionaries: Record<Locale, unknown> = {
  en,
  fr,
  es,
  de,
  it,
  pt,
  nl,
  pl,
  tr,
  ja,
  zh,
  ko,
};

/** The interface language closest to `language` (a BCP 47 tag), English
 * when Parler doesn't speak it. */
export function detectLocale(language?: string): Locale {
  const lang = (
    language ?? (typeof navigator !== "undefined" ? navigator.language : "en")
  ).toLowerCase();
  const match = LOCALES.find((l) => lang === l.code || lang.startsWith(`${l.code}-`));
  return match?.code ?? "en";
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

/** Sentences side by side: Chinese and Japanese need no space after 。 */
export function joinSentences(locale: Locale, ...sentences: string[]): string {
  return sentences.join(locale === "ja" || locale === "zh" ? "" : " ");
}
