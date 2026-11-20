import { useCallback, useEffect, useState } from "react";

import { getHistory, getSettings, onSettingsChanged, onSnapshotUpdated } from "../api";
import { Segmented } from "../components/Segmented";
import { formatAsOf, locale, t } from "../i18n";
import { chartColor } from "../palette";
import type { HistoryAccount, HistoryCycle, HistoryData, HistoryRange, HistorySeries, Settings } from "../types";
import { LineChart, valueAt, type ChartLine, type ChartMarker } from "./LineChart";
import { cycleTicks, timeTicks } from "./ticks";
import "./history.css";

const HOUR = 3_600_000;
const WEEK_HOURS = 7 * 24;
/** 사이클 툴팁에 보여 줄 지난 주기 수 (최근 것부터) */
const TOOLTIP_PAST_CYCLES = 3;

const tooltipTime = new Intl.DateTimeFormat(locale, {
  month: "short",
  day: "numeric",
  weekday: "short",
  hour: "numeric",
  minute: "2-digit",
});
const shortDay = new Intl.DateTimeFormat(locale, { month: "short", day: "numeric" });
const tableDay = new Intl.DateTimeFormat(locale, { month: "short", day: "numeric", weekday: "short" });

/** 시스템이 다크 모드인지 (바뀌면 다시 그림) */
function usePrefersDark(): boolean {
  const query = "(prefers-color-scheme: dark)";
  const [dark, setDark] = useState(() => window.matchMedia(query).matches);
  useEffect(() => {
    const media = window.matchMedia(query);
    const change = () => setDark(media.matches);
    media.addEventListener("change", change);
    return () => media.removeEventListener("change", change);
  }, []);
  return dark;
}

/** 히스토리 창: 주간·5시간 사용률 추이와 주간 주기 비교 */
export function HistoryApp() {
  const [range, setRange] = useState<HistoryRange>("7d");
  const [data, setData] = useState<HistoryData | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [settings, setSettings] = useState<Settings | null>(null);
  const dark = usePrefersDark();

  useEffect(() => {
    document.title = t("historyTitle");
    getSettings().then(setSettings);
    const unlisten = onSettingsChanged(setSettings);
    return () => {
      unlisten.then((stop) => stop());
    };
  }, []);

  const load = useCallback(() => {
    setLoading(true);
    getHistory(range)
      .then((result) => {
        setData(result);
        setError(null);
      })
      .catch((reason) => setError(String(reason)))
      .finally(() => setLoading(false));
  }, [range]);

  useEffect(load, [load]);

  // 새 사용량이 들어오면(1분마다) 다시 불러온다
  useEffect(() => {
    const unlisten = onSnapshotUpdated(() => load());
    return () => {
      unlisten.then((stop) => stop());
    };
  }, [load]);

  const remaining = settings?.percentMode !== "used";
  const show = (percent: number) => (remaining ? 100 - percent : percent);
  const colorOf = (account: HistoryAccount) => chartColor(account.color, dark);

  return (
    <div className={`history${loading && data ? " refreshing" : ""}`}>
      <header className="history-header">
        <h1>{t("historyTitle")}</h1>
        <Segmented
          value={range}
          options={[
            ["cycle", t("rangeCycle")],
            ["7d", t("range7d")],
            ["30d", t("range30d")],
            ["all", t("rangeAll")],
          ]}
          onChange={setRange}
        />
        <span className="history-basis">{remaining ? t("basisRemaining") : t("basisUsed")}</span>
      </header>

      {error && <p className="error-text">{`${t("loadFailed")}: ${error}`}</p>}

      {data && data.weekly.length === 0 && data.session.length === 0 ? (
        <p className="history-empty">{t("noHistory")}</p>
      ) : (
        data && (
          <>
            <TrendCard
              title={t("weeklyUsage")}
              data={data}
              series={data.weekly}
              show={show}
              colorOf={colorOf}
              withResets
            />
            {data.session.length > 0 && (
              <TrendCard title={t("sessionUsage")} data={data} series={data.session} show={show} colorOf={colorOf} />
            )}
            <CyclesCard data={data} show={show} colorOf={colorOf} />
          </>
        )
      )}
    </div>
  );
}

interface TrendCardProps {
  title: string;
  data: HistoryData;
  series: HistorySeries[];
  show: (percent: number) => number;
  colorOf: (account: HistoryAccount) => string;
  withResets?: boolean;
}

/** 계정별 사용률 추이 (가로축: 시각) */
function TrendCard({ title, data, series, show, colorOf, withResets = false }: TrendCardProps) {
  const [tableOpen, setTableOpen] = useState(false);
  const start = new Date(data.rangeStart).getTime();
  const end = new Date(data.rangeEnd).getTime();
  const accountOf = (id: string) => data.accounts.find((account) => account.id === id);

  const lines: ChartLine[] = series.flatMap((item) => {
    const account = accountOf(item.accountId);
    if (!account) return [];
    return [
      {
        id: item.accountId,
        label: account.name,
        color: colorOf(account),
        points: item.points.map((point) => ({ x: new Date(point.at).getTime(), y: show(point.percent) })),
      },
    ];
  });

  // 리셋 지점: 리셋 직후 값(0% → 표시 기준에 맞춰) 위에 점
  const markers: ChartMarker[] = withResets
    ? series.flatMap((item) => {
        const account = accountOf(item.accountId);
        if (!account) return [];
        return item.resets.map((at) => ({ x: new Date(at).getTime(), y: show(0), color: colorOf(account) }));
      })
    : [];

  return (
    <section className="history-card">
      <div className="history-card-head">
        <h2>{title}</h2>
        <button className="text-button" onClick={() => setTableOpen(!tableOpen)} aria-expanded={tableOpen}>
          {tableOpen ? t("hideTable") : t("showTable")}
        </button>
      </div>
      <Legend
        items={lines.map((line) => ({ key: line.id, label: line.label, color: line.color }))}
        extra={withResets && markers.length > 0 ? [{ key: "reset", label: t("resetMarker"), dot: true }] : []}
      />
      <LineChart
        lines={lines}
        xDomain={[start, end]}
        xTicks={(width) => timeTicks(start, end, width)}
        height={240}
        markers={markers}
        formatX={(x) => tooltipTime.format(new Date(x))}
        areaFor={lines.length === 1 ? lines[0].id : undefined}
        endLabels
        ariaLabel={title}
      />
      {tableOpen && <TrendTable lines={lines} start={start} end={end} />}
    </section>
  );
}

/** 표로 보기: 날짜별(그날 끝 시점) 값 */
function TrendTable({ lines, start, end }: { lines: ChartLine[]; start: number; end: number }) {
  const days: number[] = [];
  const cursor = new Date(end);
  cursor.setHours(0, 0, 0, 0);
  while (cursor.getTime() + 24 * HOUR > start && days.length < 400) {
    days.push(cursor.getTime());
    cursor.setDate(cursor.getDate() - 1);
  }
  return (
    <div className="table-wrap">
      <table className="data-table">
        <thead>
          <tr>
            <th>{t("date")}</th>
            {lines.map((line) => (
              <th key={line.id}>{line.label}</th>
            ))}
          </tr>
        </thead>
        <tbody>
          {days.map((day) => {
            const dayEnd = Math.min(end, day + 24 * HOUR - 1);
            return (
              <tr key={day}>
                <td>{tableDay.format(new Date(day))}</td>
                {lines.map((line) => {
                  const value = valueAt(line, dayEnd);
                  return <td key={line.id}>{value === null ? "–" : `${Math.round(value)}%`}</td>;
                })}
              </tr>
            );
          })}
        </tbody>
      </table>
    </div>
  );
}

interface CyclesCardProps {
  data: HistoryData;
  show: (percent: number) => number;
  colorOf: (account: HistoryAccount) => string;
}

/** 주간 주기 비교: 계정마다 작은 그래프 하나. 같은 경과 시간에서 이번 주기와 지난 주기를 겹쳐 본다. */
function CyclesCard({ data, show, colorOf }: CyclesCardProps) {
  const [tableOpen, setTableOpen] = useState(false);
  const groups = data.accounts
    .map((account) => ({
      account,
      cycles: data.cycles
        .filter((cycle) => cycle.accountId === account.id)
        .sort((a, b) => new Date(a.start).getTime() - new Date(b.start).getTime()),
    }))
    .filter((group) => group.cycles.length > 0);
  if (groups.length === 0) return null;

  const pace: ChartLine = {
    id: "pace",
    label: t("pace"),
    color: "",
    tone: "reference",
    step: false,
    points: [
      { x: 0, y: show(0) },
      { x: WEEK_HOURS, y: show(100) },
    ],
  };

  return (
    <section className="history-card">
      <div className="history-card-head">
        <h2>{t("cycleCompare")}</h2>
        <button className="text-button" onClick={() => setTableOpen(!tableOpen)} aria-expanded={tableOpen}>
          {tableOpen ? t("hideTable") : t("showTable")}
        </button>
      </div>
      <p className="history-help">{t("cycleCompareHelp")}</p>
      <Legend
        items={[
          { key: "current", label: t("thisCycleLegend"), color: "var(--text-secondary)" },
          { key: "past", label: t("pastCycles"), color: "var(--chart-muted)" },
        ]}
        extra={[{ key: "pace", label: t("pace"), dashed: true }]}
      />
      <div className="cycle-grid">
        {groups.map(({ account, cycles }) => {
          const current = cycles.find((cycle) => cycle.current);
          const past = cycles.filter((cycle) => !cycle.current).reverse(); // 최근 것부터
          const lines: ChartLine[] = [
            ...past
              .map((cycle, index) => ({
                id: cycle.start,
                label: cycleName(index + 1),
                color: "",
                tone: "muted" as const,
                // 최근 주기일수록 진하게
                strength: Math.max(0.25, 0.75 - index * 0.12),
                hideInTooltip: index >= TOOLTIP_PAST_CYCLES,
                points: cycleLine(cycle, show),
              }))
              .reverse(),
            pace,
            ...(current
              ? [{ id: current.start, label: t("thisCycle"), color: colorOf(account), points: cycleLine(current, show) }]
              : []),
          ];
          return (
            <div className="cycle-panel" key={account.id}>
              <div className="cycle-panel-head">
                <span className="line-key" style={{ borderColor: colorOf(account) }} />
                <span className="cycle-panel-name">{account.name}</span>
                {current && (
                  <span className="cycle-panel-range">
                    {`${shortDay.format(new Date(current.start))} ~ ${shortDay.format(new Date(current.end))}`}
                  </span>
                )}
              </div>
              <LineChart
                lines={lines}
                xDomain={[0, WEEK_HOURS]}
                xTicks={() => cycleTicks((day) => t("dayN", { n: day }))}
                height={190}
                formatX={(hours) =>
                  t("elapsed", { d: Math.floor(hours / 24), h: Math.floor(hours % 24) })
                }
                ariaLabel={`${t("cycleCompare")} · ${account.name}`}
              />
            </div>
          );
        })}
      </div>
      {tableOpen && <CyclesTable groups={groups} show={show} />}
    </section>
  );
}

/** 지난 주기 이름: 1 → "지난주", 2 → "2주 전" */
function cycleName(weeksAgo: number): string {
  return weeksAgo === 1 ? t("lastWeek") : t("weeksAgo", { n: weeksAgo });
}

function cycleLine(cycle: HistoryCycle, show: (percent: number) => number) {
  return cycle.points.map((point) => ({ x: point.hours, y: show(point.percent) }));
}

/** 표로 보기: 주기마다 기간과 끝 값(진행 중이면 지금까지) */
function CyclesTable({
  groups,
  show,
}: {
  groups: { account: HistoryAccount; cycles: HistoryCycle[] }[];
  show: (percent: number) => number;
}) {
  return (
    <div className="table-wrap">
      <table className="data-table">
        <thead>
          <tr>
            <th>{t("account")}</th>
            <th>{t("period")}</th>
            <th>{t("finalValue")}</th>
          </tr>
        </thead>
        <tbody>
          {groups.flatMap(({ account, cycles }) =>
            [...cycles].reverse().map((cycle) => {
              const last = cycle.points[cycle.points.length - 1];
              return (
                <tr key={`${account.id}-${cycle.start}`}>
                  <td>{account.name}</td>
                  <td>
                    {`${formatAsOf(cycle.start, Date.now())} ~ ${formatAsOf(cycle.end, Date.now())}`}
                    {!cycle.startKnown && <span className="muted">{` (${t("estimated")})`}</span>}
                  </td>
                  <td>
                    {last ? `${Math.round(show(last.percent))}%` : "–"}
                    {cycle.current && <span className="muted">{` (${t("soFar")})`}</span>}
                  </td>
                </tr>
              );
            }),
          )}
        </tbody>
      </table>
    </div>
  );
}

interface LegendItem {
  key: string;
  label: string;
  color?: string;
  dashed?: boolean;
  dot?: boolean;
}

/** 범례: 선 모양 열쇠 + 이름 (글자는 글자색) */
function Legend({ items, extra = [] }: { items: LegendItem[]; extra?: LegendItem[] }) {
  return (
    <div className="legend">
      {[...items, ...extra].map((item) => (
        <span className="legend-item" key={item.key}>
          {item.dot ? (
            <span className="legend-dot" />
          ) : (
            <span
              className={`line-key${item.dashed ? " dashed" : ""}`}
              style={{ borderColor: item.color ?? "var(--chart-muted)" }}
            />
          )}
          {item.label}
        </span>
      ))}
    </div>
  );
}
