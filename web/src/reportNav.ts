import type { MessageKey } from "./i18n/types";

export type ReportSectionId =
  | "findings"
  | "deadlocks"
  | "contention"
  | "states"
  | "clusters"
  | "threads"
  | "flamegraph"
  // GC log report (feat-075+)
  | "gc-overview"
  | "gc-findings"
  | "gc-jvm"
  | "gc-pauses"
  | "gc-heap"
  | "gc-distribution"
  | "gc-rates"
  | "gc-metaspace"
  | "gc-events"
  | "gc-diagnostics";

export interface ReportSection {
  id: ReportSectionId;
}

export const NAV_STORAGE_KEY = "jblock.reportNavCollapsed";

export const NAV_LABEL_KEYS: Record<ReportSectionId, MessageKey> = {
  findings: "nav.findings",
  deadlocks: "nav.deadlocks",
  contention: "nav.contention",
  states: "nav.states",
  clusters: "nav.clusters",
  threads: "nav.threads",
  flamegraph: "nav.flamegraph",
  "gc-overview": "gc.nav.overview",
  "gc-findings": "gc.nav.findings",
  "gc-jvm": "gc.nav.jvm",
  "gc-pauses": "gc.nav.pauses",
  "gc-heap": "gc.nav.heap",
  "gc-distribution": "gc.nav.distribution",
  "gc-rates": "gc.nav.rates",
  "gc-metaspace": "gc.nav.metaspace",
  "gc-events": "gc.nav.events",
  "gc-diagnostics": "gc.nav.diagnostics",
};

export function sectionDomId(id: ReportSectionId): string {
  return `section-${id}`;
}

export function parseSectionId(domId: string): ReportSectionId | null {
  if (!domId.startsWith("section-")) return null;
  const id = domId.slice("section-".length);
  if (id in NAV_LABEL_KEYS) return id as ReportSectionId;
  return null;
}

export function reportSections(opts: {
  hasDeadlocks: boolean;
  hasClusters: boolean;
}): ReportSection[] {
  const items: ReportSection[] = [{ id: "findings" }];
  if (opts.hasDeadlocks) items.push({ id: "deadlocks" });
  items.push({ id: "contention" }, { id: "states" });
  if (opts.hasClusters) items.push({ id: "clusters" });
  items.push({ id: "threads" }, { id: "flamegraph" });
  return items;
}

/** Sections of the GC report, in page order. */
export function gcReportSections(opts: { hasEvents: boolean }): ReportSection[] {
  const items: ReportSection[] = [{ id: "gc-overview" }, { id: "gc-findings" }];
  if (opts.hasEvents) {
    items.push({ id: "gc-pauses" }, { id: "gc-heap" }, { id: "gc-distribution" }, { id: "gc-rates" }, { id: "gc-metaspace" }, { id: "gc-events" });
  }
  items.push({ id: "gc-jvm" }, { id: "gc-diagnostics" });
  return items;
}

export function readNavCollapsed(): boolean {
  try {
    return localStorage.getItem(NAV_STORAGE_KEY) === "1";
  } catch {
    return false;
  }
}

export function storeNavCollapsed(collapsed: boolean): void {
  try {
    localStorage.setItem(NAV_STORAGE_KEY, collapsed ? "1" : "0");
  } catch {
    /* ignore quota / private mode */
  }
}
