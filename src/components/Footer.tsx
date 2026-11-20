import { formatTokens, formatUpdated, formatUsd, t } from "../i18n";
import type { Summary } from "../types";

interface FooterProps {
  summary: Summary;
  updatedAt: string;
  now: number;
  onQuit: () => void;
}

/** 팝오버 아래쪽: Claude Code 오늘 사용량, API 비용, 서비스 상태, 갱신 시각 */
export function Footer({ summary, updatedAt, now, onQuit }: FooterProps) {
  return (
    <footer className="footer">
      {summary.claudeCodeToday && (
        <div className="summary-row">
          <span className="summary-label">{t("claudeCodeToday")}</span>
          <span className="summary-value">
            {formatTokens(summary.claudeCodeToday.tokens)} · {formatUsd(summary.claudeCodeToday.costUsd)}
          </span>
        </div>
      )}
      {summary.apiMonthCostUsd !== null && (
        <div className="summary-row">
          <span className="summary-label">{t("apiThisMonth")}</span>
          <span className="summary-value">{formatUsd(summary.apiMonthCostUsd)}</span>
        </div>
      )}
      <div className="summary-row">
        <span className="summary-label">{t("services")}</span>
        <span className="services">
          {summary.services.map((service) => (
            <span className="service" key={service.name} title={t(service.state)}>
              <span className={`status-dot ${service.state}`} />
              {service.name}
            </span>
          ))}
        </span>
      </div>

      <div className="footer-bar">
        <span className="updated">{formatUpdated(updatedAt, now)}</span>
        <div className="footer-actions">
          <button className="text-button" disabled title={t("comingLater")}>
            {t("history")}
          </button>
          <button className="text-button" onClick={onQuit}>
            {t("quit")}
          </button>
        </div>
      </div>
    </footer>
  );
}
