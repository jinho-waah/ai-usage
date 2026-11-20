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
    /// 계정 이름을 그대로 메뉴바에 보여 준다 (예: "W1"). 예전 설정 파일의 "alias" 도 이것으로 읽는다.
    #[serde(alias = "alias")]
    Name,
    /// 이모지 (예: "💼")
    Emoji,
    /// 계정 색으로 칠한 작은 점
    Dot,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AccountLabel {
    pub kind: LabelKind,
    /// 이모지 글자. `Name`·`Dot` 이면 쓰지 않는다.
    #[serde(default)]
    pub text: String,
}

/// 하나의 사용량 창(주간 또는 5시간 세션).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UsageWindow {
    /// 사용한 비율 (0~100)
    pub used_percent: f64,
    /// 이 창이 리셋되는 시각 (UTC). 아직 이번 창을 한 번도 안 썼으면 서버가 알려 주지 않는다(`None`).
    pub resets_at: Option<DateTime<Utc>>,
    /// 창의 길이(초). 주간 = 604800, 5시간 = 18000
    pub window_seconds: i64,
    /// (계산값) 창이 시작된 뒤 시간이 얼마나 지났는지 (0~100). 메뉴바 막대의 시간 눈금 위치.
    /// 리셋 시각을 모르면 `None` 이고, 이때는 눈금을 그리지 않는다.
    pub elapsed_percent: Option<f64>,
    /// (계산값) 사용량이 시간 진행보다 `PACE_MARGIN_PERCENT` 이상 앞서는지. true 면 빨강.
    pub ahead_of_pace: bool,
}

impl UsageWindow {
    /// 원본 값으로 만들고, 시간 진행·페이스는 `now` 기준으로 계산해 둔다.
    pub fn new(
        used_percent: f64,
        resets_at: Option<DateTime<Utc>>,
        window_seconds: i64,
        now: DateTime<Utc>,
    ) -> Self {
        let used_percent = used_percent.clamp(0.0, 100.0);
        // `map`: 리셋 시각이 있을 때만 계산하고, 없으면 None 그대로 둔다.
        let elapsed_percent = resets_at.map(|resets_at| {
            let started_at = resets_at - Duration::seconds(window_seconds);
            let elapsed_seconds = (now - started_at).num_seconds() as f64;
            (elapsed_seconds / window_seconds as f64 * 100.0).clamp(0.0, 100.0)
        });

        UsageWindow {
            used_percent,
            resets_at,
            window_seconds,
            elapsed_percent,
            ahead_of_pace: elapsed_percent
                .is_some_and(|elapsed| used_percent >= elapsed + PACE_MARGIN_PERCENT),
        }
    }
}

/// 서비스가 알려 준 사용량 창 하나의 원래 값 (시간 진행은 아직 계산하지 않은 상태)
#[derive(Debug, Clone, PartialEq)]
pub struct RawWindow {
    pub percent: f64,
    pub resets_at: Option<DateTime<Utc>>,
}

/// 서비스(Claude, Codex)에서 한 번 받아 온 사용량
#[derive(Debug, Clone, PartialEq, Default)]
pub struct UsageReading {
    pub weekly: Option<RawWindow>,
    /// 5시간 세션. 이 한도가 없는 플랜이면 `None`.
    pub session: Option<RawWindow>,
    /// 모델별 주간 한도 (예: Fable 2%)
    pub models: Vec<ModelUsage>,
}

/// 계정 상태. 팝오버 카드에 안내 문구와 버튼을 띄울 때 쓴다.
/// JSON 으로는 `{"kind":"ok"}`, `{"kind":"error","message":"..."}` 처럼 나간다.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum AccountStatus {
    /// 정상
    Ok,
    /// 첫 데이터를 기다리는 중
    Loading,
    /// 로그인이 필요함 (아직 로그인하지 않았거나 세션이 만료됨)
    NeedsLogin,
    /// claude.ai 의 봇 확인(Cloudflare)을 통과해야 함
    Challenge,
    /// 그 밖의 오류
    Error { message: String },
}

/// 모델별 주간 한도 (예: Opus 12%)
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ModelUsage {
    pub name: String,
    pub used_percent: f64,
}

/// 한도를 넘긴 뒤 추가로 쓴 사용량(Extra usage)과 크레딧.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
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
    /// 플랜 이름 (예: "Ultra", "Max", "Pro")
    pub plan: Option<String>,
    pub weekly: Option<UsageWindow>,
    /// 5시간 세션 한도. 이 한도가 없는 플랜이면 `None`.
    pub session: Option<UsageWindow>,
    pub models: Vec<ModelUsage>,
    pub extra: Option<ExtraUsage>,
    /// 이번 달 Claude API 비용 (이 계정에 Admin API 키가 있을 때만)
    pub api_cost: Option<crate::api_cost::MonthCost>,
    pub trend: Vec<TrendPoint>,
    pub status: AccountStatus,
    /// 마지막으로 사용량을 받아 온 시각. 아직 못 받았으면 `None`.
    pub updated_at: Option<DateTime<Utc>>,
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

/// 브랜드(Claude 전체, Codex 전체)별 누적 토큰. 이 Mac 의 Claude Code·Codex 로그 기준.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct BrandTokens {
    pub provider: Provider,
    /// 캐시를 포함한 합계
    pub total: u64,
    /// 캐시를 뺀 입력
    pub input: u64,
    pub output: u64,
    pub cache_write: u64,
    pub cache_read: u64,
    /// 추론 토큰 (Codex, 출력에 포함된 값)
    pub reasoning: u64,
    /// 오늘(로컬 날짜) 합계
    pub today: u64,
    /// 기록이 시작된 날 ("YYYY-MM-DD")
    pub since: Option<String>,
}

/// 팝오버 아래쪽 요약.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Summary {
    /// 브랜드별 누적 토큰 (Claude, Codex 순)
    pub tokens: Vec<BrandTokens>,
    /// 로그를 집계하는 중이면 true
    pub tokens_scanning: bool,
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
        let resets_at = Some(now + Duration::seconds(302_400));
        let calm = UsageWindow::new(52.0, resets_at, 604_800, now);
        assert!((calm.elapsed_percent.unwrap() - 50.0).abs() < 0.01);
        assert!(!calm.ahead_of_pace, "52% < 50% + 5%p 이므로 빨강이 아님");

        let fast = UsageWindow::new(56.0, resets_at, 604_800, now);
        assert!(fast.ahead_of_pace, "56% >= 50% + 5%p 이므로 빨강");
    }

    #[test]
    fn unknown_reset_time_means_no_marker_and_no_red() {
        let window = UsageWindow::new(90.0, None, 604_800, Utc::now());
        assert_eq!(window.elapsed_percent, None);
        assert!(!window.ahead_of_pace);
    }

    #[test]
    fn account_status_serializes_with_kind_tag() {
        let json = serde_json::to_string(&AccountStatus::Error {
            message: "x".into(),
        })
        .unwrap();
        assert_eq!(json, r#"{"kind":"error","message":"x"}"#);
        let json = serde_json::to_string(&AccountStatus::NeedsLogin).unwrap();
        assert_eq!(json, r#"{"kind":"needsLogin"}"#);
    }
}
