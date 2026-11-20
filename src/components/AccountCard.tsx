import { useState, type ReactNode } from "react";

import {
  formatDuration,
  formatMoney,
  formatPercent,
  formatResetTime,
  t,
} from "../i18n";
import type { AccountUsage, PercentMode, UsageWindow } from "../types";
import { ChevronIcon } from "./Icons";
import { LabelBadge } from "./LabelBadge";
import { Sparkline } from "./Sparkline";
import { UsageBar } from "./UsageBar";

interface AccountCardProps {
  account: AccountUsage;
  percentMode: PercentMode;
  /** 현재 시각(ms). 남은 시간 계산에 쓴다. */
  now: number;
}

/** 팝오버의 계정 카드 하나 */
export function AccountCard({ account, percentMode, now }: AccountCardProps) {
  const shown = (used: number) => (percentMode === "used" ? used : 100 - used);

  return (
    <section className="card">
      <div className="card-head">
        <LabelBadge label={account.label} color={account.color} />
        <span className="card-name">{account.name}</span>
        {account.plan && <span className="card-plan">{account.plan}</span>}
      </div>

      {account.weekly && (
        <UsageRow title={t("weekly")} window={account.weekly} value={shown(account.weekly.usedPercent)} now={now} />
      )}
      {account.session && (
        <UsageRow title={t("session")} window={account.session} value={shown(account.session.usedPercent)} now={now} />
      )}

      {account.models.length > 0 && (
        <Disclosure
          title={t("models")}
          summary={account.models.map((m) => `${m.name} ${formatPercent(shown(m.usedPercent))}`).join(" · ")}
        >
          {account.models.map((model) => (
            <div className="detail-row" key={model.name}>
              <span className="detail-name">{model.name}</span>
              <UsageBar used={model.usedPercent} elapsed={account.weekly?.elapsedPercent} />
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
            <UsageBar
              used={account.extra.limit ? (account.extra.used / account.extra.limit) * 100 : 0}
            />
            <span className="detail-value">
              {account.extra.limit !== null ? formatMoney(account.extra.limit, account.extra.currency) : "—"}
            </span>
          </div>
        </Disclosure>
      )}

      {account.trend.length > 1 && (
        <div className="trend">
          <span className="trend-label">{t("recentTrend")}</span>
          <Sparkline points={account.trend} color={account.color} />
        </div>
      )}
    </section>
  );
}

function UsageRow({
  title,
  window,
  value,
  now,
}: {
  title: string;
  window: UsageWindow;
  value: number;
  now: number;
}) {
  const remaining = new Date(window.resetsAt).getTime() - now;
  return (
    <div className="usage">
      <div className="usage-row">
        <span className="usage-title">{title}</span>
        <UsageBar used={window.usedPercent} elapsed={window.elapsedPercent} warn={window.aheadOfPace} />
        <span className={`usage-value${window.aheadOfPace ? " warn" : ""}`}>{formatPercent(value)}</span>
      </div>
      <div className="usage-meta">
        {t("resets", { time: formatResetTime(window.resetsAt, now) })} ·{" "}
        {t("left", { duration: formatDuration(remaining) })}
        {window.aheadOfPace && <span className="warn-text"> · {t("aheadOfPace")}</span>}
      </div>
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
