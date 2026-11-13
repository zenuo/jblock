import type { Locale, MessageKey } from "./types";
import { GC_MESSAGES } from "./gcMessages";
import { en } from "./locales/en";
import { es } from "./locales/es";
import { fr } from "./locales/fr";
import { ja } from "./locales/ja";
import { ko } from "./locales/ko";
import { nl } from "./locales/nl";
import { pt } from "./locales/pt";
import { zh } from "./locales/zh";

export { en, zh, pt, es, nl, fr, ja, ko };

export const catalogs: Record<Locale, Record<MessageKey, string>> = {
  en: { ...en, ...GC_MESSAGES.en },
  zh: { ...zh, ...GC_MESSAGES.zh },
  pt: { ...pt, ...GC_MESSAGES.pt },
  es: { ...es, ...GC_MESSAGES.es },
  nl: { ...nl, ...GC_MESSAGES.nl },
  fr: { ...fr, ...GC_MESSAGES.fr },
  ja: { ...ja, ...GC_MESSAGES.ja },
  ko: { ...ko, ...GC_MESSAGES.ko },
};
