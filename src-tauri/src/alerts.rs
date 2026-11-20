//! macOS 알림. 이 앱이 알리는 것: 로그인 만료, 서비스 장애, 한도 알림(설정에서 끌 수 있음).
//!
//! - 로그인 만료: 로그인돼 있던 계정이 갱신 두 번 연속 "로그인 필요"가 되면 한 번 알린다.
//!   (한 번 실패는 일시적인 오류일 수 있어서 기다린다) 다시 정상이 되면 다음 만료 때 또 알린다.
//! - 서비스 장애: 상태 페이지가 정상 → 문제로 바뀌거나, 느림 → 장애로 심해질 때 알린다.
//!   마지막으로 알린 상태를 기록 DB 에 남겨서, 앱을 다시 켜도 같은 장애를 또 알리지 않는다.
//! - 한도 알림: 주간·5시간 한도를 설정한 만큼(기본 90%) 쓰면 주기마다 한 번 알리고,
//!   그렇게 알렸던 한도가 리셋되면 다시 쓸 수 있다고 한 번 알린다.

use tauri::{AppHandle, Manager};
use tauri_plugin_notification::NotificationExt;

use crate::accounts::Account;
use crate::model::{AccountStatus, Provider, RawWindow, ServiceState};
use crate::services::ServiceReport;
use crate::settings::PercentMode;
use crate::state::AppState;

/// 이만큼 연속으로 "로그인 필요"여야 알린다
const LOGIN_MISSES_BEFORE_ALERT: u32 = 2;

pub fn notify(app: &AppHandle, title: &str, body: &str) {
    if let Err(error) = app.notification().builder().title(title).body(body).show() {
        eprintln!("[ai-usage] 알림을 보내지 못했어요: {error}");
    }
}

/// Mac 의 첫 번째 선호 언어가 한국어인지 (알림 문구 언어를 고를 때)
pub fn prefers_korean() -> bool {
    #[cfg(target_os = "macos")]
    {
        let languages = objc2_foundation::NSLocale::preferredLanguages();
        if let Some(first) = languages.firstObject() {
            return first.to_string().starts_with("ko");
        }
    }
    false
}

/// 갱신이 한 번 끝날 때마다 부른다. 로그인이 풀린 계정이 새로 생겼으면 알린다.
pub fn check_logins(app: &AppHandle) {
    let state = app.state::<AppState>();
    if state.demo {
        return;
    }
    let accounts = state.accounts();
    let statuses: Vec<(Account, Option<AccountStatus>)> = {
        let data = state.account_data.lock().unwrap();
        accounts
            .into_iter()
            .map(|account| {
                let status = data.get(&account.id).and_then(|data| data.status.clone());
                (account, status)
            })
            .collect()
    };

    let mut expired = Vec::new();
    {
        let mut misses = state.login_misses.lock().unwrap();
        for (account, status) in statuses {
            match status {
                Some(AccountStatus::NeedsLogin) => {
                    // 한 번도 로그인하지 않은 Claude 계정(조직 없음)은 만료가 아니다
                    let was_signed_in =
                        account.provider == Provider::Codex || account.org_id.is_some();
                    if !was_signed_in || state.is_logging_in(&account.id) {
                        continue;
                    }
                    let count = misses.entry(account.id.clone()).or_insert(0);
                    *count += 1;
                    if *count == LOGIN_MISSES_BEFORE_ALERT {
                        expired.push(account);
                    }
                }
                Some(AccountStatus::Ok) => {
                    misses.remove(&account.id);
                }
                _ => {}
            }
        }
    }

    let korean = prefers_korean();
    for account in expired {
        let (title, body) = match (account.provider, korean) {
            (Provider::Claude, true) => (
                "로그인이 풀렸어요".to_string(),
                format!(
                    "'{}' 계정에 다시 로그인해 주세요. 메뉴바 팝오버에서 할 수 있어요.",
                    account.name
                ),
            ),
            (Provider::Claude, false) => (
                "Signed out".to_string(),
                format!(
                    "'{}' was signed out. Sign in again from the menu bar popover.",
                    account.name
                ),
            ),
            (Provider::Codex, true) => (
                "Codex 로그인이 풀렸어요".to_string(),
                "터미널에서 codex login 을 실행해 주세요.".to_string(),
            ),
            (Provider::Codex, false) => (
                "Codex signed out".to_string(),
                "Run codex login in Terminal.".to_string(),
            ),
        };
        notify(app, &title, &body);
    }
}

/// 서비스 상태를 확인할 때마다 부른다. 새로 문제가 생겼거나 심해졌으면 알린다.
pub fn service_checked(app: &AppHandle, name: &str, report: &ServiceReport) {
    let state = app.state::<AppState>();
    let Some(history) = &state.history else {
        return;
    };
    let current = match report.state {
        ServiceState::Operational => "operational",
        ServiceState::Degraded => "degraded",
        ServiceState::Outage => "outage",
        // 상태 페이지를 못 읽은 건 장애가 아니다
        ServiceState::Unknown => return,
    };
    let key = format!("service-alert:{name}");
    let previous = history.meta(&key);
    let should_alert = match (previous.as_deref(), current) {
        (_, "operational") => false,
        (Some("outage"), _) => false,
        (Some("degraded"), "degraded") => false,
        // 정상(또는 처음 확인) → 문제, 느림 → 장애
        _ => true,
    };
    if previous.as_deref() != Some(current) {
        let _ = history.set_meta(&key, current);
    }
    if !should_alert {
        return;
    }

    let korean = prefers_korean();
    let title = match (report.state, korean) {
        (ServiceState::Outage, true) => format!("{name} 서비스 장애"),
        (_, true) => format!("{name} 서비스 지연"),
        (ServiceState::Outage, false) => format!("{name} outage"),
        (_, false) => format!("{name} degraded"),
    };
    let body = report.detail.clone().unwrap_or_else(|| {
        if korean {
            "상태 페이지에 문제가 올라왔어요.".to_string()
        } else {
            "An issue is posted on the status page.".to_string()
        }
    });
    notify(app, &title, &body);
}

/// 리셋 시각 차이가 이만큼 안이면 같은 주기로 본다 (Codex 는 몇 초씩 흔들린다)
const SAME_CYCLE_TOLERANCE_SECONDS: i64 = 60 * 60;
/// 리셋 시각을 모를 때: 이만큼(%p) 내려가야 다음에 다시 알린다
const REARM_DROP: f64 = 10.0;

/// 한도 알림에서 일어난 일
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LimitEvent {
    /// 알릴 것 없음
    None,
    /// 설정한 만큼 썼다 (이 주기에서 처음)
    Reached,
    /// 알렸던 한도가 리셋됐다
    Reset,
    /// 알림 없이, 다음에 다시 알릴 수 있게만 한다 (리셋 시각을 모르는 경우)
    Rearm,
}

/// 한도 알림을 낼지 정한다. (테스트하기 쉽게 따로 뺀 계산)
/// - `alerted_cycle`: 이미 알린 주기의 리셋 시각(유닉스 초). 없으면 아직 알린 적 없음.
/// - `cycle`: 지금 주기의 리셋 시각(유닉스 초). 모르면 0.
pub fn limit_event(
    used: f64,
    threshold: f64,
    alerted_cycle: Option<i64>,
    cycle: i64,
) -> LimitEvent {
    let same_cycle = alerted_cycle
        .is_some_and(|alerted| (alerted - cycle).abs() <= SAME_CYCLE_TOLERANCE_SECONDS);
    if used >= threshold {
        return if same_cycle {
            LimitEvent::None
        } else {
            LimitEvent::Reached
        };
    }
    match alerted_cycle {
        // 알렸던 주기가 끝나고 새 주기가 시작됐다
        Some(_) if cycle != 0 && !same_cycle => LimitEvent::Reset,
        // 리셋 시각을 모르면, 한참 내려갔을 때 조용히 다시 알릴 준비만 한다
        Some(_) if cycle == 0 && used < threshold - REARM_DROP => LimitEvent::Rearm,
        _ => LimitEvent::None,
    }
}

/// 갱신이 한 번 끝날 때마다 부른다. 한도를 설정한 만큼 썼거나, 알렸던 한도가 리셋됐으면 알린다.
pub fn check_limits(app: &AppHandle) {
    let state = app.state::<AppState>();
    if state.demo {
        return;
    }
    let (threshold, percent_mode) = {
        let settings = state.settings.lock().unwrap();
        (settings.limit_alert_percent as f64, settings.percent_mode)
    };
    // 0 이면 끔
    if threshold <= 0.0 {
        return;
    }
    let Some(history) = &state.history else {
        return;
    };
    let accounts = state.accounts();
    let data = state.account_data.lock().unwrap().clone();
    let korean = prefers_korean();
    let now = chrono::Utc::now();

    for account in &accounts {
        let Some(data) = data.get(&account.id) else {
            continue;
        };
        // 방금 받아 온 값만 믿는다 (로그인이 풀렸거나 오류면 건너뜀)
        if data.status != Some(AccountStatus::Ok) {
            continue;
        }
        let Some(usage) = &data.usage else {
            continue;
        };
        let windows: [(&str, bool, &Option<RawWindow>); 2] = [
            ("weekly", true, &usage.weekly),
            ("session", false, &usage.session),
        ];
        for (kind, is_weekly, window) in windows {
            let Some(window) = window else {
                continue;
            };
            // 이 계정·이 한도에서 마지막으로 알린 주기 (리셋 시각)
            let key = format!("limit-alert:{}:{kind}", account.id);
            let alerted_cycle: Option<i64> =
                history.meta(&key).and_then(|value| value.parse().ok());
            let cycle = window.resets_at.map_or(0, |reset| reset.timestamp());

            match limit_event(window.percent, threshold, alerted_cycle, cycle) {
                LimitEvent::None => {}
                LimitEvent::Reached => {
                    let (title, body) = reached_message(
                        &account.name,
                        is_weekly,
                        window,
                        percent_mode,
                        now,
                        korean,
                    );
                    notify(app, &title, &body);
                    let _ = history.set_meta(&key, &cycle.to_string());
                }
                LimitEvent::Reset => {
                    let (title, body) = reset_message(&account.name, is_weekly, korean);
                    notify(app, &title, &body);
                    let _ = history.set_meta(&key, "");
                }
                LimitEvent::Rearm => {
                    let _ = history.set_meta(&key, "");
                }
            }
        }
    }
}

fn limit_name(is_weekly: bool, korean: bool) -> &'static str {
    match (is_weekly, korean) {
        (true, true) => "주간 한도",
        (false, true) => "5시간 한도",
        (true, false) => "weekly limit",
        (false, false) => "5-hour limit",
    }
}

/// "설정한 만큼 썼다" 알림 문구. 숫자는 설정의 표시 기준(남은 양·사용한 양)을 따른다.
fn reached_message(
    name: &str,
    is_weekly: bool,
    window: &RawWindow,
    percent_mode: PercentMode,
    now: chrono::DateTime<chrono::Utc>,
    korean: bool,
) -> (String, String) {
    let limit = limit_name(is_weekly, korean);
    let used = window.percent.clamp(0.0, 100.0);
    let title = if used >= 100.0 {
        if korean {
            format!("{name} {limit}를 다 썼어요")
        } else {
            format!("{name}: {limit} used up")
        }
    } else {
        match (percent_mode, korean) {
            (PercentMode::Remaining, true) => format!("{name} {limit} {:.0}% 남음", 100.0 - used),
            (PercentMode::Used, true) => format!("{name} {limit} {used:.0}% 사용"),
            (PercentMode::Remaining, false) => {
                format!("{name}: {:.0}% of {limit} left", 100.0 - used)
            }
            (PercentMode::Used, false) => format!("{name}: {used:.0}% of {limit} used"),
        }
    };
    let body = match window.resets_at {
        Some(reset) if reset > now => {
            let hours = (reset - now).num_minutes() as f64 / 60.0;
            if korean {
                format!("리셋까지 {hours:.1}시간 남았어요.")
            } else {
                format!("Resets in {hours:.1} hours.")
            }
        }
        _ => String::new(),
    };
    (title, body)
}

fn reset_message(name: &str, is_weekly: bool, korean: bool) -> (String, String) {
    let limit = limit_name(is_weekly, korean);
    if korean {
        (
            format!("{name} {limit}가 리셋됐어요"),
            "다시 쓸 수 있어요.".to_string(),
        )
    } else {
        (
            format!("{name}: {limit} has reset"),
            "You can use it again.".to_string(),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const DAY: i64 = 86_400;

    #[test]
    fn alerts_once_per_cycle_then_announces_the_reset() {
        let this_cycle = 1_800_000_000;
        let next_cycle = this_cycle + 7 * DAY;

        // 90% 를 넘으면 한 번 알리고, 같은 주기에서는 다시 알리지 않는다
        assert_eq!(
            limit_event(91.0, 90.0, None, this_cycle),
            LimitEvent::Reached
        );
        assert_eq!(
            limit_event(97.0, 90.0, Some(this_cycle), this_cycle),
            LimitEvent::None
        );
        // 리셋 시각이 몇 초 흔들려도 같은 주기다 (Codex)
        assert_eq!(
            limit_event(97.0, 90.0, Some(this_cycle), this_cycle + 5),
            LimitEvent::None
        );
        // 새 주기가 시작돼 사용량이 내려가면 리셋 알림
        assert_eq!(
            limit_event(2.0, 90.0, Some(this_cycle), next_cycle),
            LimitEvent::Reset
        );
        // 알린 적 없으면 리셋 알림도 없다
        assert_eq!(limit_event(2.0, 90.0, None, next_cycle), LimitEvent::None);
        // 새 주기에서 곧바로 다시 넘으면 그 주기 몫으로 알린다
        assert_eq!(
            limit_event(95.0, 90.0, Some(this_cycle), next_cycle),
            LimitEvent::Reached
        );
    }

    #[test]
    fn unknown_reset_time_rearms_quietly_after_a_big_drop() {
        assert_eq!(limit_event(92.0, 90.0, None, 0), LimitEvent::Reached);
        assert_eq!(limit_event(92.0, 90.0, Some(0), 0), LimitEvent::None);
        // 살짝 내려간 것으로는 다시 알리지 않는다
        assert_eq!(limit_event(88.0, 90.0, Some(0), 0), LimitEvent::None);
        assert_eq!(limit_event(5.0, 90.0, Some(0), 0), LimitEvent::Rearm);
    }

    #[test]
    fn message_follows_the_display_basis() {
        let now = chrono::Utc::now();
        let window = RawWindow {
            percent: 92.0,
            resets_at: Some(now + chrono::Duration::minutes(150)),
        };
        let (title, body) = reached_message("W1", true, &window, PercentMode::Remaining, now, true);
        assert_eq!(title, "W1 주간 한도 8% 남음");
        assert_eq!(body, "리셋까지 2.5시간 남았어요.");
        let (title, _) = reached_message("W1", false, &window, PercentMode::Used, now, true);
        assert_eq!(title, "W1 5시간 한도 92% 사용");
        let full = RawWindow {
            percent: 100.0,
            resets_at: None,
        };
        let (title, body) = reached_message("W1", true, &full, PercentMode::Remaining, now, true);
        assert_eq!(title, "W1 주간 한도를 다 썼어요");
        assert_eq!(body, "");
    }
}
