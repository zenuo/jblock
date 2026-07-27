import { createContext, useContext, useEffect, useRef, useState } from "react";
import {
  shortClassName,
  shortLabel,
  type Finding,
  type FindingActor,
  type FindingActors,
} from "./analysisUi";
import { useI18n } from "./i18n";
import { lockBodyScroll } from "./scrollLock";

interface Props {
  finding: Finding;
  onClose: () => void;
}

type LegendTip = { thread: string; id: string | null } | null;
type TipSetter = (tip: LegendTip) => void;
const LegendTipContext = createContext<TipSetter>(() => {});

function shortLock(lock: string | null, max = 18): string {
  if (!lock) return "Lock";
  return shortLabel(lock, max);
}

/**
 * Peer / fan legend layouts (feat-047): show at most 3 equal peer nodes so a
 * 4th lower card is not misread as a special role. Cycle layouts (deadlock)
 * are excluded — those are sequential wait-for edges, not parallel peers.
 *
 * Peer demos: busy-wait, condition starvation, sync-I/O, pool exhaustion,
 * connection-pool waiters, hot-lock waiters, blocked, clean (+ aliases that
 * reuse those demos).
 */
const PEER_SHOW = 3;

const FAN = {
  viewBox: "0 0 420 240",
  cx: 210,
  /** Three equal peer cards in one row (no closer “special” node). */
  nodes: [
    [70, 70],
    [210, 55],
    [350, 70],
  ] as const,
  hubY: 185,
} as const;

function peerSample(
  actors: FindingActors,
  from: "nodes" | "waiters" = "nodes",
): { shown: FindingActor[]; total: number } {
  const list =
    from === "waiters" && actors.waiters.length > 0
      ? actors.waiters
      : actors.nodes;
  const total = Math.max(actors.peerTotal || 0, list.length);
  return { shown: list.slice(0, PEER_SHOW), total };
}

export default function PatternLegendModal({ finding, onClose }: Props) {
  const { t } = useI18n();
  const closeRef = useRef<HTMLButtonElement>(null);
  const { kind, actors } = finding;
  const [hoverTip, setHoverTip] = useState<LegendTip>(null);

  useEffect(() => {
    closeRef.current?.focus();
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") onClose();
    };
    window.addEventListener("keydown", onKey);
    const unlock = lockBodyScroll();
    return () => {
      window.removeEventListener("keydown", onKey);
      unlock();
    };
  }, [onClose]);

  useEffect(() => {
    setHoverTip(null);
  }, [kind]);

  const titleKey =
    kind === "deadlock"
      ? "legend.deadlockTitle"
      : kind === "hot-lock"
        ? "legend.hotLockTitle"
        : kind === "blocked"
          ? "legend.blockedTitle"
          : kind === "thread-pool-exhaustion"
            ? "legend.poolExhaustionTitle"
            : kind === "sync-io-hotspot"
              ? "legend.syncIoHotspotTitle"
              : kind === "dangerous-hot-lock-owner"
                ? "legend.dangerousHotLockTitle"
                : kind === "connection-pool-borrow"
                  ? "legend.connectionPoolTitle"
                  : kind === "future-latch-wait-tree"
                    ? "legend.futureLatchTitle"
                    : kind === "logging-appender-contention"
                      ? "legend.loggingAppenderTitle"
                      : kind === "busy-wait-spin-hotspot"
                        ? "legend.busyWaitTitle"
                        : kind === "condition-park-starvation"
                          ? "legend.conditionStarvationTitle"
                          : kind === "lock-order-inconsistency"
                            ? "legend.lockOrderTitle"
                            : kind === "finalizer-pressure"
                              ? "legend.finalizerPressureTitle"
                              : kind === "sleep-as-scheduler"
                                ? "legend.sleepAsSchedulerTitle"
                                : kind === "framework-pool-saturation"
                                  ? "legend.frameworkPoolTitle"
                                  : kind === "dns-resolution-stall"
                                    ? "legend.dnsStallTitle"
                                    : kind === "thread-leak"
                                      ? "legend.threadLeakTitle"
                                      : kind === "livelock"
                                        ? "legend.livelockTitle"
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
            : kind === "sync-io-hotspot"
              ? "legend.syncIoHotspotBody"
              : kind === "dangerous-hot-lock-owner"
                ? "legend.dangerousHotLockBody"
                : kind === "connection-pool-borrow"
                  ? "legend.connectionPoolBody"
                  : kind === "future-latch-wait-tree"
                    ? "legend.futureLatchBody"
                    : kind === "logging-appender-contention"
                      ? "legend.loggingAppenderBody"
                      : kind === "busy-wait-spin-hotspot"
                        ? "legend.busyWaitBody"
                        : kind === "condition-park-starvation"
                          ? "legend.conditionStarvationBody"
                          : kind === "lock-order-inconsistency"
                            ? "legend.lockOrderBody"
                            : kind === "finalizer-pressure"
                              ? "legend.finalizerPressureBody"
                              : kind === "sleep-as-scheduler"
                                ? "legend.sleepAsSchedulerBody"
                                : kind === "framework-pool-saturation"
                                  ? "legend.frameworkPoolBody"
                                  : kind === "dns-resolution-stall"
                                    ? "legend.dnsStallBody"
                                    : kind === "thread-leak"
                                      ? "legend.threadLeakBody"
                                      : kind === "livelock"
                                        ? "legend.livelockBody"
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
        <div
          className="legend-stage"
          data-testid={`legend-demo-${kind}`}
        >
          <LegendTipContext.Provider value={setHoverTip}>
          {kind === "deadlock" && <DeadlockDemo actors={actors} />}
          {kind === "hot-lock" && <HotLockDemo actors={actors} />}
          {kind === "blocked" && <BlockedDemo actors={actors} />}
          {kind === "thread-pool-exhaustion" && (
            <PoolExhaustionDemo actors={actors} />
          )}
          {kind === "sync-io-hotspot" && <SyncIoHotspotDemo actors={actors} />}
          {kind === "dangerous-hot-lock-owner" && (
            <HotLockDemo actors={actors} />
          )}
          {kind === "connection-pool-borrow" && (
            <ConnectionPoolDemo actors={actors} />
          )}
          {kind === "future-latch-wait-tree" && (
            <DeadlockDemo actors={actors} />
          )}
          {kind === "logging-appender-contention" && (
            <HotLockDemo actors={actors} />
          )}
          {kind === "busy-wait-spin-hotspot" && (
            <BusyWaitSpinDemo actors={actors} />
          )}
          {kind === "condition-park-starvation" && (
            <ConditionStarvationDemo actors={actors} />
          )}
          {kind === "lock-order-inconsistency" && (
            <DeadlockDemo actors={actors} />
          )}
          {kind === "finalizer-pressure" && <HotLockDemo actors={actors} />}
          {kind === "sleep-as-scheduler" && (
            <ConditionStarvationDemo actors={actors} />
          )}
          {kind === "framework-pool-saturation" && (
            <PoolExhaustionDemo actors={actors} />
          )}
          {kind === "dns-resolution-stall" && (
            <SyncIoHotspotDemo actors={actors} />
          )}
          {kind === "thread-leak" && <PoolExhaustionDemo actors={actors} />}
          {kind === "livelock" && <BusyWaitSpinDemo actors={actors} />}
          {kind === "clean" && <CleanDemo actors={actors} />}
          </LegendTipContext.Provider>
          {/* Sticky after hover/focus so the full name stays selectable for copy. */}
          {hoverTip ? (
            <div className="legend-hover-tip" data-testid="legend-thread-fullname">
              <code className="legend-actor-fullname">{hoverTip.thread}</code>
              {hoverTip.id ? (
                <code
                  className="legend-actor-id"
                  data-testid="legend-thread-id"
                >
                  Id={hoverTip.id}
                </code>
              ) : null}
            </div>
          ) : null}
        </div>
        <ul className="legend-key">
          {(kind === "deadlock" ||
            kind === "future-latch-wait-tree" ||
            kind === "lock-order-inconsistency") && (
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
          {kind === "framework-pool-saturation" && (
            <>
              <li>
                <span className="swatch swatch-thread" />{" "}
                {t("legend.keyFrameworkWorker")}
              </li>
              <li>
                <span className="swatch swatch-class" /> {t("legend.keyClass")}
              </li>
            </>
          )}
          {kind === "sync-io-hotspot" && (
            <>
              <li>
                <span className="swatch swatch-waiter" /> {t("legend.keyIoThread")}
              </li>
              <li>
                <span className="swatch swatch-class" /> {t("legend.keyClass")}
              </li>
            </>
          )}
          {kind === "dns-resolution-stall" && (
            <>
              <li>
                <span className="swatch swatch-waiter" />{" "}
                {t("legend.keyDnsResolver")}
              </li>
              <li>
                <span className="swatch swatch-class" /> {t("legend.keyClass")}
              </li>
            </>
          )}
          {kind === "thread-leak" && (
            <>
              <li>
                <span className="swatch swatch-thread" />{" "}
                {t("legend.keyLeakThread")}
              </li>
              <li>
                <span className="swatch swatch-class" /> {t("legend.keyClass")}
              </li>
            </>
          )}
          {kind === "livelock" && (
            <>
              <li>
                <span className="swatch swatch-waiter" />{" "}
                {t("legend.keyLivelockThread")}
              </li>
              <li>
                <span className="swatch swatch-class" /> {t("legend.keyClass")}
              </li>
            </>
          )}
          {kind === "busy-wait-spin-hotspot" && (
            <>
              <li>
                <span className="swatch swatch-waiter" /> {t("legend.keySpinThread")}
              </li>
              <li>
                <span className="swatch swatch-class" /> {t("legend.keyClass")}
              </li>
            </>
          )}
          {kind === "condition-park-starvation" && (
            <>
              <li>
                <span className="swatch swatch-waiter" />{" "}
                {t("legend.keyConditionWaiter")}
              </li>
              <li>
                <span className="swatch swatch-lock" /> {t("legend.keyCondition")}
              </li>
              <li>
                <span className="swatch swatch-class" /> {t("legend.keyClass")}
              </li>
            </>
          )}
          {kind === "sleep-as-scheduler" && (
            <>
              <li>
                <span className="swatch swatch-waiter" />{" "}
                {t("legend.keySleepScheduler")}
              </li>
              <li>
                <span className="swatch swatch-class" /> {t("legend.keyClass")}
              </li>
            </>
          )}
          {kind === "connection-pool-borrow" && (
            <>
              <li>
                <span className="swatch swatch-waiter" />{" "}
                {t("legend.keyPoolBorrower")}
              </li>
              <li>
                <span className="swatch swatch-class" /> {t("legend.keyClass")}
              </li>
            </>
          )}
          {(kind === "hot-lock" ||
            kind === "blocked" ||
            kind === "dangerous-hot-lock-owner" ||
            kind === "logging-appender-contention" ||
            kind === "finalizer-pressure") && (
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
  width = 96,
  height = 44,
}: {
  actor: FindingActor | null;
  fallback: string;
  threadMax?: number;
  classMax?: number;
  width?: number;
  height?: number;
}) {
  const setTip = useContext(LegendTipContext);
  const thread = actor?.thread ?? fallback;
  const cls = shortClassName(actor?.className ?? null, classMax);
  const tip: LegendTip = { thread, id: actor?.id ?? null };
  return (
    <foreignObject
      x={-width / 2}
      y={-height / 2}
      width={width}
      height={height}
      className="legend-actor-fo"
    >
      <div
        className="legend-actor"
        data-testid="legend-actor"
        data-thread={thread}
        data-thread-id={actor?.id ?? undefined}
        tabIndex={0}
        onMouseEnter={() => setTip(tip)}
        onFocus={() => setTip(tip)}
        onClick={() => setTip(tip)}
      >
        <div className="legend-actor-short">{shortLabel(thread, threadMax)}</div>
        {cls ? <div className="legend-actor-class">{cls}</div> : null}
      </div>
    </foreignObject>
  );
}

function PeerSampleNote({
  shown,
  total,
  x,
  y,
}: {
  shown: number;
  total: number;
  x: number;
  y: number;
}) {
  const { t } = useI18n();
  if (total <= PEER_SHOW || total <= shown) return null;
  return (
    <text
      x={x}
      y={y}
      textAnchor="middle"
      fontSize="10"
      fontWeight="600"
      fill="#64748b"
      data-testid="legend-peer-total"
    >
      {t("legend.peerSample", { shown: Math.min(shown, PEER_SHOW), total })}
    </text>
  );
}

function DeadlockDemo({ actors }: { actors: FindingActors }) {
  const nodes =
    actors.nodes.length > 0
      ? actors.nodes.slice(0, 6)
      : [
          { thread: "T1", id: null, className: null },
          { thread: "T2", id: null, className: null },
          { thread: "T3", id: null, className: null },
        ];
  const n = Math.max(nodes.length, 2);
  const cx = 160;
  const cy = 105;
  const r = 82;
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
            <ActorLabel
              actor={nodes[i]!}
              fallback={`T${i + 1}`}
              width={52}
              height={52}
            />
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
  // Only real dump thread names — never invent W1/W2/W3 placeholders.
  const ownerThread = actors.owner?.thread;
  const fallbackWaiters = actors.nodes.filter((n) => n.thread !== ownerThread);
  const source: FindingActors = {
    ...actors,
    waiters:
      actors.waiters.length > 0 ? actors.waiters : fallbackWaiters,
    peerTotal:
      actors.waiters.length > 0
        ? actors.peerTotal
        : Math.max(actors.peerTotal, fallbackWaiters.length),
  };
  const { shown: waiters, total } = peerSample(source, "waiters");
  const positions = [
    [55, 185],
    [160, 210],
    [265, 185],
  ] as const;

  return (
    <svg viewBox="0 0 320 250" className="legend-svg" aria-hidden="true">
      <defs>
        <marker
          id="arrow-hot"
          markerWidth="7"
          markerHeight="7"
          refX="6"
          refY="3"
          orient="auto"
        >
          <path d="M0,0 L6,3 L0,6 Z" fill="#ef4444" />
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
          width={56}
          height={56}
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
              stroke="#ef4444"
              strokeWidth="2"
              strokeDasharray="6 5"
              markerEnd="url(#arrow-hot)"
            />
            <g transform={`translate(${x} ${y})`}>
              <g
                className="legend-node-bounce"
                style={{ animationDelay: `${i * 0.25}s` }}
              >
                <circle r="26" fill="#fee2e2" stroke="#ef4444" strokeWidth="2" />
                <ActorLabel
                  actor={w}
                  fallback={w.thread}
                  width={52}
                  height={52}
                />
              </g>
            </g>
          </g>
        );
      })}
      <PeerSampleNote shown={waiters.length} total={total} x={160} y={244} />
    </svg>
  );
}

function BlockedDemo({ actors }: { actors: FindingActors }) {
  const source =
    actors.nodes.length > 0
      ? actors
      : {
          ...actors,
          nodes: [
            { thread: "blocked-1", id: null, className: null },
            { thread: "blocked-2", id: null, className: null },
            { thread: "blocked-3", id: null, className: null },
          ],
          peerTotal: Math.max(actors.peerTotal, 3),
        };
  const { shown: blocked, total } = peerSample(source);
  const ys = [48, 105, 162];

  return (
    <svg viewBox="0 0 340 240" className="legend-svg" aria-hidden="true">
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
        <ActorLabel
          actor={actors.owner}
          fallback="Owner"
          threadMax={11}
          width={52}
          height={52}
        />
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
                <ActorLabel
                  actor={b}
                  fallback={`B${i + 1}`}
                  threadMax={14}
                  width={116}
                  height={44}
                />
              </g>
            </g>
          </g>
        );
      })}
      <PeerSampleNote shown={blocked.length} total={total} x={170} y={230} />
    </svg>
  );
}

function ConnectionPoolDemo({ actors }: { actors: FindingActors }) {
  const nodes =
    actors.nodes.length > 0
      ? actors.nodes
      : [
          { thread: "db-borrower-0", id: null, className: "HikariDataSource" },
          { thread: "db-borrower-1", id: null, className: "HikariDataSource" },
          { thread: "db-borrower-2", id: null, className: "HikariDataSource" },
          { thread: "pool-holder", id: null, className: null },
        ];
  const holder =
    nodes.find((n) => /holder|pool/i.test(n.thread) && !/borrow/i.test(n.thread)) ??
    nodes[nodes.length - 1] ??
    { thread: "pool-holder", id: null, className: null };
  const waiterActors = nodes.filter((n) => n.thread !== holder.thread);
  const source: FindingActors = {
    ...actors,
    waiters: waiterActors,
    peerTotal: Math.max(actors.peerTotal, waiterActors.length),
  };
  const { shown: waiters, total } = peerSample(source, "waiters");
  const waiterXs = [70, 210, 350] as const;

  return (
    <svg viewBox={FAN.viewBox} className="legend-svg" aria-hidden="true">
      <rect
        x="160"
        y="28"
        width="100"
        height="40"
        rx="8"
        fill="#e0e7ff"
        stroke="#6366f1"
        strokeWidth="1.5"
      />
      <text
        x={FAN.cx}
        y="53"
        textAnchor="middle"
        fontSize="10"
        fontWeight="700"
        fill="#4338ca"
      >
        ConnPool(1)
      </text>
      <g transform={`translate(${FAN.cx} 110)`}>
        <g className="legend-float">
          <rect
            x="-48"
            y="-22"
            width="96"
            height="44"
            rx="8"
            fill="#ffedd5"
            stroke="#f59e0b"
            strokeWidth="2"
          />
          <ActorLabel actor={holder} fallback="holder" threadMax={11} />
        </g>
      </g>
      {waiters.map((a, i) => {
        const x = waiterXs[i] ?? FAN.cx;
        return (
          <g key={`${a.thread}-${i}`}>
            <line
              x1={x}
              y1="185"
              x2={FAN.cx}
              y2="68"
              className="legend-edge-wait"
              stroke="#f59e0b"
              strokeWidth="2"
              strokeDasharray="5 4"
            />
            <g transform={`translate(${x} 200)`}>
              <g
                className="legend-pulse"
                style={{ animationDelay: `${i * 0.2}s` }}
              >
                <rect
                  x="-48"
                  y="-18"
                  width="96"
                  height="36"
                  rx="8"
                  fill="#fee2e2"
                  stroke="#ef4444"
                  strokeWidth="2"
                />
                <ActorLabel
                  actor={a}
                  fallback={`B${i}`}
                  threadMax={10}
                  height={36}
                />
              </g>
            </g>
          </g>
        );
      })}
      <PeerSampleNote shown={waiters.length} total={total} x={FAN.cx} y={232} />
    </svg>
  );
}

function ConditionStarvationDemo({ actors }: { actors: FindingActors }) {
  const source =
    actors.nodes.length > 0
      ? actors
      : {
          ...actors,
          nodes: [
            { thread: "cond-waiter-0", id: null, className: "ConditionObject" },
            { thread: "cond-waiter-1", id: null, className: "ConditionObject" },
            { thread: "cond-waiter-2", id: null, className: "ConditionObject" },
          ],
          peerTotal: Math.max(actors.peerTotal, 3),
        };
  const { shown: nodes, total } = peerSample(source);

  return (
    <svg viewBox={FAN.viewBox} className="legend-svg" aria-hidden="true">
      <rect
        x="155"
        y="160"
        width="110"
        height="40"
        rx="8"
        fill="#e0e7ff"
        stroke="#6366f1"
        strokeWidth="1.5"
        className="legend-lock-pulse"
      />
      <text
        x={FAN.cx}
        y="185"
        textAnchor="middle"
        fontSize="10"
        fontWeight="700"
        fill="#4338ca"
      >
        {shortLock(actors.lock ?? "Condition", 14)}
      </text>
      {nodes.map((a, i) => {
        const [x, y] = FAN.nodes[i] ?? [FAN.cx, 70];
        return (
          <g key={`${a.thread}-${i}`}>
            <line
              x1={x}
              y1={y + 22}
              x2={FAN.cx}
              y2="160"
              className="legend-edge-wait"
              stroke="#f59e0b"
              strokeWidth="2"
              strokeDasharray="5 4"
            />
            <g transform={`translate(${x} ${y})`}>
              <g
                className="legend-pulse"
                style={{ animationDelay: `${i * 0.2}s` }}
              >
                <rect
                  x="-48"
                  y="-22"
                  width="96"
                  height="44"
                  rx="8"
                  fill="#fee2e2"
                  stroke="#ef4444"
                  strokeWidth="2"
                />
                <ActorLabel actor={a} fallback={`W${i}`} threadMax={11} />
              </g>
            </g>
          </g>
        );
      })}
      <PeerSampleNote shown={nodes.length} total={total} x={FAN.cx} y={228} />
    </svg>
  );
}

function BusyWaitSpinDemo({ actors }: { actors: FindingActors }) {
  const source =
    actors.nodes.length > 0
      ? actors
      : {
          ...actors,
          nodes: [
            { thread: "spin-worker-0", id: null, className: "BusyWaitSpin" },
            { thread: "spin-worker-1", id: null, className: "BusyWaitSpin" },
            { thread: "spin-worker-2", id: null, className: "BusyWaitSpin" },
          ],
          peerTotal: Math.max(actors.peerTotal, 3),
        };
  const { shown: nodes, total } = peerSample(source);

  return (
    <svg viewBox={FAN.viewBox} className="legend-svg" aria-hidden="true">
      <circle
        cx={FAN.cx}
        cy={FAN.hubY}
        r="22"
        fill="#ffedd5"
        stroke="#f59e0b"
        strokeWidth="2"
        className="legend-lock-pulse"
      />
      <text
        x={FAN.cx}
        y={FAN.hubY + 4}
        textAnchor="middle"
        fontSize="9"
        fontWeight="700"
        fill="#b45309"
      >
        CPU spin
      </text>
      {nodes.map((a, i) => {
        const [x, y] = FAN.nodes[i] ?? [FAN.cx, 70];
        return (
          <g key={`${a.thread}-${i}`}>
            <line
              x1={x}
              y1={y + 22}
              x2={FAN.cx}
              y2={FAN.hubY - 22}
              className="legend-edge-wait"
              stroke="#f59e0b"
              strokeWidth="2"
              strokeDasharray="4 3"
            />
            <g transform={`translate(${x} ${y})`}>
              <g
                className="legend-pulse"
                style={{ animationDelay: `${i * 0.12}s` }}
              >
                <rect
                  x="-48"
                  y="-22"
                  width="96"
                  height="44"
                  rx="8"
                  fill="#ffedd5"
                  stroke="#f59e0b"
                  strokeWidth="2"
                />
                <ActorLabel actor={a} fallback={`S${i}`} threadMax={11} />
              </g>
            </g>
          </g>
        );
      })}
      <PeerSampleNote shown={nodes.length} total={total} x={FAN.cx} y={228} />
    </svg>
  );
}

function SyncIoHotspotDemo({ actors }: { actors: FindingActors }) {
  const source =
    actors.nodes.length > 0
      ? actors
      : {
          ...actors,
          nodes: [
            { thread: "rpc-client-0", id: null, className: "SocketInputStream" },
            { thread: "rpc-client-1", id: null, className: "SocketInputStream" },
            { thread: "rpc-client-2", id: null, className: "SocketInputStream" },
          ],
          peerTotal: Math.max(actors.peerTotal, 3),
        };
  const { shown: nodes, total } = peerSample(source);

  return (
    <svg viewBox={FAN.viewBox} className="legend-svg" aria-hidden="true">
      <rect
        x="160"
        y="160"
        width="100"
        height="36"
        rx="8"
        fill="#e0e7ff"
        stroke="#6366f1"
        strokeWidth="1.5"
      />
      <text
        x={FAN.cx}
        y="182"
        textAnchor="middle"
        fontSize="10"
        fontWeight="700"
        fill="#4338ca"
      >
        remote I/O
      </text>
      {nodes.map((a, i) => {
        const [x, y] = FAN.nodes[i] ?? [FAN.cx, 70];
        return (
          <g key={`${a.thread}-${i}`}>
            <line
              x1={x}
              y1={y + 22}
              x2={FAN.cx}
              y2="160"
              className="legend-edge-wait"
              stroke="#f59e0b"
              strokeWidth="2"
              strokeDasharray="5 4"
            />
            <g transform={`translate(${x} ${y})`}>
              <g
                className="legend-pulse"
                style={{ animationDelay: `${i * 0.18}s` }}
              >
                <rect
                  x="-48"
                  y="-22"
                  width="96"
                  height="44"
                  rx="8"
                  fill="#ffedd5"
                  stroke="#f59e0b"
                  strokeWidth="2"
                />
                <ActorLabel actor={a} fallback={`C${i}`} threadMax={11} />
              </g>
            </g>
          </g>
        );
      })}
      <PeerSampleNote shown={nodes.length} total={total} x={FAN.cx} y={228} />
    </svg>
  );
}

function PoolExhaustionDemo({ actors }: { actors: FindingActors }) {
  const source =
    actors.nodes.length > 0
      ? actors
      : {
          ...actors,
          nodes: [
            { thread: "pool-1-thread-1", id: null, className: null },
            { thread: "pool-1-thread-2", id: null, className: null },
            { thread: "pool-1-thread-3", id: null, className: null },
          ],
          peerTotal: Math.max(actors.peerTotal, 3),
        };
  const { shown: nodes, total } = peerSample(source);

  return (
    <svg viewBox={FAN.viewBox} className="legend-svg" aria-hidden="true">
      <rect
        x="40"
        y="28"
        width="340"
        height="175"
        rx="12"
        fill="#eef2ff"
        stroke="#6366f1"
        strokeWidth="1.5"
        strokeDasharray="4 3"
      />
      <text
        x={FAN.cx}
        y="48"
        textAnchor="middle"
        fontSize="10"
        fontWeight="700"
        fill="#4f46e5"
      >
        FixedThreadPool
      </text>
      {nodes.map((a, i) => {
        const [x, y] = FAN.nodes[i] ?? [FAN.cx, 110];
        const blocked = i > 0;
        return (
          <g key={`${a.thread}-${i}`} transform={`translate(${x} ${y + 35})`}>
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
      <PeerSampleNote shown={nodes.length} total={total} x={FAN.cx} y={228} />
    </svg>
  );
}

function CleanDemo({ actors }: { actors: FindingActors }) {
  const source =
    actors.nodes.length > 0
      ? actors
      : {
          ...actors,
          nodes: [
            { thread: "T1", id: null, className: null },
            { thread: "T2", id: null, className: null },
            { thread: "T3", id: null, className: null },
          ],
          peerTotal: Math.max(actors.peerTotal, 3),
        };
  const { shown: nodes, total } = peerSample(source);
  const positions = [
    [80, 100],
    [160, 65],
    [240, 110],
  ] as const;

  return (
    <svg viewBox="0 0 320 230" className="legend-svg" aria-hidden="true">
      {nodes.map((a, i) => {
        const [x, y] = positions[i] ?? [160, 100];
        return (
          <g key={`${a.thread}-${i}`} transform={`translate(${x} ${y})`}>
            <g
              className="legend-float"
              style={{ animationDelay: `${i * 0.35}s` }}
            >
              <circle r="30" fill="#dcfce7" stroke="#22c55e" strokeWidth="2" />
              <ActorLabel
                actor={a}
                fallback={`T${i + 1}`}
                width={60}
                height={60}
              />
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
      <PeerSampleNote shown={nodes.length} total={total} x={160} y={218} />
    </svg>
  );
}

