// Rust(src-tauri/src/model.rs, settings.rs)에서 보내는 데이터의 모양.
// Rust 쪽 구조체를 바꾸면 여기도 같이 바꿔야 한다.

export type Provider = "claude" | "codex";
export type LabelKind = "alias" | "emoji" | "dot";

export interface AccountLabel {
  kind: LabelKind;
  /** 별칭이나 이모지. 색 점이면 빈 문자열 */
  text: string;
}

export interface UsageWindow {
  /** 사용한 비율 0~100 */
  usedPercent: number;
  /** 리셋 시각 (ISO 8601, UTC) */
  resetsAt: string;
  windowSeconds: number;
  /** 창이 시작된 뒤 지난 시간 비율 0~100 (시간 눈금 위치) */
  elapsedPercent: number;
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
}

export type ServiceState = "operational" | "degraded" | "outage" | "unknown";

export interface ServiceStatus {
  name: string;
  state: ServiceState;
}

export interface TokenCost {
  tokens: number;
  costUsd: number;
}

export interface Summary {
  claudeCodeToday: TokenCost | null;
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
