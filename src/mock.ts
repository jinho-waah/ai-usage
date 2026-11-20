// 브라우저 미리보기(`npm run dev`)용 가짜 데이터.
// 실제 앱에서는 쓰지 않는다. 앱 안의 가짜 데이터는 Rust(src-tauri/src/mock.rs)가 만든다.

import type { Account, AccountUsage, Settings, Snapshot, TrendPoint, UsageWindow } from "./types";

const HOUR = 3_600_000;
const WEEK_SECONDS = 7 * 24 * 3600;

function usageWindow(used: number, resetsInHours: number, windowSeconds: number): UsageWindow {
  const now = Date.now();
  const resetsAt = now + resetsInHours * HOUR;
  const start = resetsAt - windowSeconds * 1000;
  const elapsedPercent = Math.min(100, Math.max(0, ((now - start) / (windowSeconds * 1000)) * 100));
  return {
    usedPercent: used,
    resetsAt: new Date(resetsAt).toISOString(),
    windowSeconds,
    elapsedPercent,
    aheadOfPace: used >= elapsedPercent + 5,
  };
}

function trend(window: UsageWindow, previousFrom: number, previousTo: number): TrendPoint[] {
  const now = Date.now();
  const cycleStart = new Date(window.resetsAt ?? Date.now()).getTime() - window.windowSeconds * 1000;
  const points: TrendPoint[] = [];
  for (let i = 0; i <= 168; i++) {
    const at = now - (168 - i) * HOUR;
    const percent =
      at >= cycleStart
        ? (window.usedPercent * (at - cycleStart)) / Math.max(1, now - cycleStart)
        : previousFrom + ((previousTo - previousFrom) * (at - (now - 168 * HOUR))) / Math.max(1, cycleStart - (now - 168 * HOUR));
    points.push({ at: new Date(at).toISOString(), percent });
  }
  return points;
}

export function mockSnapshot(): Snapshot {
  const personalWeekly = usageWindow(58, 74, WEEK_SECONDS);
  const workWeekly = usageWindow(41, 129, WEEK_SECONDS);
  const codexWeekly = usageWindow(55, 28, WEEK_SECONDS);

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
      trend: trend(personalWeekly, 30, 72),
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
      trend: trend(workWeekly, 10, 64),
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
      trend: trend(codexWeekly, 20, 80),
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
        },
      ],
      tokensScanning: false,
      apiMonthCostUsd: 3.1,
      services: [
        { name: "Claude", state: "operational" },
        { name: "OpenAI", state: "operational" },
      ],
    },
    updatedAt: new Date().toISOString(),
  };
}

export function mockSettings(): Settings {
  return { menubarStyle: "bars", percentMode: "remaining" };
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
