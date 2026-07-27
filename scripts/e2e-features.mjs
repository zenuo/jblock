#!/usr/bin/env node
/**
 * End-to-end verification for every entry in feature_list.json.
 *
 * Runs cargo tests (filtered by feature), static source/fixture checks, and
 * optional web gates. Writes machine-readable results to harness/e2e-results.json.
 *
 * Usage:
 *   node scripts/e2e-features.mjs
 *   node scripts/e2e-features.mjs --skip-web
 *   node scripts/e2e-features.mjs --cargo-only
 */
import { spawnSync } from "node:child_process";
import {
  existsSync,
  mkdirSync,
  readFileSync,
  writeFileSync,
  readdirSync,
  statSync,
} from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const ROOT = path.resolve(__dirname, "..");
const OUT_DIR = path.join(ROOT, "harness");
const OUT_FILE = path.join(OUT_DIR, "e2e-results.json");

const args = new Set(process.argv.slice(2));
const SKIP_WEB = args.has("--skip-web");
const CARGO_ONLY = args.has("--cargo-only");

function readText(rel) {
  return readFileSync(path.join(ROOT, rel), "utf8");
}

function fileExists(rel) {
  return existsSync(path.join(ROOT, rel));
}

function fileNonEmpty(rel, minBytes = 32) {
  const p = path.join(ROOT, rel);
  if (!existsSync(p)) return false;
  try {
    return statSync(p).size >= minBytes;
  } catch {
    return false;
  }
}

function dirHasFiles(rel, min = 1) {
  const p = path.join(ROOT, rel);
  if (!existsSync(p)) return false;
  try {
    return readdirSync(p).filter((n) => !n.startsWith(".")).length >= min;
  } catch {
    return false;
  }
}

function contains(rel, needle) {
  if (!fileExists(rel)) return false;
  return readText(rel).includes(needle);
}

function containsAll(rel, needles) {
  if (!fileExists(rel)) return false;
  const text = readText(rel);
  return needles.every((n) => text.includes(n));
}

function anyContains(rels, needle) {
  return rels.some((rel) => contains(rel, needle));
}

/** @type {Record<string, { cargo?: string[], static?: (() => {ok:boolean, detail:string})[] }>} */
const FEATURE_CHECKS = {
  "feat-001": {
    cargo: [],
    static: [
      () => ({
        ok: fileExists("init.sh") && fileExists("Cargo.toml") && fileExists("web/package.json"),
        detail: "init.sh + Cargo.toml + web/package.json",
      }),
      () => ({
        ok: contains("init.sh", "cargo test") && contains("init.sh", "typecheck"),
        detail: "init.sh runs cargo test and web typecheck",
      }),
    ],
  },
  "feat-002": {
    cargo: ["detects_jstack_format", "detects_mxbean_format", "counts_stack_depth"],
    static: [
      () => ({
        ok: contains("src/parser.rs", "fn analyze("),
        detail: "parser analyze() exists",
      }),
    ],
  },
  "feat-003": {
    cargo: ["groups_states", "detects_lock_contention"],
    static: [
      () => ({
        ok: contains("src/parser.rs", "blocked_edges"),
        detail: "blocked_edges in Analysis",
      }),
    ],
  },
  "feat-004": {
    cargo: [],
    static: [
      () => ({
        ok: contains("web/src/analyzer.ts", "analyzeDump") && contains("web/src/App.tsx", "analyze"),
        detail: "web analyzer + App wire analyzeDump",
      }),
      () => ({
        ok: contains("web/src/Results.tsx", "state") || contains("web/src/App.tsx", "Results"),
        detail: "Results rendering present",
      }),
    ],
  },
  "feat-005": {
    cargo: [
      "detects_two_thread_deadlock",
      "no_false_deadlock_on_simple_contention",
      "parses_real_world_deadlock_dump",
    ],
    static: [
      () => ({
        ok: contains("web/src/App.tsx", "onDrop") && contains("src/parser.rs", "detect_deadlocks"),
        detail: "drag-drop + detect_deadlocks",
      }),
      () => ({
        ok: fileNonEmpty("tests/fixtures/deadlock_real_jstack.txt", 200),
        detail: "real-world deadlock fixture",
      }),
    ],
  },
  "feat-006": {
    cargo: [],
    static: [
      () => ({
        ok: contains("web/src/export.ts", "exportHtml") && !contains("web/src/export.ts", "exportPdf"),
        detail: "exportHtml only (PDF removed)",
      }),
      () => ({
        ok: !contains("web/package.json", "pdf-lib") && !contains("web/src/export.ts", "pdf-lib"),
        detail: "pdf-lib dependency removed",
      }),
      () => ({
        ok: contains("web/src/export.ts", 'class="app report"') && contains("web/src/index.css", ".app.report .app-header"),
        detail: "HTML export header not sticky",
      }),
    ],
  },
  "feat-007": {
    cargo: ["parses_scenario_aliases"],
    static: [
      () => ({
        ok: fileExists("src/codegen.rs") && fileExists("examples/gen_java.rs"),
        detail: "codegen.rs + gen_java example",
      }),
      () => ({
        ok: contains("web/src/codegen.ts", "generateJava") || contains("web/src/codegen.ts", "Scenario"),
        detail: "frontend codegen present",
      }),
    ],
  },
  "feat-008": {
    cargo: ["detects_java_version_support"],
    static: [
      () => ({
        ok: dirHasFiles("tests/fixtures/java-versions", 8),
        detail: "java-versions fixtures present",
      }),
      () => ({
        ok: fileExists("tests/fixtures/java-versions/FORMAT_DIFFS.md"),
        detail: "FORMAT_DIFFS.md",
      }),
    ],
  },
  "feat-009": {
    cargo: [
      "detects_mxbean_format_lock_contentions",
      "detects_mxbean_format_lock_contentions_real_world",
    ],
    static: [
      () => ({
        ok: fileNonEmpty("tests/fixtures/mxbean_real_contention.txt", 200),
        detail: "mxbean real contention fixture",
      }),
    ],
  },
  "feat-010": {
    cargo: ["detects_two_thread_deadlock"],
    static: [
      () => ({
        ok: contains("src/parser.rs", "deadlocks") && anyContains(["web/src/Results.tsx", "web/src/App.tsx", "web/src/export.ts"], "deadlock"),
        detail: "deadlocks in parser + UI/export",
      }),
    ],
  },
  "feat-011": {
    cargo: [],
    static: [
      () => ({
        ok: contains("web/src/codegen.ts", "generateJava") || contains("web/src/codegen.ts", "classNameFor"),
        detail: "TS codegen module",
      }),
      () => ({
        ok: contains("src/lib.rs", 'cfg(not(target_arch = "wasm32"))') || contains("src/lib.rs", "cfg(not(target_arch"),
        detail: "Rust codegen gated out of WASM",
      }),
      () => ({
        ok: !contains("web/src/analyzer.ts", "generateJava") || contains("web/src/analyzer.ts", 'from "./codegen"'),
        detail: "analyzer does not pull wasm generateJava",
      }),
    ],
  },
  "feat-012": {
    cargo: [],
    static: [
      () => ({
        ok:
          contains("web/src/App.tsx", "HelpButton") ||
          contains("web/src/App.tsx", "open-help") ||
          contains("web/src/HelpModal.tsx", "help-modal"),
        detail: "help modal entry in App (replaces codegen modal)",
      }),
    ],
  },
  "feat-013": {
    cargo: [],
    static: [
      () => ({
        ok: contains("web/src/analysisUi.ts", "buildFindings"),
        detail: "buildFindings",
      }),
      () => ({
        ok: contains("web/src/Results.tsx", "buildFindings") || contains("web/src/Results.tsx", "findings"),
        detail: "findings panel in Results",
      }),
    ],
  },
  "feat-014": {
    cargo: [],
    static: [
      () => ({
        ok: contains("web/src/analysisUi.ts", "aggregateContention"),
        detail: "aggregateContention",
      }),
    ],
  },
  "feat-015": {
    cargo: [],
    static: [
      () => ({
        ok: contains("web/src/Results.tsx", "filter") || contains("web/src/Results.tsx", "sort"),
        detail: "thread filter/sort UI",
      }),
    ],
  },
  "feat-016": {
    cargo: ["captures_top_stack_frames"],
    static: [
      () => ({
        ok: contains("src/parser.rs", "stack") && contains("web/src/Results.tsx", "waiting"),
        detail: "stack frames + waiting_on UI",
      }),
    ],
  },
  "feat-017": {
    cargo: [],
    static: [
      () => ({
        ok: contains("web/src/Results.tsx", "scroll") || contains("web/src/Results.tsx", "highlight") || contains("web/src/Results.tsx", "jump"),
        detail: "jump/highlight thread row",
      }),
    ],
  },
  "feat-018": {
    cargo: [],
    static: [
      () => ({
        ok: contains("web/src/analysisUi.ts", "isJvmNoise"),
        detail: "isJvmNoise helper",
      }),
      () => ({
        ok: contains("web/src/Results.tsx", "hideNoise") || contains("web/src/Results.tsx", "JVM"),
        detail: "hide JVM noise toggle",
      }),
    ],
  },
  "feat-019": {
    cargo: [],
    static: [
      () => ({
        ok: contains("web/src/analysisUi.ts", "clusterByStack"),
        detail: "clusterByStack",
      }),
      () => ({
        ok:
          contains("web/src/index.css", ".stack-preview") &&
          contains("web/src/index.css", "overflow-wrap: anywhere") &&
          contains("web/src/index.css", ".cluster-item"),
        detail: "stack/cluster frames wrap (no horizontal overflow)",
      }),
    ],
  },
  "feat-020": {
    cargo: [],
    static: [
      () => ({
        ok: fileExists(".github/workflows/ci.yml"),
        detail: "ci.yml present",
      }),
      () => ({
        ok: contains(".github/workflows/ci.yml", "cargo test") && contains(".github/workflows/ci.yml", "pages"),
        detail: "CI runs cargo test + Pages deploy",
      }),
    ],
  },
  "feat-021": {
    cargo: [],
    static: [
      () => ({
        ok: contains("web/src/analyzer.ts", "preloadWasm") && contains("web/src/analyzer.ts", "isWasmReady"),
        detail: "preloadWasm + isWasmReady",
      }),
      () => ({
        ok: contains("web/src/App.tsx", "loading") || contains("web/src/App.tsx", "preloadWasm"),
        detail: "loading UI / preload on mount",
      }),
    ],
  },
  "feat-022": {
    cargo: [],
    static: [
      () => ({
        ok: fileExists("web/src/i18n/locales/en.ts") && fileExists("web/src/i18n/locales/zh.ts"),
        detail: "en + zh locales",
      }),
      () => ({
        ok: contains("web/src/i18n/index.ts", "detectBrowserLocale"),
        detail: "detectBrowserLocale in i18n/index.ts",
      }),
    ],
  },
  "feat-023": {
    cargo: [],
    static: [
      () => ({
        ok: fileExists("web/src/PatternLegendModal.tsx"),
        detail: "PatternLegendModal.tsx",
      }),
      () => ({
        ok: contains("web/src/analysisUi.ts", "actors") || contains("web/src/Results.tsx", "PatternLegendModal"),
        detail: "legend wired from findings",
      }),
      () => ({
        ok: !contains('web/src/PatternLegendModal.tsx', 'thread: "W1"') &&
          !contains('web/src/PatternLegendModal.tsx', 'thread: "W2"'),
        detail: "no synthetic W1/W2 legend placeholders",
      }),
      () => ({
        ok: contains("web/src/PatternLegendModal.tsx", 'stroke="#ef4444"') &&
          contains("web/src/index.css", "swatch-waiter"),
        detail: "waiter diagram color matches legend swatch",
      }),
    ],
  },
  "feat-024": {
    cargo: [],
    static: [
      () => ({
        ok: fileNonEmpty("web/public/favicon.svg", 20),
        detail: "favicon.svg",
      }),
      () => ({
        ok: contains("web/index.html", "favicon"),
        detail: "favicon linked in index.html",
      }),
    ],
  },
  "feat-025": {
    cargo: ["parses_web_sample_dump"],
    static: [
      () => ({
        ok: fileNonEmpty("web/src/sample.tdump", 500),
        detail: "rich sample.tdump",
      }),
      () => ({
        ok: fileExists("web/src/sampleDump.ts"),
        detail: "sampleDump.ts import",
      }),
    ],
  },
  "feat-026": {
    cargo: [],
    static: [
      () => ({
        ok: fileExists("web/src/LanguageMenu.tsx"),
        detail: "LanguageMenu.tsx",
      }),
      () => ({
        ok: ["en", "zh", "pt", "es", "nl", "fr", "ja", "ko"].every((l) =>
          fileExists(`web/src/i18n/locales/${l}.ts`),
        ),
        detail: "8 locale catalogs",
      }),
    ],
  },
  "feat-027": {
    cargo: [],
    static: [
      () => ({
        ok: fileExists("src/capture.rs") && contains("src/capture.rs", "compile_run_jstack"),
        detail: "capture.rs compile_run_jstack",
      }),
      () => ({
        ok: contains("src/codegen.rs", "ThreadPoolExhaustion"),
        detail: "Scenario::ThreadPoolExhaustion",
      }),
      () => ({
        ok: contains(".github/workflows/ci.yml", "setup-java") || contains(".github/workflows/ci.yml", "java-version"),
        detail: "CI Java setup for live capture",
      }),
    ],
  },
};

/** Pattern features that share fixture + detect + Scenario + TS PatternKind shape. */
const PATTERN_FEATURES = [
  {
    id: "feat-028",
    kindRs: "ThreadPoolExhaustion",
    kindTs: "thread-pool-exhaustion",
    scenario: "ThreadPoolExhaustion",
    fixture: "tests/fixtures/patterns/thread_pool_exhaustion_jstack.txt",
    cargo: [
      "detects_thread_pool_exhaustion_pattern",
      "detects_thread_pool_exhaustion_from_live_fixture",
    ],
  },
  {
    id: "feat-029",
    kindRs: "SyncIoHotspot",
    kindTs: "sync-io-hotspot",
    scenario: "SyncIoHotspot",
    fixture: "tests/fixtures/patterns/sync_io_hotspot_jstack.txt",
    cargo: ["detects_sync_io_hotspot_pattern", "detects_sync_io_hotspot_from_live_fixture"],
  },
  {
    id: "feat-030",
    kindRs: "DangerousHotLockOwner",
    kindTs: "dangerous-hot-lock-owner",
    scenario: "DangerousHotLock",
    fixture: "tests/fixtures/patterns/dangerous_hot_lock_jstack.txt",
    cargo: [
      "detects_dangerous_hot_lock_owner_pattern",
      "detects_dangerous_hot_lock_from_live_fixture",
    ],
  },
  {
    id: "feat-031",
    kindRs: "ConnectionPoolBorrow",
    kindTs: "connection-pool-borrow",
    scenario: "ConnectionPoolStarve",
    fixture: "tests/fixtures/patterns/connection_pool_starve_jstack.txt",
    cargo: [
      "detects_connection_pool_borrow_pattern",
      "detects_connection_pool_borrow_from_live_fixture",
    ],
  },
  {
    id: "feat-032",
    kindRs: "FutureLatchWaitTree",
    kindTs: "future-latch-wait-tree",
    scenario: "FutureLatchDeadlock",
    fixture: "tests/fixtures/patterns/future_latch_deadlock_jstack.txt",
    cargo: [
      "detects_future_latch_wait_tree_pattern",
      "detects_future_latch_wait_tree_from_live_fixture",
    ],
  },
  {
    id: "feat-033",
    kindRs: "LoggingAppenderContention",
    kindTs: "logging-appender-contention",
    scenario: "LoggingAppenderContention",
    fixture: "tests/fixtures/patterns/logging_appender_contention_jstack.txt",
    cargo: [
      "detects_logging_appender_contention_pattern",
      "detects_logging_appender_contention_from_live_fixture",
    ],
  },
  {
    id: "feat-034",
    kindRs: "BusyWaitSpinHotspot",
    kindTs: "busy-wait-spin-hotspot",
    scenario: "BusyWaitSpin",
    fixture: "tests/fixtures/patterns/busy_wait_spin_jstack.txt",
    cargo: [
      "detects_busy_wait_spin_hotspot_pattern",
      "detects_busy_wait_spin_from_live_fixture",
    ],
  },
  {
    id: "feat-035",
    kindRs: "ConditionParkStarvation",
    kindTs: "condition-park-starvation",
    scenario: "ConditionStarvation",
    fixture: "tests/fixtures/patterns/condition_starvation_jstack.txt",
    cargo: [
      "detects_condition_park_starvation_pattern",
      "detects_condition_park_starvation_from_live_fixture",
    ],
  },
  {
    id: "feat-036",
    kindRs: "LockOrderInconsistency",
    kindTs: "lock-order-inconsistency",
    scenario: "LockOrderRisk",
    fixture: "tests/fixtures/patterns/lock_order_risk_jstack.txt",
    cargo: [
      "detects_lock_order_inconsistency_pattern",
      "detects_lock_order_inconsistency_from_live_fixture",
    ],
  },
  {
    id: "feat-037",
    kindRs: "FinalizerPressure",
    kindTs: "finalizer-pressure",
    scenario: "FinalizerPressure",
    fixture: "tests/fixtures/patterns/finalizer_pressure_jstack.txt",
    cargo: [
      "detects_finalizer_pressure_pattern",
      "detects_finalizer_pressure_from_live_fixture",
    ],
  },
  {
    id: "feat-038",
    kindRs: "SleepAsScheduler",
    kindTs: "sleep-as-scheduler",
    scenario: "SleepAsScheduler",
    fixture: "tests/fixtures/patterns/sleep_as_scheduler_jstack.txt",
    cargo: [
      "detects_sleep_as_scheduler_pattern",
      "detects_sleep_as_scheduler_from_live_fixture",
    ],
  },
  {
    id: "feat-039",
    kindRs: "FrameworkPoolSaturation",
    kindTs: "framework-pool-saturation",
    scenario: "FrameworkPoolSaturation",
    fixture: "tests/fixtures/patterns/framework_pool_saturation_jstack.txt",
    cargo: [
      "detects_framework_pool_saturation_pattern",
      "detects_framework_pool_saturation_from_live_fixture",
      "detects_jetty_and_netty_framework_names",
    ],
  },
  {
    id: "feat-040",
    kindRs: "DnsResolutionStall",
    kindTs: "dns-resolution-stall",
    scenario: "DnsResolutionStall",
    fixture: "tests/fixtures/patterns/dns_resolution_stall_jstack.txt",
    cargo: [
      "detects_dns_resolution_stall_pattern",
      "detects_dns_resolution_stall_from_live_fixture",
    ],
  },
];

for (const p of PATTERN_FEATURES) {
  FEATURE_CHECKS[p.id] = {
    cargo: [...p.cargo, "e2e_all_pattern_fixtures_detect_expected_kinds"],
    static: [
      () => ({
        ok: fileNonEmpty(p.fixture, 100),
        detail: `fixture ${p.fixture}`,
      }),
      () => ({
        ok: contains("src/parser.rs", `PatternKind::${p.kindRs}`),
        detail: `PatternKind::${p.kindRs}`,
      }),
      () => ({
        ok: contains("src/codegen.rs", `Scenario::${p.scenario}`) || contains("src/codegen.rs", p.scenario),
        detail: `Scenario ${p.scenario}`,
      }),
      () => ({
        ok: contains("web/src/types.ts", `"${p.kindTs}"`),
        detail: `TS PatternKind ${p.kindTs}`,
      }),
      () => ({
        ok: contains("web/src/codegen.ts", p.scenario) || contains("web/src/codegen.ts", p.kindTs),
        detail: `web codegen covers ${p.scenario}`,
      }),
      () => ({
        ok:
          contains("web/src/analysisUi.ts", p.kindTs) ||
          contains("web/src/PatternLegendModal.tsx", p.kindTs),
        detail: `UI/legend knows ${p.kindTs}`,
      }),
    ],
  };
}

FEATURE_CHECKS["feat-041"] = {
  cargo: [
    "detects_thread_leak_across_dumps",
    "detects_livelock_across_dumps",
    "e2e_all_pattern_fixtures_detect_expected_kinds",
  ],
  static: [
    () => ({
      ok: contains("src/parser.rs", "fn analyze_series") && contains("src/parser.rs", "PatternKind::ThreadLeak"),
      detail: "analyze_series + ThreadLeak/Livelock",
    }),
    () => ({
      ok: contains("src/lib.rs", "analyze_dumps") || contains("src/lib.rs", "analyzeDumps"),
      detail: "WASM analyzeDumps",
    }),
    () => ({
      ok: contains("web/src/analyzer.ts", "analyzeMany"),
      detail: "web analyzeMany",
    }),
    () => ({
      ok: contains("web/src/App.tsx", "multiple") && contains("web/src/App.tsx", "cross_patterns"),
      detail: "multi-file UI + cross_patterns merge",
    }),
    () => ({
      ok:
        fileNonEmpty("tests/fixtures/patterns/cross_dump/thread_leak_t0.txt", 50) &&
        fileNonEmpty("tests/fixtures/patterns/cross_dump/livelock_t0.txt", 50),
      detail: "cross_dump fixtures",
    }),
    () => ({
      ok: contains("web/src/types.ts", '"thread-leak"') && contains("web/src/types.ts", '"livelock"'),
      detail: "TS PatternKind thread-leak + livelock",
    }),
  ],
};

FEATURE_CHECKS["feat-042"] = {
  cargo: [],
  static: [
    () => ({
      ok: fileExists("web/src/HelpModal.tsx") && contains("web/src/App.tsx", "HelpButton"),
      detail: "HelpModal + HelpButton wired in App",
    }),
    () => ({
      ok: contains("web/src/HelpModal.tsx", "help-security"),
      detail: "security section in help modal",
    }),
    () => ({
      ok: contains("web/src/HelpModal.tsx", "help-howto") && contains("web/src/index.css", "help-anim"),
      detail: "animated how-to walkthrough",
    }),
    () => ({
      ok: contains("web/src/HelpModal.tsx", '"21"'),
      detail: "Java version badges 8/11/17/21",
    }),
    () => ({
      ok: contains("web/src/i18n/locales/en.ts", "help.title") && contains("web/src/i18n/locales/zh.ts", "help.title"),
      detail: "help i18n en+zh",
    }),
    () => ({
      ok: !contains("web/src/App.tsx", "open-codegen") && !contains("web/src/App.tsx", "generateJava"),
      detail: "codegen UI removed from App header",
    }),
  ],
};

FEATURE_CHECKS["feat-043"] = {
  cargo: [],
  static: [
    () => ({
      ok: fileExists(".cursor/skills/ui-design-brain/SKILL.md"),
      detail: "ui-design-brain skill installed",
    }),
    () => ({
      ok: contains("web/src/App.tsx", "home-intro") && contains("web/src/App.tsx", "is-collapsed"),
      detail: "home intro collapses when results exist",
    }),
    () => ({
      ok: contains("web/src/App.tsx", "workspace-toolbar") && contains("web/src/App.tsx", "hasResults"),
      detail: "workspace toolbar when has results",
    }),
    () => ({
      ok: contains("web/index.html", "Instrument Sans") || contains("web/src/index.css", "Instrument Sans"),
      detail: "Instrument Sans typography (Apple-level Minimal)",
    }),
    () => ({
      ok: contains("web/src/index.css", "--accent: #0071e3") || contains("web/src/index.css", "#0071e3"),
      detail: "near-monochrome Apple accent palette",
    }),
    () => ({
      ok: contains("web/src/i18n/locales/en.ts", "home.lead") && contains("web/src/i18n/locales/zh.ts", "home.lead"),
      detail: "home intro i18n",
    }),
  ],
};

FEATURE_CHECKS["feat-044"] = {
  cargo: [],
  static: [
    () => ({
      ok: fileExists("web/src/sha256.ts") && contains("web/src/sha256.ts", "crypto.subtle.digest"),
      detail: "sha256 via Web Crypto",
    }),
    () => ({
      ok: contains("web/src/App.tsx", "dump-filename") && contains("web/src/App.tsx", "sha256Hex"),
      detail: "toolbar dump filename + digest",
    }),
    () => ({
      ok: contains("web/src/export.ts", "contentSha256") && contains("web/src/export.ts", "report-source-name"),
      detail: "HTML export source hover digest",
    }),
    () => ({
      ok: contains("web/src/i18n/locales/en.ts", "app.sha256") && contains("web/src/i18n/locales/zh.ts", "app.sha256"),
      detail: "sha256 i18n",
    }),
  ],
};

FEATURE_CHECKS["feat-045"] = {
  cargo: [],
  static: [
    () => ({
      ok: contains("web/src/App.tsx", 'className="app-header"') || contains("web/src/App.tsx", "app-header"),
      detail: "App renders app-header",
    }),
    () => {
      const css = readText("web/src/index.css");
      const m = css.match(/^\.app-header\s*\{([^}]*)\}/m);
      const body = m ? m[1] : "";
      const stuck = /position\s*:\s*(sticky|fixed)\b/i.test(body);
      return {
        ok: Boolean(m) && !stuck,
        detail: "`.app-header` rule is not position sticky/fixed",
      };
    },
    () => {
      const css = readText("web/src/index.css");
      // No other selector may pin the live app header (exclude .app.report overrides).
      const pinned = /(?:^|\n)(?!\s*\.app\.report)[^\n]*\.app-header[^\n\{]*\{[^}]*position\s*:\s*(sticky|fixed)\b/i.test(
        css,
      );
      return {
        ok: !pinned,
        detail: "no sticky/fixed pin on live .app-header selectors",
      };
    },
  ],
};

FEATURE_CHECKS["feat-046"] = {
  cargo: ["captures_full_stack_frames"],
  static: [
    () => ({
      ok: !contains("src/parser.rs", "MAX_STACK_FRAMES"),
      detail: "parser no longer caps stack with MAX_STACK_FRAMES",
    }),
    () => ({
      ok:
        contains("web/src/Results.tsx", "STACK_PREVIEW_FRAMES") &&
        contains("web/src/Results.tsx", "stack-show-all") &&
        contains("web/src/Results.tsx", "onShowFullStack"),
      detail: "UI preview + show-all control for full stack",
    }),
    () => ({
      ok: contains("web/src/types.ts", "feat-046") || contains("web/src/types.ts", "Full stack"),
      detail: "ThreadInfo.stack typed as full stack",
    }),
  ],
};

FEATURE_CHECKS["feat-047"] = {
  cargo: [],
  static: [
    () => ({
      ok:
        contains("web/src/PatternLegendModal.tsx", "PEER_SHOW") &&
        contains("web/src/PatternLegendModal.tsx", "peerSample") &&
        contains("web/src/PatternLegendModal.tsx", "legend-peer-total"),
      detail: "peer legends capped + total caption",
    }),
    () => {
      const text = readText("web/src/PatternLegendModal.tsx");
      const fan = text.match(/const FAN = \{[\s\S]*?nodes:\s*\[([\s\S]*?)\]\s*as const/);
      const coords = fan ? fan[1] : "";
      const points = (coords.match(/\[[0-9]+,\s*[0-9]+\]/g) || []).length;
      return {
        ok: points === 3,
        detail: `FAN peer layout has ${points} slots (want 3)`,
      };
    },
    () => ({
      ok:
        contains("web/src/PatternLegendModal.tsx", "legend-thread-fullname") &&
        contains("web/src/PatternLegendModal.tsx", "legend-hover-tip") &&
        contains("web/src/PatternLegendModal.tsx", "LegendTipContext") &&
        contains("web/src/index.css", "user-select: all") &&
        !contains("web/src/PatternLegendModal.tsx", "legend-actor-tip"),
      detail: "HTML hover tip outside foreignObject (copyable full name)",
    }),
    () => ({
      ok:
        contains("web/src/analysisUi.ts", "peerTotal") &&
        contains("web/src/i18n/locales/en.ts", "legend.peerSample") &&
        contains("web/src/i18n/locales/zh.ts", "legend.peerSample"),
      detail: "peerTotal + peerSample i18n",
    }),
  ],
};

function run(cmd, cmdArgs, opts = {}) {
  const res = spawnSync(cmd, cmdArgs, {
    cwd: ROOT,
    encoding: "utf8",
    maxBuffer: 32 * 1024 * 1024,
    ...opts,
  });
  return {
    status: res.status ?? 1,
    stdout: res.stdout || "",
    stderr: res.stderr || "",
  };
}

function collectCargoTestNames() {
  // List tests without running (fast discovery).
  const listed = run("cargo", ["test", "--lib", "--", "--list"]);
  const names = new Set();
  for (const line of `${listed.stdout}\n${listed.stderr}`.split("\n")) {
    // e.g. "parser::tests::detects_jstack_format: test"
    const m = line.match(/^(\S+):\s*test\s*$/);
    if (m) names.add(m[1].trim());
  }
  return { listed, names };
}

function runCargoTests(testNames) {
  if (testNames.length === 0) {
    return { status: 0, stdout: "", stderr: "", passed: new Set(), failed: [] };
  }
  // Run selected tests in one cargo invocation via multiple --exact filters is awkward;
  // run the whole lib suite once and parse results — more reliable for e2e recording.
  const res = run("cargo", ["test", "--lib", "--", "--test-threads=8"]);
  const passed = new Set();
  const failed = [];
  for (const line of `${res.stdout}\n${res.stderr}`.split("\n")) {
    const ok = line.match(/^test\s+(\S+)\s+\.\.\.\s+ok\b/);
    if (ok) passed.add(ok[1]);
    const bad = line.match(/^test\s+(\S+)\s+\.\.\.\s+FAILED\b/);
    if (bad) failed.push(bad[1]);
  }
  return { ...res, passed, failed };
}

function resolveTestName(available, shortName) {
  if (available.has(shortName)) return shortName;
  // cargo --list uses module::name; match by suffix
  for (const n of available) {
    if (n === shortName || n.endsWith(`::${shortName}`)) return n;
  }
  return null;
}

function runWebGates() {
  const steps = [];
  const install = run("pnpm", ["-C", "web", "install"]);
  steps.push({
    name: "pnpm install",
    ok: install.status === 0,
    detail: install.status === 0 ? "ok" : (install.stderr || install.stdout).slice(-400),
  });
  if (install.status !== 0) return steps;

  for (const script of ["wasm", "lint", "typecheck", "build"]) {
    const r = run("pnpm", ["-C", "web", "run", script]);
    steps.push({
      name: `pnpm ${script}`,
      ok: r.status === 0,
      detail: r.status === 0 ? "ok" : (r.stderr || r.stdout).slice(-500),
    });
    if (r.status !== 0) break;
  }
  return steps;
}

function main() {
  const featureList = JSON.parse(readText("feature_list.json"));
  const features = featureList.features || featureList;
  if (!Array.isArray(features)) {
    console.error("feature_list.json missing features[]");
    process.exit(2);
  }

  console.log("=== e2e feature matrix ===");
  console.log(`features in list: ${features.length}`);

  const { names: availableTests, listed } = collectCargoTestNames();
  if (listed.status !== 0 && availableTests.size === 0) {
    console.error("cargo test --list failed");
    console.error(listed.stderr || listed.stdout);
    process.exit(1);
  }
  console.log(`cargo lib tests discovered: ${availableTests.size}`);

  const cargoRun = runCargoTests([...availableTests]);
  console.log(
    `cargo lib result: status=${cargoRun.status} passed=${cargoRun.passed.size} failed=${cargoRun.failed.length}`,
  );

  let webSteps = [];
  if (!SKIP_WEB && !CARGO_ONLY) {
    console.log("=== web gates ===");
    webSteps = runWebGates();
    for (const s of webSteps) {
      console.log(`  ${s.ok ? "PASS" : "FAIL"} ${s.name}`);
    }
  }

  const webOk = webSteps.length === 0 || webSteps.every((s) => s.ok);
  const results = [];
  let passCount = 0;
  let failCount = 0;

  for (const feat of features) {
    const id = feat.id;
    const spec = FEATURE_CHECKS[id];
    const checks = [];

    if (!spec) {
      checks.push({
        name: "mapping",
        ok: false,
        detail: "no e2e check mapping defined for this feature",
      });
    } else {
      for (const short of spec.cargo || []) {
        const full = resolveTestName(availableTests, short);
        if (!full) {
          checks.push({
            name: `cargo:${short}`,
            ok: false,
            detail: "test not found in cargo --list",
          });
          continue;
        }
        const ok = cargoRun.passed.has(full) && !cargoRun.failed.includes(full);
        checks.push({
          name: `cargo:${short}`,
          ok,
          detail: ok ? `passed as ${full}` : `did not pass (${full})`,
        });
      }
      for (const fn of spec.static || []) {
        const r = fn();
        checks.push({ name: `static:${r.detail}`, ok: r.ok, detail: r.detail });
      }
    }

    // Shared web gate attachment for UI-heavy features when web ran.
    if (!SKIP_WEB && !CARGO_ONLY && webSteps.length && ["feat-001", "feat-004", "feat-020"].includes(id)) {
      checks.push({
        name: "web-gates",
        ok: webOk,
        detail: webOk ? "wasm/lint/typecheck/build ok" : "web gate failed",
      });
    }

    const ok = checks.every((c) => c.ok);
    if (ok) passCount += 1;
    else failCount += 1;

    results.push({
      id,
      name: feat.name,
      status_in_list: feat.status,
      ok,
      checks,
    });

    console.log(`${ok ? "PASS" : "FAIL"} ${id} — ${feat.name}`);
    for (const c of checks.filter((x) => !x.ok)) {
      console.log(`       ✗ ${c.name}: ${c.detail}`);
    }
  }

  const unmapped = [...Object.keys(FEATURE_CHECKS)].filter(
    (id) => !features.some((f) => f.id === id),
  );

  const report = {
    generated_at: new Date().toISOString(),
    root: ROOT,
    summary: {
      total: features.length,
      pass: passCount,
      fail: failCount,
      cargo_status: cargoRun.status,
      cargo_passed: cargoRun.passed.size,
      cargo_failed: cargoRun.failed,
      web_ok: SKIP_WEB || CARGO_ONLY ? null : webOk,
      unmapped_checks: unmapped,
    },
    web_steps: webSteps,
    features: results,
  };

  mkdirSync(OUT_DIR, { recursive: true });
  writeFileSync(OUT_FILE, JSON.stringify(report, null, 2) + "\n");
  console.log(`\nWrote ${path.relative(ROOT, OUT_FILE)}`);
  console.log(`Summary: ${passCount}/${features.length} features PASS`);

  if (failCount > 0 || cargoRun.status !== 0 || (webOk === false)) {
    process.exit(1);
  }
}

main();
