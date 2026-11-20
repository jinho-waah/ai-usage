// 브라우저 미리보기(`npm run dev`)용 가짜 데이터.
// 실제 앱에서는 쓰지 않는다. 앱 안의 가짜 데이터는 Rust(src-tauri/src/mock.rs)가 만든다.

import type {
  Account,
  AccountUsage,
  CyclePoint,
  HistoryCycle,
  HistoryData,
  HistoryRange,
  HistorySeries,
  Settings,
  Snapshot,
  TrendPoint,
  UsageWindow,
} from "./types";

const HOUR = 3_600_000;
const WEEK_SECONDS = 7 * 24 * 3600;

function usageWindow(used: number, resetsInHours: number, windowSeconds: number): UsageWindow {
  const now = Date.now();
  const resetsAt = now + resetsInHours * HOUR;
  const start = resetsAt - windowSeconds * 1000;
  const elapsedPercent = Math.min(100, Math.max(0, ((now - start) / (windowSeconds * 1000)) * 100));
  // Rust(model.rs)와 같은 계산: 이 속도면 언제 다 쓰는지, 하루에 얼마씩 쓸 수 있는지
  const runsOut = elapsedPercent >= 3 && used * 0.98 > elapsedPercent && used < 100;
  const daysLeft = resetsInHours / 24;
  return {
    usedPercent: used,
    resetsAt: new Date(resetsAt).toISOString(),
    windowSeconds,
    elapsedPercent,
    aheadOfPace: used >= elapsedPercent + 5,
    runsOutAt: runsOut ? new Date(start + ((now - start) * 100) / used).toISOString() : null,
    dailyBudgetPercent: daysLeft >= 1 ? (100 - used) / daysLeft : null,
    usedTodayPercent: windowSeconds === WEEK_SECONDS ? Math.round(used * 0.15) : null,
  };
}

/**
 * 최근 7일 동안 한 시간마다 쓴 양 (낮에 더 많이 쓴다고 가정).
 * 지난 주기는 `previousEnd` % 까지 쌓이고, 리셋 때 0% 로 떨어진 뒤 지금 사용률까지 다시 쌓인다.
 */
function recentTrend(
  window: UsageWindow,
  previousEnd: number,
): { trend: TrendPoint[]; trendStart: string | null; trendEnd: string | null } {
  const now = Date.now();
  const weekAgo = now - 7 * 24 * HOUR;
  const cycleStart = new Date(window.resetsAt ?? now).getTime() - window.windowSeconds * 1000;
  const activity = (at: number) => {
    const hour = new Date(at).getHours();
    return hour < 9 ? 0.1 : hour >= 19 ? 1.6 : 1;
  };
  const hours: number[] = [];
  for (let at = weekAgo; at <= now; at += HOUR) hours.push(at);
  const previous = hours.filter((at) => at < cycleStart);
  const current = hours.filter((at) => at >= cycleStart);
  const previousTotal = previous.reduce((sum, at) => sum + activity(at), 0) || 1;
  const currentTotal = current.reduce((sum, at) => sum + activity(at), 0) || 1;

  const trend: TrendPoint[] = [];
  let sum = 0;
  for (const at of previous) {
    sum += activity(at);
    trend.push({ at: new Date(at).toISOString(), percent: Math.round(previousEnd * (0.5 + (0.5 * sum) / previousTotal)) });
  }
  if (cycleStart > weekAgo) trend.push({ at: new Date(cycleStart).toISOString(), percent: 0 });
  sum = 0;
  for (const at of current) {
    sum += activity(at);
    trend.push({ at: new Date(at).toISOString(), percent: Math.round((window.usedPercent * sum) / currentTotal) });
  }
  trend.push({ at: new Date(now).toISOString(), percent: window.usedPercent });
  return { trend, trendStart: new Date(weekAgo).toISOString(), trendEnd: new Date(now).toISOString() };
}

export function mockSnapshot(): Snapshot {
  const personalWeekly = usageWindow(58, 74, WEEK_SECONDS);
  const workWeekly = usageWindow(41, 129, WEEK_SECONDS);
  // 리셋까지 하루가 안 남은 경우도 미리보기에서 보이게 18시간으로 둔다
  const codexWeekly = usageWindow(55, 18, WEEK_SECONDS);

  const accounts: AccountUsage[] = [
    {
      id: "mock-personal",
      provider: "claude",
      name: "개인",
      label: { kind: "name", text: "" },
      color: "#D97757",
      plan: "Ultra",
      weekly: personalWeekly,
      session: null,
      models: [
        { name: "Opus", usedPercent: 12 },
        { name: "Sonnet", usedPercent: 31 },
      ],
      extra: { used: 4.2, limit: 50, currency: "USD" },
      apiCost: { usd: 3.1, refreshedAt: new Date(Date.now() - 3 * 3_600_000).toISOString() },
      ...recentTrend(personalWeekly, 72),
      status: { kind: "ok" },
      updatedAt: new Date().toISOString(),
    },
    {
      id: "mock-work",
      provider: "claude",
      name: "회사",
      label: { kind: "emoji", text: "💼" },
      color: "#6A9BCC",
      plan: "Pro",
      weekly: workWeekly,
      session: usageWindow(12, 2.67, 5 * 3600),
      models: [],
      extra: null,
      apiCost: null,
      ...recentTrend(workWeekly, 100),
      status: { kind: "ok" },
      updatedAt: new Date().toISOString(),
    },
    {
      id: "mock-codex",
      provider: "codex",
      name: "Codex",
      label: { kind: "dot", text: "" },
      color: "#10A37F",
      plan: "Pro",
      weekly: codexWeekly,
      session: null,
      models: [],
      extra: null,
      apiCost: null,
      ...recentTrend(codexWeekly, 80),
      status: { kind: "needsLogin" },
      updatedAt: new Date(Date.now() - 42 * 60_000).toISOString(),
    },
  ];

  return {
    accounts,
    summary: {
      tokens: [
        {
          provider: "claude",
          total: 2_481_300_000,
          input: 3_100_000,
          output: 21_400_000,
          cacheWrite: 96_800_000,
          cacheRead: 2_360_000_000,
          reasoning: 0,
          today: 61_200_000,
          since: "2026-10-21",
          apiCostTotalUsd: 1284,
          apiCostMonthUsd: 912.5,
          unpricedTokens: 0,
        },
        {
          provider: "codex",
          total: 412_700_000,
          input: 58_300_000,
          output: 4_100_000,
          cacheWrite: 0,
          cacheRead: 350_300_000,
          reasoning: 1_900_000,
          today: 9_800_000,
          since: "2026-11-10",
          apiCostTotalUsd: 96.4,
          apiCostMonthUsd: 96.4,
          unpricedTokens: 0,
        },
      ],
      tokensScanning: false,
      services: [
        { name: "Claude", state: "operational" },
        { name: "OpenAI", state: "operational" },
      ],
    },
    updatedAt: new Date().toISOString(),
  };
}

export function mockSettings(): Settings {
  return { menubarStyle: "bars", percentMode: "remaining", limitAlertPercent: 90 };
}

export function mockAccounts(): Account[] {
  return mockSnapshot().accounts.map((account) => ({
    id: account.id,
    provider: account.provider,
    name: account.name,
    label: account.label,
    color: account.color,
    orgId: account.status.kind === "needsLogin" ? null : "mock-org",
    orgName: account.provider === "claude" ? `${account.name}@example.com's Organization` : null,
    plan: account.plan,
  }));
}

// ───────── 히스토리 창 미리보기 ─────────

const DAY = 24 * HOUR;
const WEEK = 7 * DAY;

/** 매번 같은 값이 나오는 간단한 난수 (미리보기 그림이 새로 고칠 때마다 바뀌지 않게) */
function seeded(seed: number) {
  let state = seed;
  return () => {
    state = (state * 1_103_515_245 + 12_345) % 2_147_483_648;
    return state / 2_147_483_648;
  };
}

/** 계정 하나의 주간 기록: 6주치. 낮 시간에만 조금씩 오르고, 매주 같은 시각에 0% 로 리셋. */
function mockWeekly(seed: number, nextResetInHours: number, now: number) {
  const random = seeded(seed);
  const nextReset = now + nextResetInHours * HOUR;
  const cycles: { start: number; end: number; points: { at: number; percent: number }[] }[] = [];
  for (let k = 5; k >= 0; k--) {
    const end = nextReset - k * WEEK;
    const start = end - WEEK;
    const target = 35 + random() * 60;
    const points = [{ at: start, percent: 0 }];
    let percent = 0;
    for (let at = start + HOUR; at < Math.min(end, now); at += HOUR) {
      const hour = new Date(at).getHours();
      if (hour >= 10 && hour <= 23 && random() < 0.45) {
        percent = Math.min(100, percent + (target / 70) * (0.5 + random()));
        points.push({ at, percent: Math.round(percent) });
      }
    }
    cycles.push({ start, end, points });
  }
  return cycles;
}

export function mockHistory(range: HistoryRange): HistoryData {
  const now = Date.now();
  const accounts = mockAccounts().map(({ id, provider, name, label, color }) => ({ id, provider, name, label, color }));
  const generated = accounts.map((account, index) => ({
    account,
    cycles: mockWeekly(index + 7, 30 + index * 41, now),
  }));

  const currentStarts = generated.map(({ cycles }) => cycles[cycles.length - 1].start);
  const rangeStart =
    range === "cycle"
      ? Math.min(...currentStarts)
      : range === "7d"
        ? now - WEEK
        : range === "30d"
          ? now - 30 * DAY
          : Math.min(...generated.map(({ cycles }) => cycles[0].points[1]?.at ?? cycles[0].start));

  const weekly: HistorySeries[] = generated.map(({ account, cycles }) => {
    const all = cycles.flatMap((cycle) => cycle.points);
    const before = all.filter((point) => point.at < rangeStart).pop();
    const inside = all.filter((point) => point.at >= rangeStart && point.at <= now);
    const points: TrendPoint[] = [
      ...(before ? [{ at: new Date(rangeStart).toISOString(), percent: before.percent }] : []),
      ...inside.map((point) => ({ at: new Date(point.at).toISOString(), percent: point.percent })),
    ];
    const last = points[points.length - 1];
    if (last) points.push({ at: new Date(now).toISOString(), percent: last.percent });
    const resets = cycles
      .slice(1)
      .map((cycle) => cycle.start)
      .filter((at) => at >= rangeStart && at <= now)
      .map((at) => new Date(at).toISOString());
    return { accountId: account.id, points, resets };
  });

  const cycles: HistoryCycle[] = generated.flatMap(({ account, cycles }) =>
    cycles
      .filter((cycle) => cycle.end > rangeStart && cycle.start < now)
      .map((cycle) => {
        const points: CyclePoint[] = cycle.points.map((point) => ({
          hours: (point.at - cycle.start) / HOUR,
          percent: point.percent,
        }));
        const endHours = (Math.min(cycle.end, now) - cycle.start) / HOUR;
        const last = points[points.length - 1];
        if (last && endHours > last.hours) points.push({ hours: endHours, percent: last.percent });
        return {
          accountId: account.id,
          start: new Date(cycle.start).toISOString(),
          end: new Date(cycle.end).toISOString(),
          current: cycle.end > now,
          startKnown: true,
          points,
        };
      }),
  );

  // 5시간 한도가 있는 계정(회사)만
  const sessionAccount = accounts[1];
  const random = seeded(99);
  const sessionPoints: TrendPoint[] = [];
  for (let at = Math.max(rangeStart, now - 3 * DAY); at <= now; at += HOUR) {
    const inWindow = Math.floor((at / HOUR) % 5);
    const percent = inWindow === 0 ? 0 : Math.min(100, Math.round(inWindow * 18 * (0.6 + random())));
    sessionPoints.push({ at: new Date(at).toISOString(), percent });
  }

  return {
    accounts,
    rangeStart: new Date(rangeStart).toISOString(),
    rangeEnd: new Date(now).toISOString(),
    weekly,
    session: sessionAccount ? [{ accountId: sessionAccount.id, points: sessionPoints, resets: [] }] : [],
    cycles,
  };
}
