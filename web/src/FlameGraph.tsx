import { useEffect, useMemo, useRef, useState, type MouseEvent } from "react";
import {
  buildFlameTree,
  defaultStateColors,
  flameFill,
  labelFill,
  layoutFlame,
  nodeAtPath,
  shortFrameLabel,
  type FlameRect,
} from "./flamegraph";
import { useI18n } from "./i18n";
import type { ThreadInfo } from "./types";

const STATE_COLORS = defaultStateColors();

interface Props {
  threads: ThreadInfo[];
}

export default function FlameGraph({ threads }: Props) {
  const { t } = useI18n();
  const wrapRef = useRef<HTMLDivElement>(null);
  const [width, setWidth] = useState(800);
  const [focusPath, setFocusPath] = useState<string[] | null>(null);
  const [tip, setTip] = useState<{
    x: number;
    y: number;
    name: string;
    count: number;
    pct: string;
  } | null>(null);

  useEffect(() => {
    const el = wrapRef.current;
    if (!el) return;
    const apply = (w: number) => {
      const next = Math.max(240, Math.floor(w));
      setWidth((prev) => (Math.abs(prev - next) < 1 ? prev : next));
    };
    apply(el.clientWidth);
    const ro = new ResizeObserver((entries) => {
      const w = entries[0]?.contentRect.width ?? el.clientWidth;
      apply(w);
    });
    ro.observe(el);
    return () => ro.disconnect();
  }, []);

  const tree = useMemo(
    () => buildFlameTree(threads, t("flame.noStack")),
    [threads, t],
  );

  const focused = useMemo(() => {
    if (!focusPath || focusPath.length === 0) return tree;
    return nodeAtPath(tree, focusPath) ?? tree;
  }, [tree, focusPath]);

  const layout = useMemo(
    () => layoutFlame(focused, width),
    [focused, width],
  );

  const zoomed = focused !== tree;

  const onRectClick = (rect: FlameRect) => {
    const isCurrentRoot =
      rect.depth === 0 &&
      rect.path[rect.path.length - 1] === focused.name;
    if (isCurrentRoot && zoomed) {
      const parentPath = focusPath?.slice(0, -1) ?? null;
      setFocusPath(parentPath && parentPath.length > 0 ? parentPath : null);
      return;
    }
    setFocusPath(rect.path);
  };

  const onRectMove = (event: MouseEvent, rect: FlameRect) => {
    const pct =
      layout.total > 0
        ? ((rect.value / layout.total) * 100).toFixed(1)
        : "0";
    setTip({
      x: event.clientX,
      y: event.clientY,
      name: rect.name,
      count: rect.value,
      pct,
    });
  };

  return (
    <div className="flame-panel-body">
      <div className="flame-toolbar">
        <span className="meta mono">
          {t("flame.samples", { count: tree.value })}
        </span>
        {zoomed && (
          <button
            type="button"
            className="btn flame-reset"
            data-testid="flame-reset"
            onClick={() => setFocusPath(null)}
          >
            {t("flame.reset")}
          </button>
        )}
      </div>
      {tree.value === 0 ? (
        <p className="empty">{t("flame.empty")}</p>
      ) : (
        <div className="flame-wrap" ref={wrapRef} data-testid="flame-wrap">
          <svg
            className="flame-svg"
            data-testid="flamegraph-svg"
            viewBox={`0 0 ${width} ${layout.height}`}
            width={width}
            height={layout.height}
            role="img"
            aria-label={t("flame.title")}
            onMouseLeave={() => setTip(null)}
          >
            {layout.rects.map((rect) => {
              const fill = flameFill(rect.name, STATE_COLORS);
              const w = Math.max(0, rect.width - 0.8);
              const h = Math.max(0, rect.height - 0.8);
              const showLabel = rect.width >= 36;
              const maxChars = Math.max(4, Math.floor((rect.width - 8) / 6.2));
              let label = shortFrameLabel(rect.name);
              if (label.length > maxChars) {
                label = `${label.slice(0, maxChars - 1)}…`;
              }
              const key = `${rect.path.join("\0")}:${rect.x}`;
              return (
                <g
                  key={key}
                  onClick={() => onRectClick(rect)}
                  onMouseMove={(e) => onRectMove(e, rect)}
                >
                  <title>
                    {t("flame.tooltip", {
                      name: rect.name,
                      count: rect.value,
                      pct:
                        layout.total > 0
                          ? ((rect.value / layout.total) * 100).toFixed(1)
                          : "0",
                    })}
                  </title>
                  <rect
                    x={rect.x + 0.4}
                    y={rect.y + 0.4}
                    width={w}
                    height={h}
                    rx={1.5}
                    fill={fill}
                  />
                  {showLabel && (
                    <text
                      x={rect.x + 4}
                      y={rect.y + rect.height / 2 + 3.5}
                      fill={labelFill(fill)}
                      fontSize={10}
                    >
                      {label}
                    </text>
                  )}
                </g>
              );
            })}
          </svg>
        </div>
      )}
      {tip && (
        <div
          className="flame-tip"
          style={{ left: tip.x + 12, top: tip.y + 14 }}
          role="status"
        >
          {t("flame.tooltip", {
            name: tip.name,
            count: tip.count,
            pct: tip.pct,
          })}
        </div>
      )}
    </div>
  );
}
