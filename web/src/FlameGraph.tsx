import { useEffect, useMemo, useRef, useState, type MouseEvent } from "react";
import {
  buildFlameTree,
  defaultStateColors,
  flameFill,
  labelFill,
  layoutFlame,
  nodeAtPath,
  readFlameGroupBy,
  storeFlameGroupBy,
  shortFrameLabel,
  type FlameGroupBy,
  type FlameRect,
} from "./flamegraph";
import { useI18n } from "./i18n";
import { lockBodyScroll } from "./scrollLock";
import type { ThreadInfo } from "./types";

const STATE_COLORS = defaultStateColors();

interface Props {
  threads: ThreadInfo[];
}

export default function FlameGraph({ threads }: Props) {
  const { t } = useI18n();
  const wrapRef = useRef<HTMLDivElement>(null);
  const shellRef = useRef<HTMLDivElement>(null);
  const panelRef = useRef<HTMLDivElement>(null);
  const [width, setWidth] = useState(800);
  const [fullscreen, setFullscreen] = useState(false);
  const [shellMinHeight, setShellMinHeight] = useState<number | null>(null);
  const [groupBy, setGroupBy] = useState<FlameGroupBy>(() => readFlameGroupBy());
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
    const raf = requestAnimationFrame(() => apply(el.clientWidth));
    const ro = new ResizeObserver((entries) => {
      const w = entries[0]?.contentRect.width ?? el.clientWidth;
      apply(w);
    });
    ro.observe(el);
    return () => {
      cancelAnimationFrame(raf);
      ro.disconnect();
    };
  }, [fullscreen]);

  useEffect(() => {
    if (!fullscreen) return;
    const onKey = (e: KeyboardEvent) => {
      if (e.key !== "Escape") return;
      e.preventDefault();
      setFullscreen(false);
      setShellMinHeight(null);
    };
    window.addEventListener("keydown", onKey);
    const unlock = lockBodyScroll();
    return () => {
      window.removeEventListener("keydown", onKey);
      unlock();
    };
  }, [fullscreen]);

  const tree = useMemo(
    () => buildFlameTree(threads, t("flame.noStack"), groupBy),
    [threads, t, groupBy],
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

  const applyGroupBy = (next: FlameGroupBy) => {
    setGroupBy(next);
    storeFlameGroupBy(next);
    setFocusPath(null);
  };

  const toggleFullscreen = () => {
    setFullscreen((prev) => {
      if (!prev) {
        const h = shellRef.current?.offsetHeight;
        setShellMinHeight(h && h > 0 ? h : null);
        return true;
      }
      setShellMinHeight(null);
      return false;
    });
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
    <div
      ref={shellRef}
      className={`flame-shell${fullscreen ? " is-fullscreen" : ""}`}
      data-testid="flame-shell"
      data-fullscreen={fullscreen ? "true" : "false"}
      data-group-by={groupBy}
      style={
        fullscreen && shellMinHeight
          ? { minHeight: shellMinHeight }
          : undefined
      }
    >
    <div
      ref={panelRef}
      className="flame-panel-body"
      data-testid="flame-panel-body"
      {...(fullscreen
        ? {
            role: "dialog" as const,
            "aria-modal": true,
            "aria-labelledby": "flame-fs-heading",
          }
        : {})}
    >
      <div className="flame-toolbar">
        <div className="flame-toolbar-lead">
          {fullscreen ? (
            <h2 className="flame-fs-heading" id="flame-fs-heading">
              {t("flame.title")}
            </h2>
          ) : null}
          <span className="meta mono">
            {t("flame.samples", { count: tree.value })}
          </span>
        </div>
        <div className="flame-toolbar-actions">
          {tree.value > 0 ? (
            <div
              className="flame-seg"
              role="group"
              aria-label={t("flame.groupBy")}
              data-testid="flame-group"
            >
              <button
                type="button"
                className={groupBy === "frames" ? "is-active" : undefined}
                data-testid="flame-group-frames"
                aria-pressed={groupBy === "frames"}
                onClick={() => applyGroupBy("frames")}
              >
                {t("flame.groupFrames")}
              </button>
              <button
                type="button"
                className={groupBy === "state" ? "is-active" : undefined}
                data-testid="flame-group-state"
                aria-pressed={groupBy === "state"}
                onClick={() => applyGroupBy("state")}
              >
                {t("flame.groupState")}
              </button>
            </div>
          ) : null}
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
          {tree.value > 0 ? (
            <button
              type="button"
              className="btn flame-reset"
              data-testid="flame-fullscreen"
              aria-pressed={fullscreen}
              onClick={toggleFullscreen}
            >
              {fullscreen ? t("flame.exitFullscreen") : t("flame.fullscreen")}
            </button>
          ) : null}
        </div>
      </div>
      {tree.value === 0 ? (
        <p className="empty">{t("flame.empty")}</p>
      ) : (
        <div className="flame-wrap" ref={wrapRef} data-testid="flame-wrap">
          {/* Native SVG titles would pop a second browser tooltip after a delay. */}
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
    </div>
  );
}
