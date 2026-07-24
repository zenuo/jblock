import { useEffect, useRef } from "react";
import {
  shortClassName,
  shortLabel,
  type Finding,
  type FindingActor,
  type FindingActors,
} from "./analysisUi";
import { useI18n } from "./i18n";

interface Props {
  finding: Finding;
  onClose: () => void;
}

function shortLock(lock: string | null, max = 18): string {
  if (!lock) return "Lock";
  return shortLabel(lock, max);
}

export default function PatternLegendModal({ finding, onClose }: Props) {
  const { t } = useI18n();
  const closeRef = useRef<HTMLButtonElement>(null);
  const { kind, actors } = finding;

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
          : kind === "thread-pool-exhaustion"
            ? "legend.poolExhaustionTitle"
            : "legend.cleanTitle";
  const bodyKey =
    kind === "deadlock"
      ? "legend.deadlockBody"
      : kind === "hot-lock"
        ? "legend.hotLockBody"
        : kind === "blocked"
          ? "legend.blockedBody"
          : kind === "thread-pool-exhaustion"
            ? "legend.poolExhaustionBody"
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
          {kind === "deadlock" && <DeadlockDemo actors={actors} />}
          {kind === "hot-lock" && <HotLockDemo actors={actors} />}
          {kind === "blocked" && <BlockedDemo actors={actors} />}
          {kind === "thread-pool-exhaustion" && (
            <PoolExhaustionDemo actors={actors} />
          )}
          {kind === "clean" && <CleanDemo actors={actors} />}
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
              <li>
                <span className="swatch swatch-class" /> {t("legend.keyClass")}
              </li>
            </>
          )}
          {kind === "thread-pool-exhaustion" && (
            <>
              <li>
                <span className="swatch swatch-thread" />{" "}
                {t("legend.keyPoolWorker")}
              </li>
              <li>
                <span className="swatch swatch-class" /> {t("legend.keyClass")}
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
              <li>
                <span className="swatch swatch-class" /> {t("legend.keyClass")}
              </li>
            </>
          )}
          {kind === "clean" && (
            <>
              <li>
                <span className="swatch swatch-ok" /> {t("legend.keyHealthy")}
              </li>
              <li>
                <span className="swatch swatch-class" /> {t("legend.keyClass")}
              </li>
            </>
          )}
        </ul>
      </div>
    </div>
  );
}

function ActorLabel({
  actor,
  fallback,
  threadMax = 12,
  classMax = 14,
}: {
  actor: FindingActor | null;
  fallback: string;
  threadMax?: number;
  classMax?: number;
}) {
  const thread = actor?.thread ?? fallback;
  const cls = shortClassName(actor?.className ?? null, classMax);
  const tip = [thread, actor?.className].filter(Boolean).join(" · ");
  return (
    <>
      <title>{tip}</title>
      <text textAnchor="middle" dy="-2" fontSize="10" fontWeight="700">
        {shortLabel(thread, threadMax)}
      </text>
      {cls ? (
        <text
          textAnchor="middle"
          dy="11"
          fontSize="8"
          fontWeight="600"
          fill="#64748b"
          className="legend-class-label"
        >
          {cls}
        </text>
      ) : null}
    </>
  );
}

function DeadlockDemo({ actors }: { actors: FindingActors }) {
  const nodes =
    actors.nodes.length > 0
      ? actors.nodes.slice(0, 6)
      : [
          { thread: "T1", className: null },
          { thread: "T2", className: null },
          { thread: "T3", className: null },
        ];
  const n = Math.max(nodes.length, 2);
  const cx = 160;
  const cy = 105;
  const r = 68;
  const points = nodes.map((_, i) => {
    const angle = -Math.PI / 2 + (i * 2 * Math.PI) / n;
    return { x: cx + r * Math.cos(angle), y: cy + r * Math.sin(angle) };
  });
  const pathD =
    points.map((p, i) => `${i === 0 ? "M" : "L"}${p.x} ${p.y}`).join(" ") +
    " Z";
  const caption = `${nodes.map((a) => shortLabel(a.thread, 10)).join(" → ")} → ${shortLabel(nodes[0]?.thread ?? "", 10)}`;

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
        d={pathD}
        fill="none"
        stroke="#ef4444"
        strokeWidth="2.5"
        strokeDasharray="8 6"
        markerMid="url(#arrow-wait)"
      />
      {points.map((p, i) => (
        <g key={i} transform={`translate(${p.x} ${p.y})`}>
          <g
            className="legend-node-pulse"
            style={{ animationDelay: `${i * 0.35}s` }}
          >
            <circle r="26" fill="#fee2e2" stroke="#ef4444" strokeWidth="2" />
            <ActorLabel actor={nodes[i]!} fallback={`T${i + 1}`} />
          </g>
        </g>
      ))}
      <text
        x="160"
        y="210"
        textAnchor="middle"
        fontSize="10"
        fontWeight="700"
        fill="#b91c1c"
        className="legend-caption-anim"
      >
        <title>{nodes.map((a) => a.thread).join(" → ")}</title>
        {caption}
      </text>
    </svg>
  );
}

function HotLockDemo({ actors }: { actors: FindingActors }) {
  const waiters =
    actors.waiters.length > 0
      ? actors.waiters.slice(0, 3)
      : [
          { thread: "W1", className: null },
          { thread: "W2", className: null },
          { thread: "W3", className: null },
        ];
  const positions = [
    [60, 175],
    [160, 195],
    [260, 175],
  ] as const;

  return (
    <svg viewBox="0 0 320 230" className="legend-svg" aria-hidden="true">
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
      <g transform="translate(160 115)">
        <rect
          className="legend-lock-pulse"
          x="-36"
          y="-22"
          width="72"
          height="44"
          rx="8"
          fill="#eef2ff"
          stroke="#6366f1"
          strokeWidth="2"
        />
        <title>{actors.lock ?? "Lock"}</title>
        <text textAnchor="middle" dy="4" fontSize="10" fontWeight="700">
          {shortLock(actors.lock)}
        </text>
      </g>
      <g transform="translate(160 38)">
        <circle r="28" fill="#dcfce7" stroke="#22c55e" strokeWidth="2" />
        <ActorLabel
          actor={actors.owner}
          fallback="Owner"
          threadMax={11}
          classMax={13}
        />
      </g>
      <line
        x1="160"
        y1="66"
        x2="160"
        y2="93"
        stroke="#22c55e"
        strokeWidth="2"
      />
      {waiters.map((w, i) => {
        const [x, y] = positions[i] ?? positions[positions.length - 1]!;
        return (
          <g key={`${w.thread}-${i}`}>
            <path
              className="legend-flow"
              style={{ animationDelay: `${i * 0.25}s` }}
              d={`M${x} ${y - 24} L160 137`}
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
                <circle r="26" fill="#fff7ed" stroke="#f59e0b" strokeWidth="2" />
                <ActorLabel actor={w} fallback={`W${i + 1}`} />
              </g>
            </g>
          </g>
        );
      })}
    </svg>
  );
}

function BlockedDemo({ actors }: { actors: FindingActors }) {
  const blocked =
    actors.nodes.length > 0
      ? actors.nodes.slice(0, 3)
      : [
          { thread: "blocked-1", className: null },
          { thread: "blocked-2", className: null },
          { thread: "blocked-3", className: null },
        ];
  const ys = [48, 105, 162];

  return (
    <svg viewBox="0 0 340 220" className="legend-svg" aria-hidden="true">
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
      <g transform="translate(250 118)">
        <rect
          className="legend-lock-pulse"
          x="-40"
          y="-24"
          width="80"
          height="48"
          rx="8"
          fill="#eef2ff"
          stroke="#6366f1"
          strokeWidth="2"
        />
        <title>{actors.lock ?? "Lock"}</title>
        <text textAnchor="middle" dy="4" fontSize="10" fontWeight="700">
          {shortLock(actors.lock)}
        </text>
      </g>
      <g transform="translate(250 42)">
        <circle r="26" fill="#dcfce7" stroke="#22c55e" strokeWidth="2" />
        <ActorLabel actor={actors.owner} fallback="Owner" threadMax={11} />
      </g>
      <line
        x1="250"
        y1="68"
        x2="250"
        y2="94"
        stroke="#22c55e"
        strokeWidth="2"
      />
      {blocked.map((b, i) => {
        const y = ys[i] ?? 105;
        return (
          <g key={`${b.thread}-${i}`}>
            <path
              className="legend-flow"
              style={{ animationDelay: `${i * 0.3}s` }}
              d={`M118 ${y} L210 118`}
              fill="none"
              stroke="#ef4444"
              strokeWidth="2"
              strokeDasharray="5 5"
              markerEnd="url(#arrow-blk)"
            />
            <g transform={`translate(70 ${y})`}>
              <g
                className="legend-node-shake"
                style={{ animationDelay: `${i * 0.3}s` }}
              >
                <rect
                  x="-58"
                  y="-22"
                  width="116"
                  height="44"
                  rx="8"
                  fill="#fee2e2"
                  stroke="#ef4444"
                  strokeWidth="2"
                />
                <ActorLabel actor={b} fallback={`B${i + 1}`} threadMax={14} />
              </g>
            </g>
          </g>
        );
      })}
    </svg>
  );
}

function PoolExhaustionDemo({ actors }: { actors: FindingActors }) {
  const nodes =
    actors.nodes.length > 0
      ? actors.nodes.slice(0, 4)
      : [
          { thread: "pool-1-thread-1", className: null },
          { thread: "pool-1-thread-2", className: null },
          { thread: "pool-1-thread-3", className: null },
          { thread: "pool-1-thread-4", className: null },
        ];
  const positions = [
    [70, 90],
    [160, 55],
    [250, 90],
    [160, 140],
  ] as const;

  return (
    <svg viewBox="0 0 320 220" className="legend-svg" aria-hidden="true">
      <rect
        x="40"
        y="30"
        width="240"
        height="150"
        rx="12"
        fill="#eef2ff"
        stroke="#6366f1"
        strokeWidth="1.5"
        strokeDasharray="4 3"
      />
      <text
        x="160"
        y="48"
        textAnchor="middle"
        fontSize="10"
        fontWeight="700"
        fill="#4f46e5"
      >
        FixedThreadPool
      </text>
      {nodes.map((a, i) => {
        const [x, y] = positions[i] ?? [160, 110];
        const blocked = i > 0;
        return (
          <g key={`${a.thread}-${i}`} transform={`translate(${x} ${y})`}>
            <g
              className={blocked ? "legend-pulse" : "legend-float"}
              style={{ animationDelay: `${i * 0.2}s` }}
            >
              <rect
                x="-48"
                y="-22"
                width="96"
                height="44"
                rx="8"
                fill={blocked ? "#fee2e2" : "#ffedd5"}
                stroke={blocked ? "#ef4444" : "#f59e0b"}
                strokeWidth="2"
              />
              <ActorLabel actor={a} fallback={`W${i + 1}`} threadMax={11} />
            </g>
          </g>
        );
      })}
    </svg>
  );
}

function CleanDemo({ actors }: { actors: FindingActors }) {
  const nodes =
    actors.nodes.length > 0
      ? actors.nodes.slice(0, 3)
      : [
          { thread: "T1", className: null },
          { thread: "T2", className: null },
          { thread: "T3", className: null },
        ];
  const positions = [
    [80, 100],
    [160, 65],
    [240, 110],
  ] as const;

  return (
    <svg viewBox="0 0 320 220" className="legend-svg" aria-hidden="true">
      {nodes.map((a, i) => {
        const [x, y] = positions[i] ?? [160, 100];
        return (
          <g key={`${a.thread}-${i}`} transform={`translate(${x} ${y})`}>
            <g
              className="legend-float"
              style={{ animationDelay: `${i * 0.35}s` }}
            >
              <circle r="30" fill="#dcfce7" stroke="#22c55e" strokeWidth="2" />
              <ActorLabel actor={a} fallback={`T${i + 1}`} />
            </g>
          </g>
        );
      })}
      <g transform="translate(160 170)">
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
