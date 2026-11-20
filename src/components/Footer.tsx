import { useState } from "react";
import { openHistory } from "../api";
import { formatApproxUsd, formatCount, formatDay, formatTokens, formatUpdated, t } from "../i18n";
import type { BrandTokens, Summary } from "../types";
import { ChevronIcon } from "./Icons";

interface FooterProps {
  summary: Summary;
  updatedAt: string;
  now: number;
  onQuit: () => void;
}

/** 팝오버 아래쪽: 브랜드별 누적 토큰, 서비스 상태, 갱신 시각 (API 비용은 계정 카드 안에) */
export function Footer({ summary, updatedAt, now, onQuit }: FooterProps) {
  const showScanning = summary.tokensScanning && summary.tokens.length === 0;
  const hasSummary = summary.tokens.length > 0 || showScanning || summary.services.length > 0;

  return (
    <footer className="footer">
      {hasSummary && (
        <div className="summary">
          {summary.tokens.map((brand) => (
            <TokenRow key={brand.provider} brand={brand} />
          ))}
          {showScanning && <div className="summary-note">{t("tokensScanning")}</div>}
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
          <button className="text-button" onClick={() => openHistory()}>
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

  // 값이 있는 항목만 (입력·출력은 항상). [이름, 보여 줄 값]
  const details: [string, string][] = [
    [t("tokenToday"), formatCount(brand.today)],
    [t("tokenInput"), formatCount(brand.input)],
    [t("tokenOutput"), formatCount(brand.output)],
    ...(brand.reasoning > 0 ? [[t("tokenReasoning"), formatCount(brand.reasoning)] as [string, string]] : []),
    ...(brand.cacheWrite > 0 ? [[t("tokenCacheWrite"), formatCount(brand.cacheWrite)] as [string, string]] : []),
    ...(brand.cacheRead > 0 ? [[t("tokenCacheRead"), formatCount(brand.cacheRead)] as [string, string]] : []),
    // 구독 대신 API 가격으로 냈다면 (추정)
    ...(brand.apiCostMonthUsd !== null
      ? [[t("apiEquivMonth"), formatApproxUsd(brand.apiCostMonthUsd)] as [string, string]]
      : []),
    ...(brand.apiCostTotalUsd !== null
      ? [[t("apiEquivTotal"), formatApproxUsd(brand.apiCostTotalUsd)] as [string, string]]
      : []),
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
              <span className="summary-value">{value}</span>
            </div>
          ))}
          {brand.since && <div className="summary-note">{t("tokensSince", { date: formatDay(brand.since) })}</div>}
          {brand.apiCostTotalUsd !== null && <div className="summary-note">{t("apiEquivNote")}</div>}
          {brand.unpricedTokens > 0 && (
            <div className="summary-note">{t("apiEquivUnpriced", { count: formatCount(brand.unpricedTokens) })}</div>
          )}
        </div>
      )}
    </div>
  );
}
