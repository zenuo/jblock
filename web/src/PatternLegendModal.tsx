import { useEffect, useRef } from "react";
import { useI18n } from "./i18n";
import type { Finding } from "./analysisUi";

export type PatternKind = Finding["kind"];

interface Props {
  kind: PatternKind;
  onClose: () => void;
}

export default function PatternLegendModal({ kind, onClose }: Props) {
  const { t } = useI18n();
  const closeRef = useRef<HTMLButtonElement>(null);

  useEffect(() => {
    closeRef.current?.focus();
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") onClose();
    };
    window.addEventListener("keydown", onKey);
    const prev = document.body.style.overflow;
    document.body.style.overflow = "hidden";
    return () => {
      window.removeEventListener("keydown", onKey);
      document.body.style.overflow = prev;
    };
  }, [onClose]);

  const titleKey =
    kind === "deadlock"
      ? "legend.deadlockTitle"
      : kind === "hot-lock"
        ? "legend.hotLockTitle"
        : kind === "blocked"
          ? "legend.blockedTitle"
          : "legend.cleanTitle";
  const bodyKey =
    kind === "deadlock"
      ? "legend.deadlockBody"
      : kind === "hot-lock"
        ? "legend.hotLockBody"
        : kind === "blocked"
          ? "legend.blockedBody"
          : "legend.cleanBody";

  return (
    <div
      className="modal-backdrop"
      data-testid="pattern-legend-modal"
      onClick={(e) => {
        if (e.target === e.currentTarget) onClose();
      }}
    >
      <div
        className="modal legend-modal"
        role="dialog"
        aria-modal="true"
        aria-labelledby="legend-title"
      >
        <div className="modal-header">
          <h2 id="legend-title">{t(titleKey)}</h2>
          <button
            ref={closeRef}
            type="button"
            className="modal-close"
            aria-label={t("legend.close")}
            onClick={onClose}
          >
            ×
          </button>
        </div>
        <p className="legend-body">{t(bodyKey)}</p>
        <div className="legend-stage" data-testid={`legend-demo-${kind}`}>
          {kind === "deadlock" && <DeadlockDemo />}
          {kind === "hot-lock" && <HotLockDemo />}
          {kind === "blocked" && <BlockedDemo />}
          {kind === "clean" && <CleanDemo />}
        </div>
        <ul className="legend-key">
          {kind === "deadlock" && (
            <>
              <li>
                <span className="swatch swatch-thread" /> {t("legend.keyThread")}
              </li>
              <li>
                <span className="swatch swatch-wait" /> {t("legend.keyWaitEdge")}
              </li>
            </>
          )}
          {(kind === "hot-lock" || kind === "blocked") && (
            <>
              <li>
                <span className="swatch swatch-owner" /> {t("legend.keyOwner")}
              </li>
              <li>
                <span className="swatch swatch-waiter" /> {t("legend.keyWaiter")}
              </li>
              <li>
                <span className="swatch swatch-lock" /> {t("legend.keyLock")}
              </li>
            </>
          )}
          {kind === "clean" && (
            <li>
              <span className="swatch swatch-ok" /> {t("legend.keyHealthy")}
            </li>
          )}
        </ul>
      </div>
    </div>
  );
}

function DeadlockDemo() {
  return (
    <svg viewBox="0 0 320 220" className="legend-svg" aria-hidden="true">
      <defs>
        <marker
          id="arrow-wait"
          markerWidth="8"
          markerHeight="8"
          refX="6"
          refY="3"
          orient="auto"
        >
          <path d="M0,0 L6,3 L0,6 Z" fill="#ef4444" />
        </marker>
      </defs>
      <path
        className="legend-flow legend-flow-cycle"
        d="M160 48 L250 170 L70 170 Z"
        fill="none"
        stroke="#ef4444"
        strokeWidth="2.5"
        strokeDasharray="8 6"
        markerMid="url(#arrow-wait)"
      />
      <g transform="translate(160 48)">
        <g className="legend-node-pulse">
          <circle r="22" fill="#fee2e2" stroke="#ef4444" strokeWidth="2" />
          <text textAnchor="middle" dy="5" fontSize="11" fontWeight="700">
            T1
          </text>
        </g>
      </g>
      <g transform="translate(250 170)">
        <g className="legend-node-pulse" style={{ animationDelay: "0.4s" }}>
          <circle r="22" fill="#fee2e2" stroke="#ef4444" strokeWidth="2" />
          <text textAnchor="middle" dy="5" fontSize="11" fontWeight="700">
            T2
          </text>
        </g>
      </g>
      <g transform="translate(70 170)">
        <g className="legend-node-pulse" style={{ animationDelay: "0.8s" }}>
          <circle r="22" fill="#fee2e2" stroke="#ef4444" strokeWidth="2" />
          <text textAnchor="middle" dy="5" fontSize="11" fontWeight="700">
            T3
          </text>
        </g>
      </g>
      <text
        x="160"
        y="118"
        textAnchor="middle"
        fontSize="12"
        fontWeight="700"
        fill="#b91c1c"
        className="legend-caption-anim"
      >
        A → B → C → A
      </text>
    </svg>
  );
}

function HotLockDemo() {
  return (
    <svg viewBox="0 0 320 220" className="legend-svg" aria-hidden="true">
      <defs>
        <marker
          id="arrow-hot"
          markerWidth="7"
          markerHeight="7"
          refX="6"
          refY="3"
          orient="auto"
        >
          <path d="M0,0 L6,3 L0,6 Z" fill="#f59e0b" />
        </marker>
      </defs>
      <g transform="translate(160 108)">
        <rect
          className="legend-lock-pulse"
          x="-20"
          y="-20"
          width="40"
          height="40"
          rx="8"
          fill="#eef2ff"
          stroke="#6366f1"
          strokeWidth="2"
        />
        <text textAnchor="middle" dy="4" fontSize="11" fontWeight="700">
          Lock
        </text>
      </g>
      <g transform="translate(160 42)">
        <circle r="20" fill="#dcfce7" stroke="#22c55e" strokeWidth="2" />
        <text textAnchor="middle" dy="4" fontSize="10" fontWeight="700">
          Owner
        </text>
      </g>
      <line
        x1="160"
        y1="62"
        x2="160"
        y2="88"
        stroke="#22c55e"
        strokeWidth="2"
      />
      {[
        [60, 170],
        [160, 190],
        [260, 170],
      ].map(([x, y], i) => (
        <g key={i}>
          <path
            className="legend-flow"
            style={{ animationDelay: `${i * 0.25}s` }}
            d={`M${x} ${y - 18} L160 128`}
            fill="none"
            stroke="#f59e0b"
            strokeWidth="2"
            strokeDasharray="6 5"
            markerEnd="url(#arrow-hot)"
          />
          <g transform={`translate(${x} ${y})`}>
            <g
              className="legend-node-bounce"
              style={{ animationDelay: `${i * 0.25}s` }}
            >
              <circle r="18" fill="#fff7ed" stroke="#f59e0b" strokeWidth="2" />
              <text textAnchor="middle" dy="4" fontSize="10" fontWeight="700">
                W{i + 1}
              </text>
            </g>
          </g>
        </g>
      ))}
    </svg>
  );
}

function BlockedDemo() {
  return (
    <svg viewBox="0 0 320 220" className="legend-svg" aria-hidden="true">
      <defs>
        <marker
          id="arrow-blk"
          markerWidth="7"
          markerHeight="7"
          refX="6"
          refY="3"
          orient="auto"
        >
          <path d="M0,0 L6,3 L0,6 Z" fill="#ef4444" />
        </marker>
      </defs>
      <g transform="translate(232 112)">
        <rect
          className="legend-lock-pulse"
          x="-22"
          y="-22"
          width="44"
          height="44"
          rx="8"
          fill="#eef2ff"
          stroke="#6366f1"
          strokeWidth="2"
        />
        <text textAnchor="middle" dy="4" fontSize="11" fontWeight="700">
          Lock
        </text>
      </g>
      <g transform="translate(232 48)">
        <circle r="18" fill="#dcfce7" stroke="#22c55e" strokeWidth="2" />
        <text textAnchor="middle" dy="4" fontSize="10" fontWeight="700">
          Own
        </text>
      </g>
      <line
        x1="232"
        y1="66"
        x2="232"
        y2="90"
        stroke="#22c55e"
        strokeWidth="2"
      />
      {[48, 100, 152].map((y, i) => (
        <g key={i}>
          <path
            className="legend-flow"
            style={{ animationDelay: `${i * 0.3}s` }}
            d={`M90 ${y} L210 112`}
            fill="none"
            stroke="#ef4444"
            strokeWidth="2"
            strokeDasharray="5 5"
            markerEnd="url(#arrow-blk)"
          />
          <g transform={`translate(52 ${y})`}>
            <g
              className="legend-node-shake"
              style={{ animationDelay: `${i * 0.3}s` }}
            >
              <rect
                x="-28"
                y="-16"
                width="56"
                height="32"
                rx="8"
                fill="#fee2e2"
                stroke="#ef4444"
                strokeWidth="2"
              />
              <text textAnchor="middle" dy="4" fontSize="10" fontWeight="700">
                BLOCKED
              </text>
            </g>
          </g>
        </g>
      ))}
    </svg>
  );
}

function CleanDemo() {
  return (
    <svg viewBox="0 0 320 220" className="legend-svg" aria-hidden="true">
      <g transform="translate(80 110)">
        <g className="legend-float">
          <circle r="24" fill="#dcfce7" stroke="#22c55e" strokeWidth="2" />
          <text textAnchor="middle" dy="5" fontSize="11" fontWeight="700">
            T1
          </text>
        </g>
      </g>
      <g transform="translate(160 70)">
        <g className="legend-float" style={{ animationDelay: "0.35s" }}>
          <circle r="24" fill="#dcfce7" stroke="#22c55e" strokeWidth="2" />
          <text textAnchor="middle" dy="5" fontSize="11" fontWeight="700">
            T2
          </text>
        </g>
      </g>
      <g transform="translate(240 120)">
        <g className="legend-float" style={{ animationDelay: "0.7s" }}>
          <circle r="24" fill="#dcfce7" stroke="#22c55e" strokeWidth="2" />
          <text textAnchor="middle" dy="5" fontSize="11" fontWeight="700">
            T3
          </text>
        </g>
      </g>
      <g transform="translate(160 160)">
        <g className="legend-check">
          <circle r="18" fill="#22c55e" />
          <path
            d="M-7 0 L-2 6 L8 -8"
            fill="none"
            stroke="white"
            strokeWidth="3"
            strokeLinecap="round"
            strokeLinejoin="round"
          />
        </g>
      </g>
    </svg>
  );
}
