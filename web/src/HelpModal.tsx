import { useEffect, useRef } from "react";
import { useI18n, type MessageKey } from "./i18n";
import { lockBodyScroll } from "./scrollLock";

type Props = {
  onClose: () => void;
};

const PATTERN_KEYS: MessageKey[] = [
  "help.pattern.deadlock",
  "help.pattern.lockContention",
  "help.pattern.poolExhaustion",
  "help.pattern.syncIo",
  "help.pattern.dangerousHotLock",
  "help.pattern.connectionPool",
  "help.pattern.futureLatch",
  "help.pattern.logging",
  "help.pattern.busyWait",
  "help.pattern.conditionStarvation",
  "help.pattern.lockOrder",
  "help.pattern.finalizer",
  "help.pattern.sleepScheduler",
  "help.pattern.frameworkPool",
  "help.pattern.dns",
  "help.pattern.threadLeak",
  "help.pattern.livelock",
];

function QuestionIcon() {
  return (
    <svg
      className="help-icon"
      viewBox="0 0 24 24"
      width="18"
      height="18"
      aria-hidden="true"
      focusable="false"
    >
      <circle
        cx="12"
        cy="12"
        r="9"
        fill="none"
        stroke="currentColor"
        strokeWidth="1.75"
      />
      <path
        d="M9.6 9.2c.35-1.35 1.45-2.2 2.7-2.2 1.45 0 2.55.95 2.55 2.3 0 1.15-.7 1.75-1.55 2.25-.75.45-1.15.85-1.15 1.65v.3"
        fill="none"
        stroke="currentColor"
        strokeWidth="1.75"
        strokeLinecap="round"
      />
      <circle cx="12.15" cy="17" r="1.05" fill="currentColor" />
    </svg>
  );
}

/** Animated three-step: dump file → drop into page → findings. */
function HowToAnimation() {
  const { t } = useI18n();
  return (
    <div className="help-howto" aria-hidden="true">
      <svg
        className="help-howto-svg"
        viewBox="0 0 560 150"
        role="img"
        aria-label={t("help.howto.title")}
      >
        {/* Step 1: thread dump document */}
        <g className="help-anim-step help-anim-step-1">
          <rect
            x="28"
            y="28"
            width="88"
            height="94"
            rx="10"
            fill="#eef2ff"
            stroke="#6366f1"
            strokeWidth="2"
          />
          <rect x="42" y="46" width="60" height="6" rx="3" fill="#a5b4fc" />
          <rect x="42" y="60" width="48" height="6" rx="3" fill="#c7d2fe" />
          <rect x="42" y="74" width="54" height="6" rx="3" fill="#c7d2fe" />
          <rect x="42" y="88" width="40" height="6" rx="3" fill="#c7d2fe" />
          <text x="72" y="132" textAnchor="middle" className="help-anim-label">
            {t("help.howto.step1Short")}
          </text>
        </g>

        {/* Arrow 1 → 2 */}
        <g className="help-anim-arrow help-anim-arrow-1">
          <path
            d="M130 75 H190"
            fill="none"
            stroke="#94a3b8"
            strokeWidth="2.5"
            strokeLinecap="round"
            strokeDasharray="6 6"
          />
          <path
            d="M182 68 L196 75 L182 82"
            fill="none"
            stroke="#6366f1"
            strokeWidth="2.5"
            strokeLinecap="round"
            strokeLinejoin="round"
          />
        </g>

        {/* Step 2: browser page / drop zone */}
        <g className="help-anim-step help-anim-step-2">
          <rect
            x="210"
            y="28"
            width="140"
            height="94"
            rx="10"
            fill="#fff"
            stroke="#6366f1"
            strokeWidth="2"
          />
          <rect x="222" y="40" width="116" height="14" rx="4" fill="#e0e7ff" />
          <rect
            x="228"
            y="66"
            width="104"
            height="40"
            rx="8"
            fill="#eef2ff"
            stroke="#818cf8"
            strokeWidth="1.5"
            strokeDasharray="5 4"
            className="help-anim-dropzone"
          />
          <g className="help-anim-flying-file">
            <rect
              x="262"
              y="74"
              width="36"
              height="26"
              rx="4"
              fill="#6366f1"
            />
            <rect x="268" y="80" width="24" height="3" rx="1.5" fill="#e0e7ff" />
            <rect x="268" y="86" width="18" height="3" rx="1.5" fill="#c7d2fe" />
          </g>
          <text x="280" y="132" textAnchor="middle" className="help-anim-label">
            {t("help.howto.step2Short")}
          </text>
        </g>

        {/* Arrow 2 → 3 */}
        <g className="help-anim-arrow help-anim-arrow-2">
          <path
            d="M364 75 H424"
            fill="none"
            stroke="#94a3b8"
            strokeWidth="2.5"
            strokeLinecap="round"
            strokeDasharray="6 6"
          />
          <path
            d="M416 68 L430 75 L416 82"
            fill="none"
            stroke="#6366f1"
            strokeWidth="2.5"
            strokeLinecap="round"
            strokeLinejoin="round"
          />
        </g>

        {/* Step 3: results / findings */}
        <g className="help-anim-step help-anim-step-3">
          <rect
            x="444"
            y="28"
            width="88"
            height="94"
            rx="10"
            fill="#ecfdf5"
            stroke="#059669"
            strokeWidth="2"
          />
          <rect x="458" y="46" width="60" height="10" rx="3" fill="#34d399" />
          <rect x="458" y="64" width="48" height="6" rx="3" fill="#a7f3d0" />
          <rect x="458" y="78" width="54" height="6" rx="3" fill="#a7f3d0" />
          <rect x="458" y="92" width="36" height="6" rx="3" fill="#6ee7b7" />
          <text x="488" y="132" textAnchor="middle" className="help-anim-label">
            {t("help.howto.step3Short")}
          </text>
        </g>
      </svg>
    </div>
  );
}

export function HelpButton({ onClick }: { onClick: () => void }) {
  const { t } = useI18n();
  return (
    <button
      type="button"
      className="btn icon-btn"
      data-testid="open-help"
      onClick={onClick}
      aria-label={t("app.help")}
      title={t("app.help")}
    >
      <QuestionIcon />
    </button>
  );
}

export default function HelpModal({ onClose }: Props) {
  const { t } = useI18n();
  const closeBtnRef = useRef<HTMLButtonElement>(null);

  useEffect(() => {
    closeBtnRef.current?.focus();
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

  return (
    <div
      className="modal-backdrop"
      data-testid="help-modal"
      onClick={(e) => {
        if (e.target === e.currentTarget) onClose();
      }}
    >
      <div
        className="modal help-modal"
        role="dialog"
        aria-modal="true"
        aria-labelledby="help-title"
      >
        <div className="modal-header">
          <h2 id="help-title">{t("help.title")}</h2>
          <button
            ref={closeBtnRef}
            type="button"
            className="modal-close"
            aria-label={t("help.close")}
            onClick={onClose}
          >
            ×
          </button>
        </div>

        <p className="help-lead">{t("help.lead")}</p>

        <section className="help-section" aria-labelledby="help-howto-heading">
          <h3 id="help-howto-heading">{t("help.howto.title")}</h3>
          <p className="help-section-body">{t("help.howto.body")}</p>
          <HowToAnimation />
          <ol className="help-steps">
            <li>{t("help.howto.step1")}</li>
            <li>{t("help.howto.step2")}</li>
            <li>{t("help.howto.step3")}</li>
          </ol>
        </section>

        <section className="help-section help-security" aria-labelledby="help-security-heading">
          <h3 id="help-security-heading">{t("help.security.title")}</h3>
          <p className="help-section-body">{t("help.security.body")}</p>
          <ul className="help-bullets">
            <li>{t("help.security.point1")}</li>
            <li>{t("help.security.point2")}</li>
            <li>{t("help.security.point3")}</li>
          </ul>
        </section>

        <section className="help-section" aria-labelledby="help-patterns-heading">
          <h3 id="help-patterns-heading">{t("help.patterns.title")}</h3>
          <p className="help-section-body">{t("help.patterns.body")}</p>
          <ul className="help-pattern-grid">
            {PATTERN_KEYS.map((key) => (
              <li key={key}>{t(key)}</li>
            ))}
          </ul>
        </section>

        <section className="help-section" aria-labelledby="help-java-heading">
          <h3 id="help-java-heading">{t("help.java.title")}</h3>
          <p className="help-section-body">{t("help.java.body")}</p>
          <div className="help-java-badges" aria-label={t("help.java.versions")}>
            {["8", "11", "17", "21"].map((v) => (
              <span key={v} className="help-java-badge">
                Java {v}
              </span>
            ))}
          </div>
          <p className="help-section-note">{t("help.java.formats")}</p>
        </section>
      </div>
    </div>
  );
}
