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
  trend: TrendPoint[];
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

export interface Summary {
  tokens: BrandTokens[];
  /** 로그를 집계하는 중 */
  tokensScanning: boolean;
  apiMonthCostUsd: number | null;
  services: ServiceStatus[];
}

export interface Snapshot {
  accounts: AccountUsage[];
  summary: Summary;
  updatedAt: string;
}

export type MenubarStyle = "bars" | "numbers";
export type PercentMode = "used" | "remaining";

export interface Settings {
  menubarStyle: MenubarStyle;
  percentMode: PercentMode;
}
