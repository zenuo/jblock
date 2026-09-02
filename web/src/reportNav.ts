import type { MessageKey } from "./i18n/types";

export type ReportSectionId =
  | "findings"
  | "deadlocks"
  | "contention"
  | "states"
  | "clusters"
  | "threads"
  | "flamegraph";

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
