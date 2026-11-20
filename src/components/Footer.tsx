import { useState } from "react";
import { formatCount, formatDay, formatTokens, formatUpdated, formatUsd, t } from "../i18n";
import type { BrandTokens, Summary } from "../types";
import { ChevronIcon } from "./Icons";

interface FooterProps {
  summary: Summary;
  updatedAt: string;
  now: number;
  onQuit: () => void;
}

/** 팝오버 아래쪽: 브랜드별 누적 토큰, API 비용, 서비스 상태, 갱신 시각 */
export function Footer({ summary, updatedAt, now, onQuit }: FooterProps) {
  const showScanning = summary.tokensScanning && summary.tokens.length === 0;
  const hasSummary =
    summary.tokens.length > 0 || showScanning || summary.apiMonthCostUsd !== null || summary.services.length > 0;

  return (
    <footer className="footer">
      {hasSummary && (
        <div className="summary">
          {summary.tokens.map((brand) => (
            <TokenRow key={brand.provider} brand={brand} />
          ))}
          {showScanning && <div className="summary-note">{t("tokensScanning")}</div>}
          {summary.apiMonthCostUsd !== null && (
            <div className="summary-row">
              <span className="summary-label">{t("apiThisMonth")}</span>
              <span className="summary-value">{formatUsd(summary.apiMonthCostUsd)}</span>
            </div>
          )}
          {summary.services.length > 0 && (
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
          )}
        </div>
      )}

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

/** "Claude 누적  24.8억 토큰" 한 줄. 누르면 오늘·입력·출력·캐시 내역이 펼쳐진다. */
function TokenRow({ brand }: { brand: BrandTokens }) {
  const [open, setOpen] = useState(false);
  const label = brand.provider === "claude" ? t("tokensClaude") : t("tokensCodex");

  // 값이 있는 항목만 (입력·출력은 항상)
  const details: [string, number][] = [
    [t("tokenToday"), brand.today],
    [t("tokenInput"), brand.input],
    [t("tokenOutput"), brand.output],
    ...(brand.reasoning > 0 ? [[t("tokenReasoning"), brand.reasoning] as [string, number]] : []),
    ...(brand.cacheWrite > 0 ? [[t("tokenCacheWrite"), brand.cacheWrite] as [string, number]] : []),
    ...(brand.cacheRead > 0 ? [[t("tokenCacheRead"), brand.cacheRead] as [string, number]] : []),
  ];

  return (
    <div className="token-row">
      <button className="summary-row token-head" onClick={() => setOpen(!open)} aria-expanded={open} title={t("tokensHelp")}>
        <span className="summary-label token-label">
          <ChevronIcon direction={open ? "down" : "right"} />
          {label}
        </span>
        <span className="summary-value">{formatTokens(brand.total)}</span>
      </button>
      {open && (
        <div className="token-detail">
          {details.map(([name, value]) => (
            <div className="token-detail-row" key={name}>
              <span>{name}</span>
              <span className="summary-value">{formatCount(value)}</span>
            </div>
          ))}
          {brand.since && <div className="summary-note">{t("tokensSince", { date: formatDay(brand.since) })}</div>}
        </div>
      )}
    </div>
  );
}
