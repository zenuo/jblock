import type { Analysis } from "./types";

function escapeHtml(value: string): string {
  return value
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;");
}

/** Build a self-contained HTML report string from an analysis result. */
export function buildReportHtml(analysis: Analysis, sourceName: string): string {
  const stateRows = analysis.state_counts
    .map((s) => `<tr><td>${escapeHtml(s.state)}</td><td>${s.count}</td></tr>`)
    .join("");

  const blockedRows = analysis.blocked_edges
    .map(
      (e) =>
        `<tr><td>${escapeHtml(e.blocked_thread)}</td><td>${escapeHtml(
          e.lock,
        )}</td><td>${escapeHtml(e.owner_thread ?? "(unknown)")}</td></tr>`,
    )
    .join("");

  const threadRows = analysis.threads
    .map(
      (t) =>
        `<tr><td>${escapeHtml(t.name)}</td><td>${escapeHtml(
          t.id ?? "",
        )}</td><td>${escapeHtml(t.state)}</td><td>${t.stack_depth}</td><td>${escapeHtml(
          t.held_locks.join(", "),
        )}</td></tr>`,
    )
    .join("");

  return `<!doctype html>
<html lang="en">
<head>
<meta charset="UTF-8" />
<title>jblock report — ${escapeHtml(sourceName)}</title>
<style>
  body { font-family: system-ui, sans-serif; margin: 2rem; color: #1e293b; }
  h1 { font-size: 1.4rem; }
  h2 { font-size: 1.1rem; margin-top: 1.5rem; }
  table { border-collapse: collapse; width: 100%; margin-top: 0.5rem; }
  th, td { border: 1px solid #cbd5e1; padding: 4px 8px; text-align: left; font-size: 0.85rem; }
  th { background: #f1f5f9; }
  .meta { color: #64748b; }
</style>
</head>
<body>
<h1>jblock — Java Thread Dump Report</h1>
<p class="meta">Source: ${escapeHtml(sourceName)} · Format: ${escapeHtml(
    analysis.format,
  )} · Total threads: ${analysis.total_threads}</p>
<h2>Thread states</h2>
<table><thead><tr><th>State</th><th>Count</th></tr></thead><tbody>${stateRows}</tbody></table>
<h2>Lock contention (${analysis.blocked_edges.length})</h2>
<table><thead><tr><th>Blocked thread</th><th>Lock</th><th>Held by</th></tr></thead><tbody>${
    blockedRows || '<tr><td colspan="3">None detected</td></tr>'
  }</tbody></table>
<h2>Threads</h2>
<table><thead><tr><th>Name</th><th>Id</th><th>State</th><th>Stack depth</th><th>Held locks</th></tr></thead><tbody>${threadRows}</tbody></table>
</body>
</html>`;
}

/** Trigger a browser download of the HTML report. */
export function exportHtml(analysis: Analysis, sourceName: string): void {
  const html = buildReportHtml(analysis, sourceName);
  const blob = new Blob([html], { type: "text/html" });
  const url = URL.createObjectURL(blob);
  const a = document.createElement("a");
  a.href = url;
  a.download = `jblock-report-${sourceName || "dump"}.html`;
  document.body.appendChild(a);
  a.click();
  document.body.removeChild(a);
  URL.revokeObjectURL(url);
}

/**
 * Export to PDF by opening the HTML report in a new window and invoking the
 * browser's print dialog (users pick "Save as PDF"). This keeps the harness
 * dependency-free; swap in a dedicated PDF lib later if needed.
 */
export function exportPdf(analysis: Analysis, sourceName: string): void {
  const html = buildReportHtml(analysis, sourceName);
  const win = window.open("", "_blank");
  if (!win) return;
  win.document.write(html);
  win.document.close();
  win.focus();
  win.addEventListener("load", () => {
    win.print();
  });
  // Fallback in case the load event already fired.
  setTimeout(() => win.print(), 300);
}
