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
    labelTextHelp: "메뉴바에 이 글자가 나와요 (4글자까지, 이모지도 돼요)",
    emoji: "글자",
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
    forecastRunsOut: "이 속도면 {time}쯤 소진 예정",
    todayAt: "오늘 {time}",
    tomorrowAt: "내일 {time}",
    weekdayAt: "{day} {time}",
    limitAlert: "한도 알림",
    limitAlertOff: "끔",
    limitAlertUsed: "{n}% 사용",
    limitAlertLeft: "{n}% 남음",
    limitAlertHelp: "주간·5시간 한도가 이만큼 되면 알려 줘요. 주기마다 한 번만 알리고, 그 한도가 리셋되면 다시 쓸 수 있다고 알려 줘요.",
    apiEquivMonth: "API 환산 (이번 달)",
    apiEquivTotal: "API 환산 (누적)",
    approxUsd: "약 {amount}",
    apiEquivNote: "구독 대신 API 가격으로 냈다면 들었을 금액의 추정이에요.",
    apiEquivUnpriced: "가격을 모르는 모델의 {count} 토큰은 뺐어요.",
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
    historyTitle: "사용 기록",
    startup: "시작",
    launchAtLogin: "로그인할 때 자동으로 열기",
    launchAtLoginHelp: "Mac을 켜고 로그인하면 메뉴바에 바로 나타나요. 시스템 설정 > 일반 > 로그인 항목에서도 끌 수 있어요.",
    launchAtLoginDev: "설치한 앱에서만 켤 수 있어요 (지금은 개발 모드).",
    launchAtLoginApproval: "시스템 설정 > 일반 > 로그인 항목에서 AI Usage를 허용해 주세요.",
    rangeCycle: "이번 주기",
    range7d: "7일",
    range30d: "30일",
    rangeAll: "전체",
    graphBasis: "그래프는 쓴 양(누적) 기준",
    weeklyUsage: "주간 사용률 추이",
    sessionUsage: "5시간 사용률",
    cycleCompare: "주기별 누적 (리셋까지)",
    resetTick: "리셋",
    cycleCompareHelp: "주기가 시작된 뒤 리셋까지 얼마나 쌓였는지 봐요. 오른쪽 끝이 리셋이고, 지난 주기는 회색으로 겹쳐 그려요. 점선은 일정한 속도로 써서 리셋 때 딱 다 쓰는 경우예요.",
    thisCycle: "이번 주기",
    thisCycleLegend: "이번 주기 (계정 색)",
    pastCycles: "지난 주기",
    lastWeek: "지난주",
    weeksAgo: "{n}주 전",
    pace: "일정한 속도",
    resetMarker: "리셋",
    showTable: "표로 보기",
    hideTable: "표 닫기",
    noHistory: "아직 기록이 없어요. 앱이 켜져 있으면 사용률이 바뀔 때마다 남아요.",
    loadFailed: "기록을 불러오지 못했어요",
    dayN: "{n}일",
    elapsed: "{d}일 {h}시간째",
    account: "계정",
    date: "날짜",
    period: "기간",
    finalValue: "끝 값",
    soFar: "지금까지",
    estimated: "추정",
    quit: "종료",
    menubarStyle: "메뉴바 표시",
    bars: "막대",
    numbers: "숫자",
    menubarStyleHelp: "막대 위의 세로 눈금은 주간 시간을 나타내요. 이 속도면 리셋 전에 다 쓸 것 같으면 막대가 빨강으로 바뀌어요.",
    percentMode: "표시 기준",
    used: "사용한 양",
    remaining: "남은 양",
    percentModeHelpRemaining: "숫자와 막대가 남은 양을 보여 주고, 눈금은 남은 시간이에요. 남은 양이 남은 시간보다 5%p 이상 적으면 빨강이에요.",
    percentModeHelpUsed: "숫자와 막대가 사용한 양을 보여 주고, 눈금은 지난 시간이에요. 사용량이 지난 시간보다 5%p 이상 많으면 빨강이에요.",
    apiCostTitle: "API 비용",
    apiCostHelp: "이 계정의 Claude Console 조직에서 만든 Admin API 키(sk-ant-admin…)를 넣으면 이번 달 API 비용을 카드에 보여 줘요. 키는 이 Mac의 키체인에만 저장돼요.",
    apiKeyPlaceholder: "sk-ant-admin…",
    apiKeyConnected: "연결됨 · 비용을 불러오는 중",
    removeKey: "키 삭제",
    checking: "확인 중…",
    apiCostAsOf: "{time} 기준이에요. 보통 몇 시간 늦게 반영돼요.",
    apiCostNoData: "아직 집계된 사용량이 없어요.",
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
    labelTextHelp: "These characters show in the menu bar (up to 4, emoji work too)",
    emoji: "Text",
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
    forecastRunsOut: "At this pace it runs out around {time}",
    todayAt: "{time} today",
    tomorrowAt: "{time} tomorrow",
    weekdayAt: "{time} on {day}",
    limitAlert: "Limit alerts",
    limitAlertOff: "Off",
    limitAlertUsed: "{n}% used",
    limitAlertLeft: "{n}% left",
    limitAlertHelp: "Notifies once per cycle when a weekly or 5-hour limit gets here, and again when that limit resets.",
    apiEquivMonth: "API equivalent (this month)",
    apiEquivTotal: "API equivalent (all time)",
    approxUsd: "about {amount}",
    apiEquivNote: "An estimate of what this usage would have cost at API prices instead of a subscription.",
    apiEquivUnpriced: "{count} tokens from models without a known price are left out.",
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
    historyTitle: "Usage history",
    startup: "Startup",
    launchAtLogin: "Open at login",
    launchAtLoginHelp: "Shows up in the menu bar right after you log in. You can also turn it off in System Settings > General > Login Items.",
    launchAtLoginDev: "Available in the installed app only (this is a development build).",
    launchAtLoginApproval: "Allow AI Usage in System Settings > General > Login Items.",
    rangeCycle: "This cycle",
    range7d: "7 days",
    range30d: "30 days",
    rangeAll: "All",
    graphBasis: "Graphs show amount used (cumulative)",
    weeklyUsage: "Weekly usage over time",
    sessionUsage: "5-hour usage",
    cycleCompare: "Per cycle, until reset",
    resetTick: "Reset",
    cycleCompareHelp: "How much has piled up since the cycle started, up to the reset at the right edge. Past cycles are drawn in gray; the dashed line is an even pace that runs out exactly at reset.",
    thisCycle: "This cycle",
    thisCycleLegend: "This cycle (account color)",
    pastCycles: "Past cycles",
    lastWeek: "Last week",
    weeksAgo: "{n} weeks ago",
    pace: "Even pace",
    resetMarker: "Reset",
    showTable: "Show table",
    hideTable: "Hide table",
    noHistory: "No history yet. Usage is recorded whenever it changes while the app is running.",
    loadFailed: "Couldn't load history",
    dayN: "Day {n}",
    elapsed: "Day {d}, hour {h}",
    account: "Account",
    date: "Date",
    period: "Period",
    finalValue: "End value",
    soFar: "so far",
    estimated: "estimated",
    quit: "Quit",
    menubarStyle: "Menu bar style",
    bars: "Bars",
    numbers: "Numbers",
    menubarStyleHelp: "The tick on each bar marks the week's time. A bar turns red when you are on pace to run out before the reset.",
    percentMode: "Show",
    used: "Used",
    remaining: "Remaining",
    percentModeHelpRemaining: "Numbers and bars show what is left; the tick is the time left. Red when what is left trails the time left by 5 points.",
    percentModeHelpUsed: "Numbers and bars show what is used; the tick is the time passed. Red when usage runs 5 points ahead of the time passed.",
    apiCostTitle: "API cost",
    apiCostHelp: "Enter an Admin API key (sk-ant-admin…) from this account's Claude Console organization to show this month's API cost on its card. The key is stored only in this Mac's Keychain.",
    apiKeyPlaceholder: "sk-ant-admin…",
    apiKeyConnected: "Connected · loading cost",
    removeKey: "Remove key",
    checking: "Checking…",
    apiCostAsOf: "As of {time}. Usually a few hours behind.",
    apiCostNoData: "No usage counted yet.",
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

/** 지난 시각: 오늘이면 시각만, 아니면 날짜와 시각 */
export function formatAsOf(iso: string, now: number): string {
  const at = new Date(iso);
  return at.toDateString() === new Date(now).toDateString() ? sameDayTime.format(at) : otherDayTime.format(at);
}

/**
 * 남은 시간. 1시간 이상은 시간 단위 소수 한 자리(2시간 40분 → "2.7시간"),
 * 1시간 미만은 분(24분 → "24분"). 분은 올려서, 시간이 남아 있는 동안 "0분"이 되지 않게 한다.
 */
export function formatHours(milliseconds: number): string {
  const minutes = Math.ceil(Math.max(0, milliseconds) / 60_000);
  if (minutes < 60) return t("minutes", { n: minutes });
  return t("hoursDecimal", { n: (minutes / 60).toFixed(1) });
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

const usdWhole = new Intl.NumberFormat("en-US", { style: "currency", currency: "USD", maximumFractionDigits: 0 });
/** 어림 금액 (예: "약 $1,284") */
export function formatApproxUsd(amount: number): string {
  return t("approxUsd", { amount: usdWhole.format(amount) });
}

const hourOnly = new Intl.DateTimeFormat(locale, { hour: "numeric" });
const weekdayLong = new Intl.DateTimeFormat(locale, { weekday: "long" });
/**
 * 어림 시각. 추정값이라 분까지 적지 않고 가까운 정시로 맞추고,
 * 날짜는 "오늘 / 내일 / 요일"로 적는다 (예: "월요일 오전 1시").
 */
export function formatApproxTime(iso: string, now: number): string {
  const at = new Date(iso);
  at.setMinutes(at.getMinutes() >= 30 ? 60 : 0, 0, 0);
  const startOfDay = (date: Date) => new Date(date.getFullYear(), date.getMonth(), date.getDate()).getTime();
  const days = Math.round((startOfDay(at) - startOfDay(new Date(now))) / 86_400_000);
  const time = hourOnly.format(at);
  if (days <= 0) return t("todayAt", { time });
  if (days === 1) return t("tomorrowAt", { time });
  if (days < 7) return t("weekdayAt", { day: weekdayLong.format(at), time });
  return `${shortDate.format(at)} ${time}`;
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
