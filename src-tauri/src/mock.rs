//! M1 단계용 가짜 데이터.
//!
//! 실제 데이터 수집(claude.ai, Codex, Admin API, 로컬 로그)은 M2~M5 에서 붙인다.
//! 그 전까지 메뉴바와 팝오버가 어떻게 보이는지 확인하려고 그럴듯한 값을 만든다.
//! 리셋 시각은 앱을 켠 시각(`started_at`) 기준으로 정해 두어서,
//! 시간이 흐르면 메뉴바의 시간 눈금이 실제로 조금씩 움직인다.

use chrono::{DateTime, Duration, Timelike, Utc};

use crate::model::{
    AccountLabel, AccountStatus, AccountUsage, BrandTokens, ExtraUsage, LabelKind, ModelUsage,
    Provider, ServiceState, ServiceStatus, Snapshot, Summary, TrendPoint, UsageWindow,
};

const WEEK_SECONDS: i64 = 7 * 24 * 60 * 60;
const FIVE_HOURS_SECONDS: i64 = 5 * 60 * 60;

pub fn snapshot(started_at: DateTime<Utc>, now: DateTime<Utc>) -> Snapshot {
    // 개인: 페이스와 비슷하게 쓰는 중 (단색)
    let personal_weekly = UsageWindow::new(
        58.0,
        Some(started_at + Duration::days(3) + Duration::hours(2)),
        WEEK_SECONDS,
        now,
    );
    // 회사: 주 초반인데 많이 씀 → 시간 눈금보다 5%p 이상 앞서서 빨강
    let work_weekly = UsageWindow::new(
        41.0,
        Some(started_at + Duration::days(5) + Duration::hours(9)),
        WEEK_SECONDS,
        now,
    );
    let work_session = UsageWindow::new(
        12.0,
        Some(started_at + Duration::hours(2) + Duration::minutes(40)),
        FIVE_HOURS_SECONDS,
        now,
    );
    // Codex: 주 후반, 여유 있음
    let codex_weekly = UsageWindow::new(
        55.0,
        Some(started_at + Duration::days(1) + Duration::hours(4)),
        WEEK_SECONDS,
        now,
    );

    let accounts = vec![
        AccountUsage {
            id: "mock-personal".into(),
            provider: Provider::Claude,
            name: "개인".into(),
            label: AccountLabel {
                kind: LabelKind::Name,
                text: String::new(),
            },
            color: "#D97757".into(),
            plan: Some("Ultra".into()),
            trend: weekly_trend(&personal_weekly, now, 30.0, 72.0),
            weekly: Some(personal_weekly),
            session: None,
            models: vec![
                ModelUsage {
                    name: "Opus".into(),
                    used_percent: 12.0,
                },
                ModelUsage {
                    name: "Sonnet".into(),
                    used_percent: 31.0,
                },
            ],
            extra: Some(ExtraUsage {
                used: 4.2,
                limit: Some(50.0),
                currency: "USD".into(),
            }),
            api_cost: Some(crate::api_cost::MonthCost {
                usd: 3.1,
                refreshed_at: Some(now - Duration::hours(3)),
            }),
            status: AccountStatus::Ok,
            updated_at: Some(now),
        },
        AccountUsage {
            id: "mock-work".into(),
            provider: Provider::Claude,
            name: "회사".into(),
            label: AccountLabel {
                kind: LabelKind::Emoji,
                text: "💼".into(),
            },
            color: "#6A9BCC".into(),
            plan: Some("Pro".into()),
            trend: weekly_trend(&work_weekly, now, 10.0, 64.0),
            weekly: Some(work_weekly),
            session: Some(work_session),
            models: vec![],
            extra: None,
            api_cost: None,
            status: AccountStatus::Ok,
            updated_at: Some(now),
        },
        AccountUsage {
            id: "mock-codex".into(),
            provider: Provider::Codex,
            name: "Codex".into(),
            label: AccountLabel {
                kind: LabelKind::Dot,
                text: String::new(),
            },
            color: "#10A37F".into(),
            plan: Some("Pro".into()),
            trend: weekly_trend(&codex_weekly, now, 20.0, 80.0),
            weekly: Some(codex_weekly),
            session: None,
            models: vec![],
            extra: None,
            api_cost: None,
            status: AccountStatus::Ok,
            updated_at: Some(now),
        },
    ];

    Snapshot {
        accounts,
        summary: Summary {
            tokens: vec![
                BrandTokens {
                    provider: Provider::Claude,
                    total: 2_481_300_000,
                    input: 3_100_000,
                    output: 21_400_000,
                    cache_write: 96_800_000,
                    cache_read: 2_360_000_000,
                    reasoning: 0,
                    today: 61_200_000,
                    since: Some("2026-10-21".into()),
                },
                BrandTokens {
                    provider: Provider::Codex,
                    total: 412_700_000,
                    input: 58_300_000,
                    output: 4_100_000,
                    cache_write: 0,
                    cache_read: 350_300_000,
                    reasoning: 1_900_000,
                    today: 9_800_000,
                    since: Some("2026-11-10".into()),
                },
            ],
            tokens_scanning: false,
            services: vec![
                ServiceStatus {
                    name: "Claude".into(),
                    state: ServiceState::Operational,
                },
                ServiceStatus {
                    name: "OpenAI".into(),
                    state: ServiceState::Operational,
                },
            ],
        },
        updated_at: now,
    }
}

/// 최근 7일 동안 한 시간 간격의 사용률 변화를 만든다.
/// 이번 주기(리셋 이후)는 0% 에서 현재 사용률까지 오르고,
/// 그 전(지난 주기)은 `prev_from` 에서 `prev_to` 까지 오른 것으로 만든다.
fn weekly_trend(
    window: &UsageWindow,
    now: DateTime<Utc>,
    prev_from: f64,
    prev_to: f64,
) -> Vec<TrendPoint> {
    let cycle_start = window.resets_at.unwrap_or(now) - Duration::seconds(window.window_seconds);
    let hours: Vec<DateTime<Utc>> = (0..=168).map(|i| now - Duration::hours(168 - i)).collect();

    // 시간대별 활동량: 한국 시간 새벽에는 적게, 저녁에는 많이 쓴다고 가정
    let activity = |t: &DateTime<Utc>| {
        let korean_hour = (t.hour() + 9) % 24;
        if korean_hour < 9 {
            0.1
        } else if korean_hour >= 19 {
            1.6
        } else {
            1.0
        }
    };

    let current_total: f64 = hours
        .iter()
        .filter(|t| **t >= cycle_start)
        .map(activity)
        .sum();
    let previous_total: f64 = hours
        .iter()
        .filter(|t| **t < cycle_start)
        .map(activity)
        .sum();

    let mut current_sum = 0.0;
    let mut previous_sum = 0.0;
    let mut points = Vec::with_capacity(hours.len());
    for t in &hours {
        let percent = if *t >= cycle_start {
            current_sum += activity(t);
            window.used_percent * current_sum / current_total.max(f64::EPSILON)
        } else {
            previous_sum += activity(t);
            prev_from + (prev_to - prev_from) * previous_sum / previous_total.max(f64::EPSILON)
        };
        points.push(TrendPoint { at: *t, percent });
    }
    points
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mock_shows_one_red_account_at_start() {
        let now = Utc::now();
        let snap = snapshot(now, now);
        let red: Vec<&str> = snap
            .accounts
            .iter()
            .filter(|a| a.primary_window().is_some_and(|w| w.ahead_of_pace))
            .map(|a| a.name.as_str())
            .collect();
        assert_eq!(red, vec!["회사"]);
    }

    #[test]
    fn trend_ends_at_current_usage() {
        let now = Utc::now();
        let snap = snapshot(now, now);
        let personal = &snap.accounts[0];
        let last = personal.trend.last().unwrap().percent;
        assert!((last - 58.0).abs() < 0.01);
    }
}
