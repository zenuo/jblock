import { useEffect, useRef } from "react";
import uPlot from "uplot";
import "uplot/dist/uPlot.min.css";

/** One tooltip row: a short line key in the series color, the value, then the label. */
export interface TipRow {
  value: string;
  label: string;
  color?: string;
}

interface Props {
  /** Builds options for a given width; must be memoized by the caller. */
  options: (width: number) => uPlot.Options;
  data: uPlot.AlignedData;
  /** Visible x range in chart units (null = full data extent). */
  xRange: [number, number] | null;
  /** Drag-select finished: x range in chart units. */
  onSelect: (x0: number, x1: number) => void;
  /** Double-click: reset the range. */
  onReset: () => void;
  /** Tooltip content for a data index (null hides it). */
  tooltip: (idx: number) => { title: string; rows: TipRow[] } | null;
  height: number;
  testId: string;
  /** X (chart units) of the selected event: drawn as a hairline on the plot. */
  markX?: number | null;
  /** Click (not drag) on the plot: the data index under the cursor. */
  onPick?: (idx: number) => void;
}

export default function GcChart({ options, data, xRange, onSelect, onReset, tooltip, height, testId, markX, onPick }: Props) {
  const hostRef = useRef<HTMLDivElement>(null);
  const tipRef = useRef<HTMLDivElement>(null);
  const plotRef = useRef<uPlot | null>(null);
  // Latest callbacks without recreating the chart.
  const cb = useRef({ onSelect, onReset, tooltip, onPick });
  cb.current = { onSelect, onReset, tooltip, onPick };
  const markRef = useRef(markX ?? null);
  markRef.current = markX ?? null;
  const rangeRef = useRef(xRange);
  rangeRef.current = xRange;

  useEffect(() => {
    const host = hostRef.current;
    const tip = tipRef.current;
    if (!host || !tip) return;
    const base = options(Math.max(host.clientWidth, 200));
    const renderTip = (u: uPlot) => {
      const idx = u.cursor.idx;
      const content = idx == null ? null : cb.current.tooltip(idx);
      if (content == null || u.cursor.left == null || u.cursor.left < 0) {
        tip.hidden = true;
        return;
      }
      tip.replaceChildren();
      const title = document.createElement("div");
      title.className = "gc-tip-title";
      title.textContent = content.title;
      tip.appendChild(title);
      for (const row of content.rows) {
        const line = document.createElement("div");
        line.className = "gc-tip-row";
        const key = document.createElement("span");
        key.className = "gc-tip-key";
        if (row.color) key.style.background = row.color;
        const value = document.createElement("strong");
        value.textContent = row.value;
        const label = document.createElement("span");
        label.className = "gc-tip-label";
        label.textContent = row.label;
        line.append(key, value, label);
        tip.appendChild(line);
      }
      tip.hidden = false;
      const left = u.cursor.left + u.bbox.left / devicePixelRatio;
      const top = (u.cursor.top ?? 0) + u.bbox.top / devicePixelRatio;
      const flip = left > host.clientWidth * 0.6;
      tip.style.transform = `translate(${flip ? `calc(${left - 14}px - 100%)` : `${left + 14}px`}, ${Math.max(top - 10, 0)}px)`;
    };
    const opts: uPlot.Options = {
      ...base,
      height,
      legend: { show: false },
      cursor: {
        ...base.cursor,
        drag: { x: true, y: false, setScale: false },
        bind: {
          dblclick: () => () => {
            cb.current.onReset();
            return null;
          },
        },
      },
      hooks: {
        ...base.hooks,
        setCursor: [...(base.hooks?.setCursor ?? []), renderTip],
        setSelect: [
          ...(base.hooks?.setSelect ?? []),
          (u) => {
            if (u.select.width > 2) {
              const x0 = u.posToVal(u.select.left, "x");
              const x1 = u.posToVal(u.select.left + u.select.width, "x");
              cb.current.onSelect(x0, x1);
            }
            u.setSelect({ left: 0, top: 0, width: 0, height: 0 }, false);
          },
        ],
        draw: [
          ...(base.hooks?.draw ?? []),
          (u) => {
            const x = markRef.current;
            if (x == null) return;
            const px = Math.round(u.valToPos(x, "x", true)) + 0.5;
            const { ctx, bbox } = u;
            if (px < bbox.left || px > bbox.left + bbox.width) return;
            ctx.save();
            ctx.strokeStyle = "#0b0b0b";
            ctx.lineWidth = devicePixelRatio;
            ctx.beginPath();
            ctx.moveTo(px, bbox.top);
            ctx.lineTo(px, bbox.top + bbox.height);
            ctx.stroke();
            ctx.restore();
          },
        ],
        ready: [
          ...(base.hooks?.ready ?? []),
          (u) => {
            const r = rangeRef.current;
            if (r) u.setScale("x", { min: r[0], max: r[1] });
          },
        ],
      },
    };
    const u = new uPlot(opts, data, host);
    plotRef.current = u;
    const ro = new ResizeObserver(() => {
      const w = host.clientWidth;
      if (w > 0 && Math.abs(w - u.width) > 1) u.setSize({ width: w, height });
    });
    ro.observe(host);
    const hide = () => {
      tip.hidden = true;
    };
    host.addEventListener("mouseleave", hide);
    // A click picks the event under the cursor; a drag (range select) does not.
    let downX = 0;
    const down = (e: MouseEvent) => {
      downX = e.clientX;
    };
    const click = (e: MouseEvent) => {
      const idx = u.cursor.idx;
      if (Math.abs(e.clientX - downX) <= 3 && idx != null) cb.current.onPick?.(idx);
    };
    u.over.addEventListener("mousedown", down);
    u.over.addEventListener("click", click);
    return () => {
      ro.disconnect();
      host.removeEventListener("mouseleave", hide);
      u.over.removeEventListener("mousedown", down);
      u.over.removeEventListener("click", click);
      u.destroy();
      plotRef.current = null;
    };
  }, [options, data, height]);

  useEffect(() => {
    const u = plotRef.current;
    if (!u) return;
    const xs = u.data[0];
    if (xRange) {
      u.setScale("x", { min: xRange[0], max: xRange[1] });
    } else if (xs.length > 0) {
      u.setScale("x", { min: xs[0], max: xs[xs.length - 1] });
    }
  }, [xRange]);

  // Redraw (without recreating the chart) when the selected event changes.
  useEffect(() => {
    plotRef.current?.redraw(false);
  }, [markX]);

  return (
    <div className="gc-chart" data-testid={testId}>
      <div ref={hostRef} className="gc-chart-host" />
      <div ref={tipRef} className="gc-tip" role="status" hidden />
    </div>
  );
}
