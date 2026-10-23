import { createContext, useContext, useMemo, type ReactNode } from "react";
import {
  resolveLocale,
  translate,
  type LanguagePref,
  type Locale,
  type Params,
} from "./index";

interface LocaleValue {
  locale: Locale;
  t: (key: string, params?: Params) => string;
}

const LocaleContext = createContext<LocaleValue>({
  locale: "en",
  t: (key, params) => translate("en", key, params),
});

export function LocaleProvider({
  pref,
  children,
}: {
  pref: LanguagePref;
  children: ReactNode;
}) {
  const value = useMemo<LocaleValue>(() => {
    const locale = resolveLocale(pref);
    return { locale, t: (key, params) => translate(locale, key, params) };
  }, [pref]);
  return (
    <LocaleContext.Provider value={value}>{children}</LocaleContext.Provider>
  );
}

export const useT = () => useContext(LocaleContext);
