import { useEffect, useRef, useState } from "react";
import { clipboardTextIsEmpty } from "./clipboardImport";
import { useI18n } from "./i18n";
import { lockBodyScroll } from "./scrollLock";

type Props = {
  onClose: () => void;
  onAnalyze: (text: string) => void;
};

export default function PasteDumpModal({ onClose, onAnalyze }: Props) {
  const { t } = useI18n();
  const textareaRef = useRef<HTMLTextAreaElement>(null);
  const [text, setText] = useState("");
  const canAnalyze = !clipboardTextIsEmpty(text);

  useEffect(() => {
    textareaRef.current?.focus();
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") {
        e.preventDefault();
        onClose();
      }
    };
    window.addEventListener("keydown", onKey);
    const unlock = lockBodyScroll();
    return () => {
      window.removeEventListener("keydown", onKey);
      unlock();
    };
  }, [onClose]);

  const submit = () => {
    if (!canAnalyze) return;
    onAnalyze(text);
  };

  return (
    <div
      className="modal-backdrop"
      data-testid="paste-modal"
      onClick={(e) => {
        if (e.target === e.currentTarget) onClose();
      }}
    >
      <div
        className="modal paste-modal"
        role="dialog"
        aria-modal="true"
        aria-labelledby="paste-modal-title"
      >
        <div className="modal-header">
          <h2 id="paste-modal-title">{t("app.pasteModalTitle")}</h2>
          <button
            type="button"
            className="modal-close"
            aria-label={t("help.close")}
            onClick={onClose}
          >
            ×
          </button>
        </div>
        <p className="paste-modal-hint">{t("app.pasteModalHint")}</p>
        <textarea
          ref={textareaRef}
          className="paste-modal-input"
          data-testid="paste-modal-input"
          rows={12}
          spellCheck={false}
          placeholder={t("app.pasteModalPlaceholder")}
          value={text}
          onChange={(e) => setText(e.target.value)}
          onKeyDown={(e) => {
            if ((e.metaKey || e.ctrlKey) && e.key === "Enter") {
              e.preventDefault();
              submit();
            }
          }}
        />
        <div className="modal-actions">
          <button type="button" className="btn" onClick={onClose}>
            {t("app.pasteModalCancel")}
          </button>
          <button
            type="button"
            className="btn primary"
            data-testid="paste-modal-analyze"
            disabled={!canAnalyze}
            onClick={submit}
          >
            {t("app.pasteModalAnalyze")}
          </button>
        </div>
      </div>
    </div>
  );
}
