//! macOS 알림. 이 앱은 두 가지만 알린다: 로그인 만료, 서비스 장애.
//!
//! - 로그인 만료: 로그인돼 있던 계정이 갱신 두 번 연속 "로그인 필요"가 되면 한 번 알린다.
//!   (한 번 실패는 일시적인 오류일 수 있어서 기다린다) 다시 정상이 되면 다음 만료 때 또 알린다.
//! - 서비스 장애: 상태 페이지가 정상 → 문제로 바뀌거나, 느림 → 장애로 심해질 때 알린다.
//!   마지막으로 알린 상태를 기록 DB 에 남겨서, 앱을 다시 켜도 같은 장애를 또 알리지 않는다.

use tauri::{AppHandle, Manager};
use tauri_plugin_notification::NotificationExt;

use crate::accounts::Account;
use crate::model::{AccountStatus, Provider, ServiceState};
use crate::services::ServiceReport;
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
