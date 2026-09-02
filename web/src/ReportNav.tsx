import { NAV_LABEL_KEYS, sectionDomId, type ReportSection } from "./reportNav";
import { useI18n } from "./i18n";

interface Props {
  sections: ReportSection[];
  collapsed: boolean;
  onToggle: () => void;
  activeId: string | null;
  onNavigate: (domId: string) => void;
}

export default function ReportNav({
  sections,
  collapsed,
  onToggle,
  activeId,
  onNavigate,
}: Props) {
  const { t } = useI18n();

  return (
    <nav
      className="report-nav"
      data-testid="report-nav"
      aria-label={t("nav.title")}
    >
      <p className="report-nav-heading">{t("nav.title")}</p>
      <ul className="report-nav-list">
        {sections.map((section) => {
          const domId = sectionDomId(section.id);
          const active = activeId === section.id;
          return (
            <li key={section.id}>
              <a
                href={`#${domId}`}
                className={`report-nav-link${active ? " is-active" : ""}`}
                aria-current={active ? "location" : undefined}
                onClick={(e) => {
                  e.preventDefault();
                  onNavigate(domId);
                }}
              >
                {t(NAV_LABEL_KEYS[section.id])}
              </a>
            </li>
          );
        })}
      </ul>
      <button
        type="button"
        className="report-nav-toggle"
        data-testid="report-nav-toggle"
        aria-expanded={!collapsed}
        onClick={onToggle}
      >
        {collapsed ? t("nav.expand") : t("nav.collapse")}
      </button>
    </nav>
  );
}
