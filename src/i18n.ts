// 화면 문구(한국어·영어)와 날짜·숫자 표기.
// Mac 의 언어 설정이 한국어면 한국어, 아니면 영어로 보여 준다.

export const locale = navigator.language || "en";
const lang: "ko" | "en" = locale.toLowerCase().startsWith("ko") ? "ko" : "en";

const strings = {
  ko: {
    accounts: "계정",
    importTitle: "다른 앱 기록 가져오기",
    importHelp: "원본 Claude Usage 앱에 남은 사용률 기록을 계정 그래프로 가져와요. 어느 계정의 기록인지 골라 주세요.",
    chooseAccount: "계정 선택",
    importButton: "가져오기",
    imported: "{count}개 가져왔어요",
    legacyRecords: "주간 {weekly}개 · 세션 {session}개",
    addClaudeAccount: "Claude 계정 추가",
    addCodex: "Codex 추가",
    codexCli: "Codex CLI 로그인",
    codexNeedsLogin: "Codex CLI 로그인이 필요해요 (터미널에서 codex login)",
    staleAsOf: "{time} 기준",
    loginWindowOpened: "로그인 창이 열렸어요. claude.ai 에 로그인하면 창이 저절로 닫히고 사용량이 보여요.",
    googleLoginHint: "구글 로그인이 막히면 이메일로 로그인해 주세요.",
    emptyHint: "Claude 계정이나 Codex 를 추가하면 사용량이 여기에 보여요.",
    loading: "불러오는 중…",
    needsLogin: "로그인이 필요해요",
    challenge: "claude.ai 확인이 필요해요",
    signIn: "로그인",
    open: "열기",
    notUsedYet: "아직 사용 전이에요",
    noUsageData: "사용량 정보가 없어요",
    staleSince: "{time} 데이터",
    notSignedIn: "로그인 전",
    edit: "편집",
    save: "저장",
    cancel: "취소",
    remove: "삭제",
    confirmRemove: "한 번 더 누르면 삭제",
    relogin: "다시 로그인",
    moveUp: "위로",
    moveDown: "아래로",
    name: "이름",
    menubarLabel: "메뉴바 라벨",
    labelName: "이름",
    labelNameHelp: "메뉴바에 이름이 그대로 나와요 (앞 8글자까지)",
    emoji: "이모지",
    dot: "색 점",
    color: "색",
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
    recentSpan: "최근 {duration}",
    tokensClaude: "Claude 누적",
    tokensCodex: "Codex 누적",
    tokensScanning: "로그에서 토큰을 세는 중…",
    tokenToday: "오늘",
    tokenInput: "입력",
    tokenOutput: "출력",
    tokenCacheWrite: "캐시 쓰기",
    tokenCacheRead: "캐시 읽기",
    tokenReasoning: "추론 (출력에 포함)",
    tokensSince: "{date}부터 이 Mac에서 쓴 양이에요",
    tokensHelp: "Claude Code·Codex 로그를 세서 앱에 따로 저장해요. 로그가 지워져도 합계는 남아요.",
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
    menubarStyle: "메뉴바 표시",
    bars: "막대",
    numbers: "숫자",
    menubarStyleHelp: "막대 위의 세로 눈금은 주간 시간을 나타내요. 이 속도면 리셋 전에 다 쓸 것 같으면 막대가 빨강으로 바뀌어요.",
    percentMode: "표시 기준",
    used: "사용한 양",
    remaining: "남은 양",
    percentModeHelpRemaining: "숫자와 막대가 남은 양을 보여 주고, 눈금은 남은 시간이에요. 남은 양이 남은 시간보다 5%p 이상 적으면 빨강이에요.",
    percentModeHelpUsed: "숫자와 막대가 사용한 양을 보여 주고, 눈금은 지난 시간이에요. 사용량이 지난 시간보다 5%p 이상 많으면 빨강이에요.",
    settingsNote: "API 비용 연동은 다음 단계에서 추가돼요.",
    days: "{n}일",
    hours: "{n}시간",
    hoursDecimal: "{n}시간",
    minutes: "{n}분",
  },
  en: {
    accounts: "Accounts",
    importTitle: "Import from another app",
    importHelp: "Bring usage history from the original Claude Usage app into an account's graph. Pick which account each profile belongs to.",
    chooseAccount: "Choose account",
    importButton: "Import",
    imported: "Imported {count}",
    legacyRecords: "{weekly} weekly · {session} session",
    addClaudeAccount: "Add Claude account",
    addCodex: "Add Codex",
    codexCli: "Codex CLI sign-in",
    codexNeedsLogin: "Codex CLI sign-in required (run codex login)",
    staleAsOf: "As of {time}",
    loginWindowOpened: "A sign-in window opened. Once you sign in to claude.ai it closes by itself and your usage appears.",
    googleLoginHint: "If Google sign-in is blocked, sign in with email instead.",
    emptyHint: "Add a Claude account or Codex to see usage here.",
    loading: "Loading…",
    needsLogin: "Sign-in required",
    challenge: "claude.ai needs a quick check",
    signIn: "Sign in",
    open: "Open",
    notUsedYet: "Not used yet",
    noUsageData: "No usage data",
    staleSince: "Data from {time}",
    notSignedIn: "Not signed in",
    edit: "Edit",
    save: "Save",
    cancel: "Cancel",
    remove: "Remove",
    confirmRemove: "Click again to remove",
    relogin: "Sign in again",
    moveUp: "Move up",
    moveDown: "Move down",
    name: "Name",
    menubarLabel: "Menu bar label",
    labelName: "Name",
    labelNameHelp: "The name shows in the menu bar (first 8 characters)",
    emoji: "Emoji",
    dot: "Dot",
    color: "Color",
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
    recentSpan: "Last {duration}",
    tokensClaude: "Claude total",
    tokensCodex: "Codex total",
    tokensScanning: "Counting tokens from logs…",
    tokenToday: "Today",
    tokenInput: "Input",
    tokenOutput: "Output",
    tokenCacheWrite: "Cache write",
    tokenCacheRead: "Cache read",
    tokenReasoning: "Reasoning (in output)",
    tokensSince: "Used on this Mac since {date}",
    tokensHelp: "Counted from Claude Code and Codex logs and kept by this app, even after the logs are deleted.",
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
    menubarStyle: "Menu bar style",
    bars: "Bars",
    numbers: "Numbers",
    menubarStyleHelp: "The tick on each bar marks the week's time. A bar turns red when you are on pace to run out before the reset.",
    percentMode: "Show",
    used: "Used",
    remaining: "Remaining",
    percentModeHelpRemaining: "Numbers and bars show what is left; the tick is the time left. Red when what is left trails the time left by 5 points.",
    percentModeHelpUsed: "Numbers and bars show what is used; the tick is the time passed. Red when usage runs 5 points ahead of the time passed.",
    settingsNote: "API cost comes in a later milestone.",
    days: "{n}d",
    hours: "{n}h",
    hoursDecimal: "{n}h",
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

/** 남은 시간을 시간 단위로, 분은 소수 한 자리로 (예: 2시간 40분 → "2.7시간") */
export function formatHours(milliseconds: number): string {
  const hours = Math.max(0, milliseconds) / 3_600_000;
  return t("hoursDecimal", { n: hours.toFixed(1) });
}

export function formatPercent(value: number): string {
  return `${Math.round(value)}%`;
}

const compactNumber = new Intl.NumberFormat(locale, { notation: "compact", maximumFractionDigits: 2 });
export function formatTokens(count: number): string {
  return t("tokens", { count: compactNumber.format(count) });
}

/** 단위 없이 짧게 (예: "1.2M", "24.8억") */
export function formatCount(count: number): string {
  return compactNumber.format(count);
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

const clockTime = new Intl.DateTimeFormat(locale, { hour: "numeric", minute: "2-digit" });
/** 오래된 데이터 표시용 시각 (예: "오후 3:40") */
export function formatClock(iso: string): string {
  return clockTime.format(new Date(iso));
}

const shortDate = new Intl.DateTimeFormat(locale, { month: "short", day: "numeric" });
const fullDate = new Intl.DateTimeFormat(locale, { year: "numeric", month: "short", day: "numeric" });
/** 로컬 날짜 "YYYY-MM-DD" 를 "2026년 11월 10일" 처럼 */
export function formatDay(day: string): string {
  return fullDate.format(new Date(`${day}T00:00:00`));
}
/** 기간 표시 (예: "9월 29일 ~ 11월 20일") */
export function formatDateRange(from: string | null, to: string | null): string {
  if (!from || !to) return "";
  return `${shortDate.format(new Date(from))} ~ ${shortDate.format(new Date(to))}`;
}
