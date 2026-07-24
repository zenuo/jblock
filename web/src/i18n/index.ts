import {
  createContext,
  createElement,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useState,
  type ReactNode,
} from "react";
import { catalogs } from "./messages";
import {
  LOCALES,
  LOCALE_HTML_LANG,
  LOCALE_LABELS,
  type Locale,
  type MessageKey,
  type TranslateFn,
} from "./types";

export { LOCALES, LOCALE_HTML_LANG, LOCALE_LABELS };
export type { Locale, MessageKey, TranslateFn };

const STORAGE_KEY = "jblock.locale";

/** Prefix match order: longer / more specific tags first when needed. */
const BROWSER_LOCALE_PREFIXES: ReadonlyArray<readonly [string, Locale]> = [
  ["zh", "zh"],
  ["pt", "pt"],
  ["es", "es"],
  ["nl", "nl"],
  ["fr", "fr"],
  ["ja", "ja"],
  ["ko", "ko"],
  ["en", "en"],
];

/** Map browser language tags to a supported locale (default: en). */
export function detectBrowserLocale(
  languages: readonly string[] = typeof navigator !== "undefined"
    ? navigator.languages?.length
      ? navigator.languages
      : [navigator.language]
    : ["en"],
): Locale {
  for (const raw of languages) {
    const tag = (raw || "").toLowerCase();
    for (const [prefix, locale] of BROWSER_LOCALE_PREFIXES) {
      if (tag === prefix || tag.startsWith(`${prefix}-`)) return locale;
    }
  }
  return "en";
}

export function isLocale(value: string): value is Locale {
  return (LOCALES as readonly string[]).includes(value);
}

export function readStoredLocale(): Locale | null {
  try {
    const raw = localStorage.getItem(STORAGE_KEY);
    if (raw && isLocale(raw)) return raw;
  } catch {
    /* ignore */
  }
  return null;
}

export function storeLocale(locale: Locale): void {
  try {
    localStorage.setItem(STORAGE_KEY, locale);
  } catch {
    /* ignore */
  }
}

/** Initial locale: stored preference, else browser detect. */
export function resolveInitialLocale(): Locale {
  return readStoredLocale() ?? detectBrowserLocale();
}

export function htmlLangFor(locale: Locale): string {
  return LOCALE_HTML_LANG[locale];
}

export function createTranslator(locale: Locale): TranslateFn {
  const messages = catalogs[locale];
  const fallback = catalogs.en;
  return (key, params) => {
    let text = messages[key] ?? fallback[key] ?? key;
    if (params) {
      for (const [k, v] of Object.entries(params)) {
        text = text.split(`{${k}}`).join(String(v));
      }
    }
    return text;
  };
}

interface I18nValue {
  locale: Locale;
  setLocale: (locale: Locale) => void;
  t: TranslateFn;
}

const I18nContext = createContext<I18nValue | null>(null);

export function I18nProvider({ children }: { children: ReactNode }) {
  const [locale, setLocaleState] = useState<Locale>(() => resolveInitialLocale());

  const setLocale = useCallback((next: Locale) => {
    setLocaleState(next);
    storeLocale(next);
  }, []);

  useEffect(() => {
    document.documentElement.lang = htmlLangFor(locale);
  }, [locale]);

  const t = useMemo(() => createTranslator(locale), [locale]);
  const value = useMemo(
    () => ({ locale, setLocale, t }),
    [locale, setLocale, t],
  );

  return createElement(I18nContext.Provider, { value }, children);
}

export function useI18n(): I18nValue {
  const ctx = useContext(I18nContext);
  if (!ctx) {
    throw new Error("useI18n must be used within I18nProvider");
  }
  return ctx;
}
