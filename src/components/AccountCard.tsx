import { useState, type ReactNode } from "react";

import {
  formatAsOf,
  formatClock,
  formatHours,
  formatMoney,
  formatPercent,
  formatApproxTime,
  formatResetTime,
  formatUsd,
  t,
} from "../i18n";
import type { AccountStatus, AccountUsage, PercentMode, UsageWindow } from "../types";
import { ChevronIcon } from "./Icons";
import { LabelBadge } from "./LabelBadge";
import { Sparkline, WEEK_MS, trendSpanMs } from "./Sparkline";
import { UsageBar } from "./UsageBar";

interface AccountCardProps {
  account: AccountUsage;
  percentMode: PercentMode;
  /** 현재 시각(ms). 남은 시간 계산에 쓴다. */
  now: number;
  /** 로그인 창 열기 (로그인 필요, claude.ai 확인 필요일 때) */
  onSignIn: (accountId: string) => void;
}

/** 팝오버의 계정 카드 하나 */
export function AccountCard({ account, percentMode, now, onSignIn }: AccountCardProps) {
  const remaining = percentMode === "remaining";
  // 작은 그래프 가로축: 최근 7일 (모든 계정이 같은 시간축)
  const trendDomain: [number, number] | undefined =
    account.trendStart && account.trendEnd
      ? [new Date(account.trendStart).getTime(), new Date(account.trendEnd).getTime()]
      : undefined;
  // 남은 양 모드: 숫자·막대 채움 = 남은 사용량, 눈금 = 남은 시간
  const shown = (used: number) => (remaining ? 100 - used : used);
  const marker = (elapsed: number | null | undefined) =>
    elapsed === null || elapsed === undefined ? undefined : remaining ? 100 - elapsed : elapsed;
  const hasUsage = account.weekly !== null || account.session !== null;

  return (
    <section className="card">
      <div className="card-head">
        <LabelBadge label={account.label} color={account.color} />
        <span className="card-name">{account.name}</span>
        {account.plan && <span className="card-plan">{account.plan}</span>}
      </div>

      <StatusLine
        status={account.status}
        updatedAt={hasUsage ? account.updatedAt : null}
        now={now}
        canSignIn={account.provider === "claude"}
        onSignIn={() => onSignIn(account.id)}
      />

      {account.weekly && (
        <UsageRow
          title={t("weekly")}
          window={account.weekly}
          value={shown(account.weekly.usedPercent)}
          marker={marker(account.weekly.elapsedPercent)}
          now={now}
        />
      )}
      {account.session && (
        <UsageRow
          title={t("session")}
          window={account.session}
          value={shown(account.session.usedPercent)}
          marker={marker(account.session.elapsedPercent)}
          now={now}
        />
      )}
      {!hasUsage && account.status.kind === "ok" && <p className="card-note">{t("noUsageData")}</p>}

      {account.models.length > 0 && (
        <Disclosure
          title={t("models")}
          summary={account.models.map((m) => `${m.name} ${formatPercent(shown(m.usedPercent))}`).join(" · ")}
        >
          {account.models.map((model) => (
            <div className="detail-row" key={model.name}>
              <span className="detail-name">{model.name}</span>
              <UsageBar used={shown(model.usedPercent)} elapsed={marker(account.weekly?.elapsedPercent)} />
              <span className="detail-value">{formatPercent(shown(model.usedPercent))}</span>
            </div>
          ))}
        </Disclosure>
      )}

      {account.extra && (
        <Disclosure
          title={t("extraUsage")}
          summary={
            account.extra.limit !== null
              ? `${formatMoney(account.extra.used, account.extra.currency)} / ${formatMoney(account.extra.limit, account.extra.currency)}`
              : formatMoney(account.extra.used, account.extra.currency)
          }
        >
          <div className="detail-row">
            <span className="detail-name">{formatMoney(account.extra.used, account.extra.currency)}</span>
            <UsageBar used={account.extra.limit ? (account.extra.used / account.extra.limit) * 100 : 0} />
            <span className="detail-value">
              {account.extra.limit !== null ? formatMoney(account.extra.limit, account.extra.currency) : "—"}
            </span>
          </div>
        </Disclosure>
      )}

      {account.apiCost && (
        <div
          className="detail-line"
          title={
            account.apiCost.refreshedAt
              ? t("apiCostAsOf", { time: formatAsOf(account.apiCost.refreshedAt, Date.now()) })
              : t("apiCostNoData")
          }
        >
          <span>{t("apiThisMonth")}</span>
          <span className="detail-line-value">{formatUsd(account.apiCost.usd)}</span>
        </div>
      )}

      {/* 그래프는 표시 기준과 상관없이 쓴 양(누적): 리셋 때 0% 로 떨어졌다 다시 쌓이는 모양 */}
      {(trendDomain ? account.trend.length > 0 : account.trend.length > 1) && (
        <div className="trend">
          <span className="trend-label">{trendDomain ? t("recentTrend") : trendLabel(account.trend)}</span>
          <Sparkline points={account.trend} color={account.color} domain={trendDomain} />
        </div>
      )}
    </section>
  );
}

/** 그래프 이름표: 7일치가 있으면 "최근 7일", 아니면 실제 기록 기간 (예: "최근 1시간") */
function trendLabel(points: AccountUsage["trend"]): string {
  const span = trendSpanMs(points);
  if (span >= WEEK_MS) return t("recentTrend");
  // 한 단위로만 짧게: 하루가 넘으면 "N일", 아니면 "N시간"
  const hours = Math.max(1, Math.floor(span / 3_600_000));
  const duration = hours >= 24 ? t("days", { n: Math.floor(hours / 24) }) : t("hours", { n: hours });
  return t("recentSpan", { duration });
}

/** 정상이 아닐 때 보여 주는 안내 줄 (불러오는 중, 로그인 필요, 오류) */
/** 데이터가 이만큼 오래되면 "몇 시 기준" 이라고 알려 준다 */
const STALE_AFTER_MS = 10 * 60_000;

function StatusLine({
  status,
  updatedAt,
  now,
  canSignIn,
  onSignIn,
}: {
  status: AccountStatus;
  updatedAt: string | null;
  now: number;
  /** 앱 안 로그인 창이 있는 계정인지 (Claude 만) */
  canSignIn: boolean;
  onSignIn: () => void;
}) {
  if (status.kind === "ok") {
    // 정상이어도 데이터가 오래됐으면(예: Codex 로그로 대신 보여 줄 때) 기준 시각을 알려 준다.
    const isStale = updatedAt !== null && now - new Date(updatedAt).getTime() > STALE_AFTER_MS;
    return isStale ? <p className="status-line">{t("staleAsOf", { time: formatClock(updatedAt) })}</p> : null;
  }

  // 오류가 났어도 예전 데이터가 있으면 언제 데이터인지 알려 준다.
  const stale = updatedAt ? ` · ${t("staleSince", { time: formatClock(updatedAt) })}` : "";

  switch (status.kind) {
    case "loading":
      return <p className="status-line">{t("loading")}</p>;
    case "needsLogin":
      return (
        <p className="status-line attention">
          <span>
            {canSignIn ? t("needsLogin") : t("codexNeedsLogin")}
            {stale}
          </span>
          {canSignIn && (
            <button className="link-button" onClick={onSignIn}>
              {t("signIn")}
            </button>
          )}
        </p>
      );
    case "challenge":
      return (
        <p className="status-line attention">
          <span>
            {t("challenge")}
            {stale}
          </span>
          <button className="link-button" onClick={onSignIn}>
            {t("open")}
          </button>
        </p>
      );
    case "error":
      return (
        <p className="status-line attention" title={status.message}>
          <span className="status-message">
            {status.message}
            {stale}
          </span>
        </p>
      );
  }
}

function UsageRow({
  title,
  window,
  value,
  marker,
  now,
}: {
  title: string;
  window: UsageWindow;
  /** 숫자와 막대 채움 (사용한 양 또는 남은 양) */
  value: number;
  /** 시간 눈금 위치 (지난 시간 또는 남은 시간). 없으면 눈금 없음 */
  marker: number | undefined;
  now: number;
}) {
  return (
    <div className="usage">
      <div className="usage-row">
        <span className="usage-title">{title}</span>
        <UsageBar used={value} elapsed={marker} warn={window.aheadOfPace} />
        <span className={`usage-value${window.aheadOfPace ? " warn" : ""}`}>{formatPercent(value)}</span>
      </div>
      <div className="usage-meta">
        {window.resetsAt ? (
          <>
            {t("resets", { time: formatResetTime(window.resetsAt, now) })} ·{" "}
            {t("left", { duration: formatHours(new Date(window.resetsAt).getTime() - now) })}
          </>
        ) : (
          t("notUsedYet")
        )}
        {/* 주기 초반이라 아직 예측을 못 할 때만 짧게 알린다 (예측이 있으면 아래 줄이 대신한다) */}
        {window.aheadOfPace && !window.runsOutAt && window.usedPercent < 100 && (
          <span className="warn-text"> · {t("aheadOfPace")}</span>
        )}
      </div>
      <Forecast window={window} now={now} />
    </div>
  );
}

/**
 * 한도 아래의 예측 한 줄: 이 속도면 리셋 전에 바닥날 때만, 언제쯤 다 쓰는지 알려 준다.
 * 페이스보다 5%p 넘게 앞서면 빨강. 여유가 있으면 아무것도 띄우지 않는다.
 */
function Forecast({ window, now }: { window: UsageWindow; now: number }) {
  if (!window.runsOutAt || window.usedPercent >= 100) return null;
  return (
    <div className={`usage-forecast${window.aheadOfPace ? " warn-text" : ""}`}>
      {t("forecastRunsOut", { time: formatApproxTime(window.runsOutAt, now) })}
    </div>
  );
}

function Disclosure({ title, summary, children }: { title: string; summary: string; children: ReactNode }) {
  const [open, setOpen] = useState(false);
  return (
    <div className="disclosure">
      <button className="disclosure-head" onClick={() => setOpen(!open)} aria-expanded={open}>
        <ChevronIcon direction={open ? "down" : "right"} />
        <span>{title}</span>
        {!open && <span className="disclosure-summary">{summary}</span>}
      </button>
      {open && <div className="disclosure-body">{children}</div>}
    </div>
  );
}
