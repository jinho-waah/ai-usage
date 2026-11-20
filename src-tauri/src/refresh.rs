//! 갱신: 계정마다 사용량을 받아 오고, 스냅샷을 만들어 메뉴바와 화면에 알린다.

use std::collections::HashMap;

use chrono::{DateTime, Utc};
use tauri::{AppHandle, Emitter, Manager};

use crate::accounts::Account;
use crate::claude::{SESSION_SECONDS, WEEK_SECONDS};
use crate::codex::{self, ApiOutcome};
use crate::history::WindowKind;
use crate::menubar::MenubarModel;
use crate::model::{
    AccountStatus, AccountUsage, Provider, Snapshot, Summary, TrendPoint, UsageWindow,
};
use crate::state::{AccountData, AppState};
use crate::{claude_web, mock};

pub const TRAY_ID: &str = "main";

/// 모든 계정을 새로 조회한 뒤 알린다. (네트워크 사용)
pub async fn refresh_all(app: &AppHandle) -> Snapshot {
    let state = app.state::<AppState>();
    // 갱신이 겹치지 않게: 다른 갱신이 진행 중이면 끝날 때까지 기다린다.
    let _guard = state.refresh_lock.lock().await;
    if !state.demo {
        for account in state.accounts() {
            match account.provider {
                Provider::Claude => claude_web::refresh_account(app, &account).await,
                Provider::Codex => refresh_codex_account(app, &account).await,
            }
        }
        record_history(&state);
    }
    publish(app)
}

/// 방금 받은 사용률을 기록 DB 에 남긴다. (값이 바뀐 경우만 실제로 저장된다)
pub fn record_history(state: &AppState) {
    let Some(history) = &state.history else {
        return;
    };
    let data = state.account_data.lock().unwrap().clone();
    for (account_id, data) in data {
        let (Some(usage), Some(at), Some(AccountStatus::Ok)) =
            (&data.usage, data.updated_at, &data.status)
        else {
            continue;
        };
        if let Some(window) = &usage.weekly {
            let _ = history.record(&account_id, WindowKind::Weekly, at, window.percent, "live");
        }
        if let Some(window) = &usage.session {
            let _ = history.record(&account_id, WindowKind::Session, at, window.percent, "live");
        }
    }
}

/// 팝오버 미니 그래프용 최근 7일 기록. 주간 한도가 있으면 주간, 없으면 세션.
fn trends(
    state: &AppState,
    accounts: &[Account],
    data: &HashMap<String, AccountData>,
    now: DateTime<Utc>,
) -> HashMap<String, Vec<TrendPoint>> {
    let Some(history) = &state.history else {
        return HashMap::new();
    };
    accounts
        .iter()
        .map(|account| {
            let has_session_only = data
                .get(&account.id)
                .and_then(|data| data.usage.as_ref())
                .is_some_and(|usage| usage.weekly.is_none() && usage.session.is_some());
            let window = if has_session_only {
                WindowKind::Session
            } else {
                WindowKind::Weekly
            };
            let points = history
                .trend(&account.id, window, now, 7)
                .unwrap_or_default();
            (account.id.clone(), points)
        })
        .collect()
}

/// Codex 계정 하나를 조회한다.
/// 사용량 API 를 못 부르면(토큰 만료 등) Codex 세션 로그의 마지막 한도 정보로 대신한다.
pub async fn refresh_codex_account(app: &AppHandle, account: &Account) {
    let state = app.state::<AppState>();
    let home = codex::codex_home();
    let credentials = codex::read_credentials(&home);
    let outcome = match &credentials {
        Ok(credentials) => Some(codex::fetch_usage(credentials).await),
        Err(_) => None,
    };

    if let Some(ApiOutcome::Ok(usage)) = outcome {
        return state.update_data(&account.id, |data| {
            data.usage = Some(usage.reading);
            data.plan = usage.plan;
            data.status = Some(AccountStatus::Ok);
            data.updated_at = Some(Utc::now());
        });
    }

    #[cfg(debug_assertions)]
    eprintln!(
        "[ai-usage] codex api: {:?}",
        outcome.as_ref().map(|o| match o {
            ApiOutcome::Ok(_) => "ok".to_string(),
            ApiOutcome::Unauthorized => "unauthorized".to_string(),
            ApiOutcome::Failed(message) => message.clone(),
        })
    );

    // 실시간 조회 실패 → 세션 로그에서 마지막 기록을 찾는다 (파일 읽기라 별도 스레드에서)
    let logged = tauri::async_runtime::spawn_blocking(move || codex::latest_logged_usage(&home))
        .await
        .ok()
        .flatten();
    match (logged, credentials) {
        (Some((usage, logged_at)), _) => state.update_data(&account.id, |data| {
            data.usage = Some(usage.reading);
            data.plan = usage.plan.or(data.plan.take());
            data.status = Some(AccountStatus::Ok);
            data.updated_at = Some(logged_at);
        }),
        (None, Err(_)) => state.update_data(&account.id, |data| {
            data.status = Some(AccountStatus::NeedsLogin)
        }),
        (None, Ok(_)) => state.update_data(&account.id, |data| {
            let message = match &outcome {
                Some(ApiOutcome::Unauthorized) => {
                    "Codex 로그인이 만료됐어요. Codex 를 한 번 실행하면 갱신돼요.".to_string()
                }
                Some(ApiOutcome::Failed(message)) => message.clone(),
                _ => "Codex 사용량을 가져오지 못했어요".to_string(),
            };
            data.status = Some(AccountStatus::Error { message });
        }),
    }
}

/// 지금 가진 데이터로 스냅샷을 다시 만들어 메뉴바와 화면에 알린다. (네트워크 없음)
pub fn publish(app: &AppHandle) -> Snapshot {
    let state = app.state::<AppState>();
    let now = Utc::now();
    let snapshot = if state.demo {
        mock::snapshot(state.started_at, now)
    } else {
        let data = state.account_data.lock().unwrap().clone();
        let accounts = state.accounts();
        let trends = trends(&state, &accounts, &data, now);
        build_snapshot(&accounts, &data, &trends, now)
    };
    *state.snapshot.lock().unwrap() = snapshot.clone();
    update_menubar(app);
    let _ = app.emit("snapshot-updated", &snapshot);
    snapshot
}

/// 계정 목록과 조회 결과로 화면에 보낼 스냅샷을 만든다.
pub fn build_snapshot(
    accounts: &[Account],
    data: &HashMap<String, AccountData>,
    trends: &HashMap<String, Vec<TrendPoint>>,
    now: DateTime<Utc>,
) -> Snapshot {
    let accounts = accounts
        .iter()
        .map(|account| {
            let data = data.get(&account.id).cloned().unwrap_or_default();
            let usage = data.usage.unwrap_or_default();
            let status = data.status.unwrap_or(
                if account.provider == Provider::Claude && account.org_id.is_none() {
                    AccountStatus::NeedsLogin
                } else {
                    AccountStatus::Loading
                },
            );
            AccountUsage {
                id: account.id.clone(),
                provider: account.provider,
                name: account.name.clone(),
                label: account.label.clone(),
                color: account.color.clone(),
                plan: data.plan.clone().or_else(|| account.plan.clone()),
                weekly: usage.weekly.map(|window| {
                    UsageWindow::new(window.percent, window.resets_at, WEEK_SECONDS, now)
                }),
                session: usage.session.map(|window| {
                    UsageWindow::new(window.percent, window.resets_at, SESSION_SECONDS, now)
                }),
                models: usage.models,
                extra: data.extra,
                trend: trends.get(&account.id).cloned().unwrap_or_default(),
                status,
                updated_at: data.updated_at,
            }
        })
        .collect();

    Snapshot {
        accounts,
        // Claude Code 로그, API 비용, 서비스 상태는 M3·M5 에서 붙인다
        summary: Summary {
            claude_code_today: None,
            api_month_cost_usd: None,
            services: Vec::new(),
        },
        updated_at: now,
    }
}

/// 현재 스냅샷과 설정으로 메뉴바 아이콘을 다시 그린다.
pub fn update_menubar(app: &AppHandle) {
    let state = app.state::<AppState>();
    let model = MenubarModel::from_snapshot(
        &state.snapshot.lock().unwrap(),
        &state.settings.lock().unwrap(),
    );
    let Some(tray) = app.tray_by_id(TRAY_ID) else {
        return;
    };

    #[cfg(target_os = "macos")]
    {
        // Tauri 가 감싸고 있는 실제 macOS 메뉴바 객체를 메인 스레드에서 꺼내 그림을 바꿔 끼운다.
        let _ = tray.with_inner_tray_icon(move |inner| {
            if let (Some(status_item), Some(mtm)) =
                (inner.ns_status_item(), objc2::MainThreadMarker::new())
            {
                crate::menubar::macos::apply(&status_item, &model, mtm);
            }
        });
    }

    // 툴팁을 다시 넣으면 Tauri 가 클릭 영역을 새 아이콘 크기에 맞춰 준다.
    let _ = tray.set_tooltip(Some("AI Usage"));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{RawWindow, UsageReading};
    use chrono::Duration;

    #[test]
    fn accounts_without_data_show_login_or_loading() {
        let mut logged_out = Account::new_claude(&[]);
        logged_out.org_id = None;
        let mut logged_in = Account::new_claude(std::slice::from_ref(&logged_out));
        logged_in.org_id = Some("org".into());

        let snapshot = build_snapshot(
            &[logged_out, logged_in],
            &HashMap::new(),
            &HashMap::new(),
            Utc::now(),
        );
        assert_eq!(snapshot.accounts[0].status, AccountStatus::NeedsLogin);
        assert_eq!(snapshot.accounts[1].status, AccountStatus::Loading);
        assert!(snapshot.accounts[1].weekly.is_none());
    }

    #[test]
    fn usage_becomes_windows_with_pace() {
        let now = Utc::now();
        let mut account = Account::new_claude(&[]);
        account.org_id = Some("org".into());
        let mut data = HashMap::new();
        data.insert(
            account.id.clone(),
            AccountData {
                usage: Some(UsageReading {
                    // 주간 창이 절반 지났는데 80% 사용 → 빨강
                    weekly: Some(RawWindow {
                        percent: 80.0,
                        resets_at: Some(now + Duration::seconds(WEEK_SECONDS / 2)),
                    }),
                    session: None,
                    models: vec![],
                }),
                status: Some(AccountStatus::Ok),
                updated_at: Some(now),
                ..AccountData::default()
            },
        );
        let snapshot = build_snapshot(&[account], &data, &HashMap::new(), now);
        let weekly = snapshot.accounts[0].weekly.as_ref().unwrap();
        assert!((weekly.elapsed_percent.unwrap() - 50.0).abs() < 0.1);
        assert!(weekly.ahead_of_pace);
    }
}
