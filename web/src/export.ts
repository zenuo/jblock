import { PDFDocument, StandardFonts, rgb } from "pdf-lib";
import { aggregateContention, buildFindings } from "./analysisUi";
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
export function buildReportHtml(analysis: Analysis, sourceName: string): string {
  const maxState = Math.max(1, ...analysis.state_counts.map((s) => s.count));
  const findings = buildFindings(analysis);
  const groups = aggregateContention(analysis.blocked_edges);

  const findingsHtml = `
    <section class="panel findings">
      <div class="findings-header">
        <h2>Findings</h2>
        <span class="meta mono">${analysis.total_threads} threads · ${escapeHtml(analysis.format)}</span>
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
      <h2>Deadlocks (${analysis.deadlocks.length})</h2>
      ${analysis.deadlocks
        .map(
          (d) =>
            `<p class="mono">${d.threads
              .map((t) => escapeHtml(t))
              .join(" &rarr; ")} &rarr; ${escapeHtml(d.threads[0] ?? "")}</p>`,
        )
        .join("")}
    </section>`;

  const states = analysis.state_counts
    .map(
      (s) => `
      <li>
        <span class="state-name">${escapeHtml(s.state)}</span>
        <span class="bar-track"><span class="bar-fill" style="width:${
          (s.count / maxState) * 100
        }%;background:${STATE_COLORS[s.state] ?? "#64748b"}"></span></span>
        <span class="state-count">${s.count}</span>
      </li>`,
    )
    .join("");

  const contentionRows =
    groups.length === 0
      ? '<tr><td colspan="3">None detected</td></tr>'
      : groups
          .map(
            (g) =>
              `<tr><td class="mono">${escapeHtml(g.lock)}</td><td>${escapeHtml(
                g.owner_thread ?? "(unknown)",
              )}</td><td>${g.waiters.length}: ${escapeHtml(
                g.waiters.slice(0, 8).join(", "),
              )}${g.waiters.length > 8 ? ", …" : ""}</td></tr>`,
          )
          .join("");

  const threadRows = analysis.threads
    .map(
      (t) =>
        `<tr><td>${escapeHtml(t.name)}</td><td>${escapeHtml(
          t.id ?? "",
        )}</td><td><span class="state-pill" style="background:${
          STATE_COLORS[t.state] ?? "#64748b"
        }">${escapeHtml(t.state)}</span></td><td class="mono">${escapeHtml(
          t.waiting_on ?? "",
        )}</td><td>${t.stack_depth}</td><td class="mono">${escapeHtml(
          t.held_locks.join(", "),
        )}</td></tr>`,
    )
    .join("");

  return `<!doctype html>
<html lang="en">
<head>
<meta charset="UTF-8" />
<meta name="viewport" content="width=device-width, initial-scale=1.0" />
<title>jblock report — ${escapeHtml(sourceName)}</title>
<style>${appCss}</style>
</head>
<body>
<div class="app">
  <header class="app-header">
    <h1><span class="logo">jblock</span> Thread Dump Report</h1>
    <p class="tagline">Source: ${escapeHtml(sourceName)}</p>
  </header>
  ${findingsHtml}
  ${deadlockPanel}
  <section class="panel">
    <h2>Thread states</h2>
    <ul class="states">${states}</ul>
  </section>
  <section class="panel">
    <h2>Lock contention (by lock)</h2>
    <table><thead><tr><th>Lock</th><th>Held by</th><th>Waiters</th></tr></thead><tbody>${contentionRows}</tbody></table>
  </section>
  <section class="panel">
    <h2>Threads (${analysis.threads.length})</h2>
    <table><thead><tr><th>Name</th><th>Id</th><th>State</th><th>Waiting on</th><th>Stack</th><th>Held locks</th></tr></thead><tbody>${threadRows}</tbody></table>
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

/** Download the HTML report. */
export function exportHtml(analysis: Analysis, sourceName: string): void {
  const html = buildReportHtml(analysis, sourceName);
  triggerDownload(new Blob([html], { type: "text/html" }), `jblock-report-${sourceName || "dump"}.html`);
}

// pdf-lib's standard fonts are WinAnsi-encoded; drop anything they can't render.
function ansi(text: string): string {
  // eslint-disable-next-line no-control-regex
  return text.replace(/[^\x00-\xFF]/g, "?");
}

function hexToRgb(hex: string) {
  const h = hex.replace("#", "");
  const r = parseInt(h.slice(0, 2), 16) / 255;
  const g = parseInt(h.slice(2, 4), 16) / 255;
  const b = parseInt(h.slice(4, 6), 16) / 255;
  return rgb(r, g, b);
}

/**
 * Render a concise one-page PDF report using pdf-lib (feat-005). Lists are
 * capped so the whole summary fits on a single A4 page.
 */
export async function exportPdf(analysis: Analysis, sourceName: string): Promise<void> {
  const doc = await PDFDocument.create();
  const page = doc.addPage([595.28, 841.89]); // A4 portrait
  const font = await doc.embedFont(StandardFonts.Helvetica);
  const bold = await doc.embedFont(StandardFonts.HelveticaBold);

  const margin = 40;
  let y = page.getHeight() - margin;
  const ink = rgb(0.1, 0.13, 0.16);
  const muted = rgb(0.4, 0.45, 0.5);

  const line = (text: string, size: number, opts: { bold?: boolean; color?: ReturnType<typeof rgb>; gap?: number } = {}) => {
    page.drawText(ansi(text), {
      x: margin,
      y,
      size,
      font: opts.bold ? bold : font,
      color: opts.color ?? ink,
    });
    y -= size + (opts.gap ?? 6);
  };

  line("jblock — Java Thread Dump Report", 18, { bold: true, gap: 10 });
  line(
    `Source: ${sourceName || "dump"}   Format: ${analysis.format}   Threads: ${analysis.total_threads}`,
    9,
    { color: muted, gap: 8 },
  );
  const findings = buildFindings(analysis).slice(0, 4);
  for (const f of findings) {
    line(`• ${f.title}`, 9, { bold: true, gap: 2 });
    line(`  ${f.detail}`, 8, { color: muted, gap: 6 });
  }
  y -= 4;

  // Thread states with proportional bars.
  line("Thread states", 12, { bold: true });
  const maxState = Math.max(1, ...analysis.state_counts.map((s) => s.count));
  const barX = margin + 130;
  const barMax = 220;
  for (const s of analysis.state_counts) {
    page.drawText(ansi(s.state), { x: margin, y, size: 9, font, color: ink });
    page.drawRectangle({ x: barX, y: y - 1, width: barMax, height: 9, color: rgb(0.94, 0.96, 0.98) });
    page.drawRectangle({
      x: barX,
      y: y - 1,
      width: Math.max(2, (s.count / maxState) * barMax),
      height: 9,
      color: hexToRgb(STATE_COLORS[s.state] ?? "#64748b"),
    });
    page.drawText(String(s.count), { x: barX + barMax + 8, y, size: 9, font, color: ink });
    y -= 15;
  }
  y -= 6;

  // Deadlocks.
  line(`Deadlocks (${analysis.deadlocks.length})`, 12, { bold: true });
  if (analysis.deadlocks.length === 0) {
    line("None detected", 9, { color: muted, gap: 10 });
  } else {
    for (const d of analysis.deadlocks.slice(0, 6)) {
      line(`${d.threads.join(" -> ")} -> ${d.threads[0] ?? ""}`, 9, { gap: 4 });
    }
    y -= 6;
  }

  // Lock contention (aggregated, capped).
  const groups = aggregateContention(analysis.blocked_edges);
  line(`Lock contention (${groups.length} lock(s))`, 12, { bold: true });
  const shownGroups = groups.slice(0, 8);
  if (shownGroups.length === 0) {
    line("None detected", 9, { color: muted, gap: 10 });
  } else {
    for (const g of shownGroups) {
      line(
        `${g.lock}  owner=${g.owner_thread ?? "?"}  waiters=${g.waiters.length}`,
        8,
        { gap: 4 },
      );
    }
    if (groups.length > shownGroups.length) {
      line(`(+${groups.length - shownGroups.length} more locks)`, 8, { color: muted });
    }
    y -= 6;
  }

  // Threads (capped to remaining space).
  line(`Threads (${analysis.threads.length})`, 12, { bold: true });
  const maxRows = Math.max(0, Math.floor((y - margin) / 12));
  const rows = analysis.threads.slice(0, maxRows);
  for (const t of rows) {
    line(
      `${t.name}  [${t.state}]  wait=${t.waiting_on ?? "-"}  stack=${t.stack_depth}`,
      8,
      { gap: 4 },
    );
  }
  if (analysis.threads.length > rows.length) {
    line(`(+${analysis.threads.length - rows.length} more)`, 8, { color: muted });
  }

  const bytes = await doc.save();
  const buffer = bytes.buffer.slice(
    bytes.byteOffset,
    bytes.byteOffset + bytes.byteLength,
  ) as ArrayBuffer;
  triggerDownload(
    new Blob([buffer], { type: "application/pdf" }),
    `jblock-report-${sourceName || "dump"}.pdf`,
  );
}
