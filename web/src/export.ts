import { aggregateContention, buildFindings } from "./analysisUi";
import { htmlLangFor, type Locale, type TranslateFn } from "./i18n";
import appCss from "./index.css?inline";
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
): string {
  const maxState = Math.max(1, ...analysis.state_counts.map((s) => s.count));
  const findings = buildFindings(analysis, t);
  const groups = aggregateContention(analysis.blocked_edges);
  const htmlLang = htmlLangFor(locale);

  const findingsHtml = `
    <section class="panel findings">
      <div class="findings-header">
        <h2>${escapeHtml(t("findings.title"))}</h2>
        <span class="meta mono">${escapeHtml(
          t("findings.meta", {
            count: analysis.total_threads,
            format: analysis.format,
          }),
        )}</span>
      </div>
      <ul class="findings-list">
        ${findings
          .map(
            (f) =>
              `<li class="finding finding-${f.severity}"><strong>${escapeHtml(
                f.title,
              )}</strong><span class="mono">${escapeHtml(f.detail)}</span></li>`,
          )
          .join("")}
      </ul>
    </section>`;

  const deadlockPanel =
    analysis.deadlocks.length === 0
      ? ""
      : `
    <section class="panel">
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
      return `<tr><td class="cell-break">${escapeHtml(th.name)}</td><td>${escapeHtml(
        th.id ?? "",
      )}</td><td><span class="state-pill" style="background:${
        STATE_COLORS[th.state] ?? "#64748b"
      }">${escapeHtml(th.state)}</span></td><td class="mono cell-break">${escapeHtml(
        th.waiting_on ?? "",
      )}</td><td>${th.stack_depth}</td><td class="held-locks-cell">${locks}</td></tr>`;
    })
    .join("");

  return `<!doctype html>
<html lang="${htmlLang}">
<head>
<meta charset="UTF-8" />
<meta name="viewport" content="width=device-width, initial-scale=1.0" />
<title>jblock — ${escapeHtml(t("report.title"))} — ${escapeHtml(sourceName)}</title>
<style>${appCss}</style>
</head>
<body>
<div class="app">
  <header class="app-header">
    <h1><span class="logo">jblock</span> ${escapeHtml(t("report.title"))}</h1>
    <p class="tagline">${escapeHtml(t("report.source", { name: sourceName }))}</p>
  </header>
  ${findingsHtml}
  ${deadlockPanel}
  <section class="panel">
    <h2>${escapeHtml(t("states.title"))}</h2>
    <ul class="states">${states}</ul>
  </section>
  <section class="panel">
    <h2>${escapeHtml(t("report.contention"))}</h2>
    <div class="table-scroll">
    <table><thead><tr><th>${escapeHtml(t("report.lock"))}</th><th>${escapeHtml(
      t("report.heldBy"),
    )}</th><th>${escapeHtml(t("report.waiters"))}</th></tr></thead><tbody>${contentionRows}</tbody></table>
    </div>
  </section>
  <section class="panel">
    <h2>${escapeHtml(t("threads.title", { shown: String(analysis.threads.length) }))}</h2>
    <div class="table-scroll">
    <table class="threads-table"><thead><tr><th>${escapeHtml(t("threads.colName"))}</th><th>${escapeHtml(
      t("threads.colId"),
    )}</th><th>${escapeHtml(t("threads.colState"))}</th><th>${escapeHtml(
      t("threads.colWaitingOn"),
    )}</th><th>${escapeHtml(t("threads.colStack"))}</th><th>${escapeHtml(
      t("threads.colHeldLocks"),
    )}</th></tr></thead><tbody>${threadRows}</tbody></table>
    </div>
  </section>
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
): void {
  const html = buildReportHtml(analysis, sourceName, t, locale);
  triggerDownload(
    new Blob([html], { type: "text/html" }),
    `jblock-report-${sourceName || "dump"}.html`,
  );
}
