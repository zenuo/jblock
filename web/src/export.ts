import { aggregateContention, buildFindings, threadHasWaitingOn } from "./analysisUi";
import { buildFlameTree, renderFlameSvg } from "./flamegraph";
import { htmlLangFor, type Locale, type TranslateFn } from "./i18n";
import appCss from "./index.css?inline";
import { NAV_LABEL_KEYS, reportSections, sectionDomId } from "./reportNav";
import type { Analysis } from "./types";

const STATE_COLORS: Record<string, string> = {
  RUNNABLE: "#22c55e",
  BLOCKED: "#ef4444",
  WAITING: "#f59e0b",
  TIMED_WAITING: "#eab308",
  NEW: "#38bdf8",
  TERMINATED: "#94a3b8",
};

function escapeHtml(value: string): string {
  return value
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;");
}

/**
 * Build a self-contained HTML report that reuses the web app's own stylesheet
 * (imported with `?inline`) and result markup, so the report looks identical to
 * the on-screen analysis (feat-005).
 */
export function buildReportHtml(
  analysis: Analysis,
  sourceName: string,
  t: TranslateFn,
  locale: Locale,
  contentSha256 = "",
): string {
  const maxState = Math.max(1, ...analysis.state_counts.map((s) => s.count));
  const findings = buildFindings(analysis, t);
  const groups = aggregateContention(analysis.blocked_edges);
  const htmlLang = htmlLangFor(locale);

  const findingsHtml = `
    <section class="panel findings" id="${sectionDomId("findings")}">
      <div class="findings-header">
        <h2>${escapeHtml(t("findings.title"))}</h2>
        <div class="findings-header-meta">
          <span class="meta mono">${escapeHtml(
            t("findings.meta", {
              count: analysis.total_threads,
              format: analysis.format,
            }),
          )}</span>
        </div>
      </div>
      ${
        findings.length === 0
          ? `<div class="finding finding-ok"><div class="finding-row"><strong><span class="finding-ok-mark" aria-hidden="true">✅</span>${escapeHtml(
              t("findings.okTitle"),
            )}</strong></div><span class="mono">${escapeHtml(
              t("findings.okDetail", {
                count: analysis.total_threads,
                format: analysis.format,
              }),
            )}</span></div>`
          : `<ul class="findings-list">
        ${findings
          .map(
            (f) =>
              `<li class="finding finding-${f.severity}"><strong>${escapeHtml(
                f.title,
              )}</strong><span class="mono">${escapeHtml(f.detail)}</span></li>`,
          )
          .join("")}
      </ul>`
      }
    </section>`;

  const deadlockPanel =
    analysis.deadlocks.length === 0
      ? ""
      : `
    <section class="panel" id="${sectionDomId("deadlocks")}">
      <h2>${escapeHtml(t("report.deadlocks", { count: analysis.deadlocks.length }))}</h2>
      ${analysis.deadlocks
        .map(
          (d) =>
            `<p class="mono">${d.threads
              .map((name) => escapeHtml(name))
              .join(" &rarr; ")} &rarr; ${escapeHtml(d.threads[0] ?? "")}</p>`,
        )
        .join("")}
    </section>`;

  const states = analysis.state_counts
    .map(
      (s) => `
      <li>
        <div class="state-row">
          <span class="state-name">${escapeHtml(s.state)}</span>
          <span class="bar-track"><span class="bar-fill" style="width:${
            (s.count / maxState) * 100
          }%;background:${STATE_COLORS[s.state] ?? "#64748b"}"></span></span>
          <span class="state-count">${s.count}</span>
        </div>
      </li>`,
    )
    .join("");

  const contentionRows =
    groups.length === 0
      ? `<tr><td colspan="3">${escapeHtml(t("report.none"))}</td></tr>`
      : groups
          .map(
            (g) =>
              `<tr><td class="mono">${escapeHtml(g.lock)}</td><td>${escapeHtml(
                g.owner_thread ?? t("contention.unknownOwner"),
              )}</td><td>${g.waiters.length}: ${escapeHtml(
                g.waiters.slice(0, 8).join(", "),
              )}${g.waiters.length > 8 ? ", …" : ""}</td></tr>`,
          )
          .join("");

  const showWaitingOn = analysis.threads.some(threadHasWaitingOn);

  const threadRows = analysis.threads
    .map((th) => {
      const locks =
        th.held_locks.length === 0
          ? ""
          : `<ul class="held-locks-list">${th.held_locks
              .map(
                (lock) =>
                  `<li class="mono cell-break">${escapeHtml(lock)}</li>`,
              )
              .join("")}</ul>`;
      const waitingCell = showWaitingOn
        ? `<td class="mono cell-break col-waiting">${escapeHtml(
            th.waiting_on ?? "",
          )}</td>`
        : "";
      return `<tr><td class="cell-break col-name">${escapeHtml(th.name)}</td><td class="col-id">${escapeHtml(
        th.id ?? "",
      )}</td><td class="col-state"><span class="state-pill" style="background:${
        STATE_COLORS[th.state] ?? "#64748b"
      }">${escapeHtml(th.state)}</span></td>${waitingCell}<td class="col-stack">${th.stack_depth}</td><td class="held-locks-cell col-locks">${locks}</td></tr>`;
    })
    .join("");

  const sourceTitle = contentSha256
    ? ` title="${escapeHtml(t("app.sha256", { hash: contentSha256 }))}"`
    : "";

  const flameTree = buildFlameTree(analysis.threads, t("flame.noStack"));
  const flameSvg = renderFlameSvg(flameTree, 1100, STATE_COLORS);
  const flameHtml = `
    <section class="panel flame-panel" id="${sectionDomId("flamegraph")}" data-testid="flamegraph">
      <h2>${escapeHtml(t("flame.title"))}</h2>
      <p class="empty">${escapeHtml(t("flame.blurb"))}</p>
      <div class="flame-toolbar"><span class="meta mono">${escapeHtml(
        t("flame.samples", { count: analysis.threads.length }),
      )}</span></div>
      ${
        flameSvg
          ? `<div class="flame-wrap">${flameSvg}</div>`
          : `<p class="empty">${escapeHtml(t("flame.empty"))}</p>`
      }
    </section>`;

  const navSections = reportSections({
    hasDeadlocks: analysis.deadlocks.length > 0,
    hasClusters: false,
  });
  const navHtml = `
    <input type="checkbox" id="report-nav-collapsed" class="report-nav-state" hidden />
    <nav class="report-nav" data-testid="report-nav" aria-label="${escapeHtml(t("nav.title"))}">
      <p class="report-nav-heading">${escapeHtml(t("nav.title"))}</p>
      <ul class="report-nav-list">
        ${navSections
          .map(
            (s) =>
              `<li><a class="report-nav-link" href="#${sectionDomId(s.id)}">${escapeHtml(
                t(NAV_LABEL_KEYS[s.id]),
              )}</a></li>`,
          )
          .join("")}
      </ul>
      <label class="report-nav-toggle" data-testid="report-nav-toggle" for="report-nav-collapsed">
        <span class="nav-collapse-label">${escapeHtml(t("nav.collapse"))}</span>
        <span class="nav-expand-label">${escapeHtml(t("nav.expand"))}</span>
      </label>
    </nav>`;

  return `<!doctype html>
<html lang="${htmlLang}">
<head>
<meta charset="UTF-8" />
<meta name="viewport" content="width=device-width, initial-scale=1.0" />
<title>jblock — ${escapeHtml(t("report.title"))} — ${escapeHtml(sourceName)}</title>
<style>${appCss}</style>
</head>
<body>
<div class="app report">
  <header class="app-header">
    <h1><span class="logo">jblock</span> ${escapeHtml(t("report.title"))}</h1>
    <p class="tagline"><span class="report-source-name"${sourceTitle}>${escapeHtml(
      t("report.source", { name: sourceName }),
    )}</span></p>
  </header>
  <div class="results-shell">
  ${navHtml}
  <div class="results">
  ${findingsHtml}
  ${deadlockPanel}
  <section class="panel" id="${sectionDomId("contention")}">
    <h2>${escapeHtml(t("report.contention"))}</h2>
    <div class="table-scroll">
    <table><thead><tr><th>${escapeHtml(t("report.lock"))}</th><th>${escapeHtml(
      t("report.heldBy"),
    )}</th><th>${escapeHtml(t("report.waiters"))}</th></tr></thead><tbody>${contentionRows}</tbody></table>
    </div>
  </section>
  <section class="panel" id="${sectionDomId("states")}">
    <h2>${escapeHtml(t("states.title"))}</h2>
    <ul class="states">${states}</ul>
  </section>
  <section class="panel" id="${sectionDomId("threads")}">
    <h2>${escapeHtml(t("threads.title", { shown: String(analysis.threads.length) }))}</h2>
    <div class="table-scroll">
    <table class="threads-table" data-has-waiting="${showWaitingOn ? "true" : "false"}"><thead><tr><th class="col-name">${escapeHtml(t("threads.colName"))}</th><th class="col-id">${escapeHtml(
      t("threads.colId"),
    )}</th><th class="col-state">${escapeHtml(t("threads.colState"))}</th>${
      showWaitingOn
        ? `<th class="col-waiting">${escapeHtml(t("threads.colWaitingOn"))}</th>`
        : ""
    }<th class="col-stack">${escapeHtml(
      t("threads.colStack"),
    )}</th><th class="col-locks">${escapeHtml(
      t("threads.colHeldLocks"),
    )}</th></tr></thead><tbody>${threadRows}</tbody></table>
    </div>
  </section>
  ${flameHtml}
  </div>
  </div>
</div>
</body>
</html>`;
}

function triggerDownload(blob: Blob, filename: string): void {
  const url = URL.createObjectURL(blob);
  const a = document.createElement("a");
  a.href = url;
  a.download = filename;
  document.body.appendChild(a);
  a.click();
  document.body.removeChild(a);
  URL.revokeObjectURL(url);
}

/** Download the HTML report (localized). */
export function exportHtml(
  analysis: Analysis,
  sourceName: string,
  t: TranslateFn,
  locale: Locale,
  contentSha256 = "",
): void {
  const html = buildReportHtml(analysis, sourceName, t, locale, contentSha256);
  triggerDownload(
    new Blob([html], { type: "text/html" }),
    `jblock-report-${sourceName || "dump"}.html`,
  );
}
