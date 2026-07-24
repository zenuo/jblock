import type { Locale, Messages } from "./types";
import { en } from "./locales/en";
import { es } from "./locales/es";
import { fr } from "./locales/fr";
import { ja } from "./locales/ja";
import { ko } from "./locales/ko";
import { nl } from "./locales/nl";
import { pt } from "./locales/pt";
import { zh } from "./locales/zh";

export { en, zh, pt, es, nl, fr, ja, ko };

export const catalogs: Record<Locale, Messages> = {
  en,
  zh,
  pt,
  es,
  nl,
  fr,
  ja,
  ko,
};
