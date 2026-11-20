//! 앱 전체에서 쓰는 데이터 구조를 모아 둔 파일.
//!
//! Rust 메모
//! - `struct` 는 여러 값을 묶은 자료형, `enum` 은 "여러 경우 중 하나"를 나타내는 자료형이다.
//! - `#[derive(Serialize)]` 를 붙이면 그 자료형을 JSON 으로 바꿀 수 있다.
//!   Tauri 는 Rust 함수의 반환값을 JSON 으로 바꿔서 React 화면으로 보내므로,
//!   화면에 보낼 자료형에는 모두 `Serialize` 가 붙어 있다.
//! - `#[serde(rename_all = "camelCase")]` 는 Rust 식 이름(`used_percent`)을
//!   TypeScript 식 이름(`usedPercent`)으로 바꿔서 내보내라는 뜻이다.

use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};

/// 사용량이 시간 진행보다 이만큼(%p) 이상 앞서면 "빨리 쓰는 중"으로 보고 빨강으로 표시한다.
pub const PACE_MARGIN_PERCENT: f64 = 5.0;

/// 사용량을 어디서 가져오는지.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum Provider {
    Claude,
    Codex,
}

/// 메뉴바에서 계정을 구분하는 표시 종류.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum LabelKind {
    /// 1~2글자 별칭 (예: "J")
    Alias,
    /// 이모지 (예: "💼")
    Emoji,
    /// 계정 색으로 칠한 작은 점
    Dot,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountLabel {
    pub kind: LabelKind,
    /// 별칭이나 이모지 글자. `Dot` 이면 쓰지 않는다.
    pub text: String,
}

/// 하나의 사용량 창(주간 또는 5시간 세션).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UsageWindow {
    /// 사용한 비율 (0~100)
    pub used_percent: f64,
    /// 이 창이 리셋되는 시각 (UTC)
    pub resets_at: DateTime<Utc>,
    /// 창의 길이(초). 주간 = 604800, 5시간 = 18000
    pub window_seconds: i64,
    /// (계산값) 창이 시작된 뒤 시간이 얼마나 지났는지 (0~100). 메뉴바 막대의 시간 눈금 위치.
    pub elapsed_percent: f64,
    /// (계산값) 사용량이 시간 진행보다 `PACE_MARGIN_PERCENT` 이상 앞서는지. true 면 빨강.
    pub ahead_of_pace: bool,
}

impl UsageWindow {
    /// 원본 값으로 만들고, 시간 진행·페이스는 `now` 기준으로 계산해 둔다.
    pub fn new(
        used_percent: f64,
        resets_at: DateTime<Utc>,
        window_seconds: i64,
        now: DateTime<Utc>,
    ) -> Self {
        let started_at = resets_at - Duration::seconds(window_seconds);
        let elapsed_seconds = (now - started_at).num_seconds() as f64;
        let elapsed_percent = (elapsed_seconds / window_seconds as f64 * 100.0).clamp(0.0, 100.0);
        let used_percent = used_percent.clamp(0.0, 100.0);

        UsageWindow {
            used_percent,
            resets_at,
            window_seconds,
            elapsed_percent,
            ahead_of_pace: used_percent >= elapsed_percent + PACE_MARGIN_PERCENT,
        }
    }
}

/// 모델별 주간 한도 (예: Opus 12%)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelUsage {
    pub name: String,
    pub used_percent: f64,
}

/// 한도를 넘긴 뒤 추가로 쓴 사용량(Extra usage)과 크레딧.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExtraUsage {
    /// 이번 달에 쓴 금액
    pub used: f64,
    /// 월 한도. 없으면 `None` (JSON 에서는 null)
    pub limit: Option<f64>,
    /// 통화 (예: "USD")
    pub currency: String,
}

/// 미니 추세 차트의 점 하나.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TrendPoint {
    pub at: DateTime<Utc>,
    pub percent: f64,
}

/// 계정 하나의 사용량.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountUsage {
    pub id: String,
    pub provider: Provider,
    /// 사용자가 붙인 이름 (예: "개인")
    pub name: String,
    pub label: AccountLabel,
    /// 계정 색 ("#RRGGBB"). 색 점 라벨과 차트에 쓴다.
    pub color: String,
    /// 플랜 이름 (예: "Max 20x")
    pub plan: Option<String>,
    pub weekly: Option<UsageWindow>,
    /// 5시간 세션 한도. 이 한도가 없는 플랜이면 `None`.
    pub session: Option<UsageWindow>,
    pub models: Vec<ModelUsage>,
    pub extra: Option<ExtraUsage>,
    pub trend: Vec<TrendPoint>,
}

impl AccountUsage {
    /// 메뉴바에 보여 줄 대표 창. 주간이 있으면 주간, 없으면 세션.
    pub fn primary_window(&self) -> Option<&UsageWindow> {
        self.weekly.as_ref().or(self.session.as_ref())
    }
}

/// 서비스 상태 (status.claude.com 등)
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum ServiceState {
    Operational,
    Degraded,
    Outage,
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ServiceStatus {
    pub name: String,
    pub state: ServiceState,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TokenCost {
    pub tokens: u64,
    pub cost_usd: f64,
}

/// 팝오버 아래쪽 요약.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Summary {
    /// 오늘 Claude Code 실제 토큰과 추정 비용 (로컬 로그 기준)
    pub claude_code_today: Option<TokenCost>,
    /// 이번 달 API 비용 (Admin API 기준)
    pub api_month_cost_usd: Option<f64>,
    pub services: Vec<ServiceStatus>,
}

/// 한 번 갱신했을 때의 전체 상태. 메뉴바와 팝오버는 모두 이것을 그린다.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Snapshot {
    pub accounts: Vec<AccountUsage>,
    pub summary: Summary,
    pub updated_at: DateTime<Utc>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn elapsed_and_pace_are_computed_from_reset_time() {
        let now = Utc::now();
        // 7일짜리 창이 3.5일 뒤에 리셋 → 정확히 절반(50%) 지남
        let resets_at = now + Duration::seconds(302_400);
        let calm = UsageWindow::new(52.0, resets_at, 604_800, now);
        assert!((calm.elapsed_percent - 50.0).abs() < 0.01);
        assert!(!calm.ahead_of_pace, "52% < 50% + 5%p 이므로 빨강이 아님");

        let fast = UsageWindow::new(56.0, resets_at, 604_800, now);
        assert!(fast.ahead_of_pace, "56% >= 50% + 5%p 이므로 빨강");
    }
}
