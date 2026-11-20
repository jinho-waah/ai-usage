// Rust(src-tauri/src/model.rs, settings.rs)에서 보내는 데이터의 모양.
// Rust 쪽 구조체를 바꾸면 여기도 같이 바꿔야 한다.

export type Provider = "claude" | "codex";
/** 메뉴바 라벨: 계정 이름 그대로 / 이모지 / 색 점 */
export type LabelKind = "name" | "emoji" | "dot";

export interface AccountLabel {
  kind: LabelKind;
  /** 이모지 라벨일 때의 이모지. 이름·색 점이면 빈 문자열 */
  text: string;
}

export interface UsageWindow {
  /** 사용한 비율 0~100 */
  usedPercent: number;
  /** 리셋 시각 (ISO 8601, UTC). 이번 창을 아직 안 썼으면 null */
  resetsAt: string | null;
  windowSeconds: number;
  /** 창이 시작된 뒤 지난 시간 비율 0~100 (시간 눈금 위치). 리셋 시각을 모르면 null */
  elapsedPercent: number | null;
  /** 사용량이 시간 진행보다 5%p 이상 앞섬 → 빨강 */
  aheadOfPace: boolean;
}

export interface ModelUsage {
  name: string;
  usedPercent: number;
}

export interface ExtraUsage {
  used: number;
  limit: number | null;
  currency: string;
}

export interface TrendPoint {
  at: string;
  percent: number;
}

export interface AccountUsage {
  id: string;
  provider: Provider;
  name: string;
  label: AccountLabel;
  /** "#RRGGBB" */
  color: string;
  plan: string | null;
  weekly: UsageWindow | null;
  session: UsageWindow | null;
  models: ModelUsage[];
  extra: ExtraUsage | null;
  /** 이번 달 Claude API 비용 (이 계정에 Admin API 키가 있을 때만) */
  apiCost: MonthCost | null;
  /** 이번 주기(리셋까지)의 누적 사용률 */
  trend: TrendPoint[];
  /** 그래프 가로축: 이번 주기 시작 ~ 리셋 시각. 모르면 null (그때 그래프는 최근 7일) */
  trendStart: string | null;
  trendEnd: string | null;
  status: AccountStatus;
  /** 마지막으로 사용량을 받아 온 시각. 아직 못 받았으면 null */
  updatedAt: string | null;
}

/** 계정 상태: 정상, 불러오는 중, 로그인 필요, Cloudflare 확인 필요, 오류 */
export type AccountStatus =
  | { kind: "ok" }
  | { kind: "loading" }
  | { kind: "needsLogin" }
  | { kind: "challenge" }
  | { kind: "error"; message: string };

/** 설정 화면에서 다루는 계정 정보 (Rust accounts.rs 의 Account) */
export interface Account {
  id: string;
  provider: Provider;
  name: string;
  label: AccountLabel;
  color: string;
  orgId: string | null;
  orgName: string | null;
  plan: string | null;
}

/** 계정에서 사용자가 고칠 수 있는 항목 */
export interface AccountEdit {
  id: string;
  name: string;
  label: AccountLabel;
  color: string;
}

/** 원본 Claude Usage 앱에서 가져올 수 있는 프로필 */
export interface LegacyProfile {
  id: string;
  name: string;
  weeklyCount: number;
  sessionCount: number;
  firstAt: string | null;
  lastAt: string | null;
}

export type ServiceState = "operational" | "degraded" | "outage" | "unknown";

export interface ServiceStatus {
  name: string;
  state: ServiceState;
}

/** 브랜드(Claude 전체, Codex 전체)별 누적 토큰. 이 Mac 의 Claude Code·Codex 로그 기준 */
export interface BrandTokens {
  provider: Provider;
  /** 캐시를 포함한 합계 */
  total: number;
  /** 캐시를 뺀 입력 */
  input: number;
  output: number;
  cacheWrite: number;
  cacheRead: number;
  /** 추론 토큰 (Codex, 출력에 포함된 값) */
  reasoning: number;
  /** 오늘 합계 */
  today: number;
  /** 기록이 시작된 날 "YYYY-MM-DD" */
  since: string | null;
}

/** 이번 달 Claude API 비용 (Admin API) */
export interface MonthCost {
  usd: number;
  /** 이 시각까지의 사용분만 반영됨 (보통 몇 시간 늦음). null: 아직 집계 전 */
  refreshedAt: string | null;
}

/** 계정 편집 화면의 Claude API 칸 */
export interface ApiCostInfo {
  configured: boolean;
  /** 가린 키 ("sk-ant-admin…abcd") */
  keyHint: string | null;
  error: string | null;
  cost: MonthCost | null;
}

export interface Summary {
  tokens: BrandTokens[];
  /** 로그를 집계하는 중 */
  tokensScanning: boolean;
  services: ServiceStatus[];
}

export interface Snapshot {
  accounts: AccountUsage[];
  summary: Summary;
  updatedAt: string;
}

// ───────── 히스토리 창 ─────────

/** 기간: 이번 주기, 7일, 30일, 전체 */
export type HistoryRange = "cycle" | "7d" | "30d" | "all";

export interface HistoryAccount {
  id: string;
  provider: Provider;
  name: string;
  label: AccountLabel;
  color: string;
}

/** 계정 하나의 사용률 추이 (사용한 양 %) */
export interface HistorySeries {
  accountId: string;
  points: TrendPoint[];
  /** 기간 안의 리셋 시각 */
  resets: string[];
}

/** 주기 시작 후 지난 시간과 사용률 */
export interface CyclePoint {
  hours: number;
  percent: number;
}

/** 주기 하나의 소진 곡선 */
export interface HistoryCycle {
  accountId: string;
  start: string;
  end: string;
  current: boolean;
  /** 시작 시각이 정확한 리셋 시각인지 (아니면 추정) */
  startKnown: boolean;
  points: CyclePoint[];
}

export interface HistoryData {
  accounts: HistoryAccount[];
  rangeStart: string;
  rangeEnd: string;
  weekly: HistorySeries[];
  session: HistorySeries[];
  cycles: HistoryCycle[];
}

/** 로그인할 때 자동으로 열기 */
export interface LoginItemState {
  /** 설치한 앱(.app)이라 켤 수 있는지 (개발 모드면 false) */
  supported: boolean;
  enabled: boolean;
  /** 시스템 설정에서 허용이 필요함 */
  needsApproval: boolean;
}

export type MenubarStyle = "bars" | "numbers";
export type PercentMode = "used" | "remaining";

export interface Settings {
  menubarStyle: MenubarStyle;
  percentMode: PercentMode;
}
