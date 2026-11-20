// 화면 문구(한국어·영어)와 날짜·숫자 표기.
// Mac 의 언어 설정이 한국어면 한국어, 아니면 영어로 보여 준다.

export const locale = navigator.language || "en";
const lang: "ko" | "en" = locale.toLowerCase().startsWith("ko") ? "ko" : "en";

const strings = {
  ko: {
    refresh: "새로고침",
    settings: "설정",
    back: "뒤로",
    weekly: "주간",
    session: "세션",
    resets: "리셋 {time}",
    left: "{duration} 남음",
    aheadOfPace: "빠르게 쓰는 중",
    models: "모델별 한도",
    extraUsage: "Extra usage",
    recentTrend: "최근 7일",
    claudeCodeToday: "Claude Code 오늘",
    tokens: "{count} 토큰",
    apiThisMonth: "API 이번 달",
    services: "서비스 상태",
    operational: "정상",
    degraded: "느림",
    outage: "장애",
    unknown: "알 수 없음",
    updatedJustNow: "방금 갱신",
    updatedMinutesAgo: "{minutes}분 전 갱신",
    history: "히스토리",
    quit: "종료",
    comingLater: "다음 단계에서 추가돼요",
    noAccounts: "아직 등록된 계정이 없어요",
    menubarStyle: "메뉴바 표시",
    bars: "막대",
    numbers: "숫자",
    menubarStyleHelp:
      "막대는 주간 사용량을, 세로 눈금은 주간 시간이 얼마나 지났는지를 보여 줘요. 사용량이 눈금보다 5%p 이상 앞서면 빨강으로 바뀌어요.",
    percentMode: "퍼센트 기준",
    used: "사용한 %",
    remaining: "남은 %",
    percentModeHelp: "숫자로 보여 줄 때의 기준이에요. 막대는 항상 사용량만큼 채워져요.",
    settingsNote: "계정 추가와 API 키 설정은 다음 단계에서 추가돼요.",
    days: "{n}일",
    hours: "{n}시간",
    minutes: "{n}분",
  },
  en: {
    refresh: "Refresh",
    settings: "Settings",
    back: "Back",
    weekly: "Weekly",
    session: "Session",
    resets: "Resets {time}",
    left: "{duration} left",
    aheadOfPace: "Ahead of pace",
    models: "Model limits",
    extraUsage: "Extra usage",
    recentTrend: "Last 7 days",
    claudeCodeToday: "Claude Code today",
    tokens: "{count} tokens",
    apiThisMonth: "API this month",
    services: "Service status",
    operational: "OK",
    degraded: "Degraded",
    outage: "Outage",
    unknown: "Unknown",
    updatedJustNow: "Updated just now",
    updatedMinutesAgo: "Updated {minutes}m ago",
    history: "History",
    quit: "Quit",
    comingLater: "Coming in a later milestone",
    noAccounts: "No accounts yet",
    menubarStyle: "Menu bar style",
    bars: "Bars",
    numbers: "Numbers",
    menubarStyleHelp:
      "Bars show weekly usage; the tick shows how much of the week has passed. A bar turns red when usage runs 5 points ahead of the tick.",
    percentMode: "Percentage",
    used: "Used",
    remaining: "Remaining",
    percentModeHelp: "Applies when numbers are shown. Bars always fill with usage.",
    settingsNote: "Adding accounts and API keys comes in a later milestone.",
    days: "{n}d",
    hours: "{n}h",
    minutes: "{n}m",
  },
} as const;

export type StringKey = keyof (typeof strings)["ko"];

/** 문구를 가져온다. `{이름}` 자리는 values 로 채운다. */
export function t(key: StringKey, values: Record<string, string | number> = {}): string {
  let text: string = strings[lang][key];
  for (const [name, value] of Object.entries(values)) {
    text = text.replace(`{${name}}`, String(value));
  }
  return text;
}

const sameDayTime = new Intl.DateTimeFormat(locale, { hour: "numeric", minute: "2-digit" });
const otherDayTime = new Intl.DateTimeFormat(locale, {
  month: "short",
  day: "numeric",
  weekday: "short",
  hour: "numeric",
  minute: "2-digit",
});

/** 리셋 시각: 24시간 안이면 시각만, 아니면 날짜와 요일까지. */
export function formatResetTime(iso: string, now: number): string {
  const at = new Date(iso);
  return at.getTime() - now < 24 * 3_600_000 ? sameDayTime.format(at) : otherDayTime.format(at);
}

/** 남은 시간: "3일 2시간", "2시간 40분", "15분" */
export function formatDuration(milliseconds: number): string {
  const totalMinutes = Math.max(0, Math.round(milliseconds / 60_000));
  const days = Math.floor(totalMinutes / 1440);
  const hours = Math.floor((totalMinutes % 1440) / 60);
  const minutes = totalMinutes % 60;
  if (days > 0) return [t("days", { n: days }), hours > 0 ? t("hours", { n: hours }) : ""].join(" ").trim();
  if (hours > 0) return [t("hours", { n: hours }), minutes > 0 ? t("minutes", { n: minutes }) : ""].join(" ").trim();
  return t("minutes", { n: minutes });
}

export function formatPercent(value: number): string {
  return `${Math.round(value)}%`;
}

const compactNumber = new Intl.NumberFormat(locale, { notation: "compact", maximumFractionDigits: 2 });
export function formatTokens(count: number): string {
  return t("tokens", { count: compactNumber.format(count) });
}

const usd = new Intl.NumberFormat("en-US", { style: "currency", currency: "USD" });
export function formatUsd(amount: number): string {
  return usd.format(amount);
}

export function formatMoney(amount: number, currency: string): string {
  return new Intl.NumberFormat("en-US", { style: "currency", currency }).format(amount);
}

export function formatUpdated(iso: string, now: number): string {
  const minutes = Math.floor((now - new Date(iso).getTime()) / 60_000);
  return minutes < 1 ? t("updatedJustNow") : t("updatedMinutesAgo", { minutes });
}
