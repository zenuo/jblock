export type Locale = "en" | "zh" | "pt" | "es" | "nl" | "fr" | "ja" | "ko";

export const LOCALES: readonly Locale[] = [
  "en",
  "zh",
  "pt",
  "es",
  "nl",
  "fr",
  "ja",
  "ko",
] as const;

/** Native-script labels shown in the language menu. */
export const LOCALE_LABELS: Record<Locale, string> = {
  en: "English",
  zh: "中文",
  pt: "Português",
  es: "Español",
  nl: "Nederlands",
  fr: "Français",
  ja: "日本語",
  ko: "한국어",
};

/** BCP 47 tags for <html lang> / report documents. */
export const LOCALE_HTML_LANG: Record<Locale, string> = {
  en: "en",
  zh: "zh-CN",
  pt: "pt",
  es: "es",
  nl: "nl",
  fr: "fr",
  ja: "ja",
  ko: "ko",
};

export type MessageKey =
  | "app.title"
  | "app.tagline"
  | "app.generateJava"
  | "app.chooseDump"
  | "app.loadSample"
  | "app.exportHtml"
  | "app.exportPdf"
  | "app.hint"
  | "app.dropOverlay"
  | "app.loadingWasm"
  | "app.analyzing"
  | "app.errorPrefix"
  | "app.wasmLoadFailed"
  | "app.language"
  | "codegen.title"
  | "codegen.close"
  | "codegen.blurb"
  | "codegen.scenario"
  | "codegen.deadlock"
  | "codegen.lockContention"
  | "codegen.threadPoolExhaustion"
  | "codegen.syncIoHotspot"
  | "codegen.dangerousHotLock"
  | "codegen.connectionPoolStarve"
  | "codegen.futureLatchDeadlock"
  | "codegen.loggingAppenderContention"
  | "codegen.busyWaitSpin"
  | "codegen.conditionStarvation"
  | "codegen.threads"
  | "codegen.generate"
  | "codegen.download"
  | "findings.title"
  | "findings.meta"
  | "findings.deadlockTitle"
  | "findings.hotLockTitle"
  | "findings.hotLockDetail"
  | "findings.blockedTitle"
  | "findings.blockedDetail"
  | "findings.cleanTitle"
  | "findings.cleanDetail"
  | "findings.poolExhaustionTitle"
  | "findings.poolExhaustionDetail"
  | "findings.syncIoHotspotTitle"
  | "findings.syncIoHotspotDetail"
  | "findings.dangerousHotLockTitle"
  | "findings.dangerousHotLockDetail"
  | "findings.connectionPoolTitle"
  | "findings.connectionPoolDetail"
  | "findings.futureLatchTitle"
  | "findings.futureLatchDetail"
  | "findings.loggingAppenderTitle"
  | "findings.loggingAppenderDetail"
  | "findings.busyWaitTitle"
  | "findings.busyWaitDetail"
  | "findings.conditionStarvationTitle"
  | "findings.conditionStarvationDetail"
  | "findings.legendBtn"
  | "legend.close"
  | "legend.deadlockTitle"
  | "legend.deadlockBody"
  | "legend.hotLockTitle"
  | "legend.hotLockBody"
  | "legend.blockedTitle"
  | "legend.blockedBody"
  | "legend.cleanTitle"
  | "legend.cleanBody"
  | "legend.poolExhaustionTitle"
  | "legend.poolExhaustionBody"
  | "legend.syncIoHotspotTitle"
  | "legend.syncIoHotspotBody"
  | "legend.dangerousHotLockTitle"
  | "legend.dangerousHotLockBody"
  | "legend.connectionPoolTitle"
  | "legend.connectionPoolBody"
  | "legend.futureLatchTitle"
  | "legend.futureLatchBody"
  | "legend.loggingAppenderTitle"
  | "legend.loggingAppenderBody"
  | "legend.busyWaitTitle"
  | "legend.busyWaitBody"
  | "legend.conditionStarvationTitle"
  | "legend.conditionStarvationBody"
  | "legend.keyThread"
  | "legend.keyWaitEdge"
  | "legend.keyOwner"
  | "legend.keyWaiter"
  | "legend.keyLock"
  | "legend.keyHealthy"
  | "legend.keyClass"
  | "legend.keyPoolWorker"
  | "legend.keyIoThread"
  | "legend.keyPoolBorrower"
  | "legend.keySpinThread"
  | "legend.keyConditionWaiter"
  | "legend.keyCondition"
  | "deadlocks.title"
  | "deadlocks.waitsOn"
  | "deadlocks.heldBy"
  | "deadlocks.unknown"
  | "contention.title"
  | "contention.empty"
  | "contention.heldBy"
  | "contention.unknownOwner"
  | "states.title"
  | "states.all"
  | "clusters.title"
  | "clusters.blurb"
  | "threads.title"
  | "threads.hideNoise"
  | "threads.state"
  | "threads.empty"
  | "threads.colName"
  | "threads.colId"
  | "threads.colState"
  | "threads.colWaitingOn"
  | "threads.colStack"
  | "threads.colHeldLocks"
  | "threads.moreFrames"
  | "report.title"
  | "report.source"
  | "report.deadlocks"
  | "report.contention"
  | "report.lock"
  | "report.heldBy"
  | "report.waiters"
  | "report.none";

export type Messages = Record<MessageKey, string>;

export type TranslateFn = (
  key: MessageKey,
  params?: Record<string, string | number>,
) => string;
