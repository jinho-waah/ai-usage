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
    AccountStatus, AccountUsage, BrandTokens, Provider, Snapshot, Summary, TrendPoint, UsageWindow,
};
use crate::state::{AccountData, AppState};
use crate::{claude_web, mock, tokens};

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
        crate::alerts::check_logins(app);
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
        let sample = |window: &crate::model::RawWindow| crate::history::Sample {
            at,
            used_percent: window.percent,
            resets_at: window.resets_at,
        };
        if let Some(window) = &usage.weekly {
            let _ = history.record(&account_id, WindowKind::Weekly, &sample(window), "live");
        }
        if let Some(window) = &usage.session {
            let _ = history.record(&account_id, WindowKind::Session, &sample(window), "live");
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
        let summary = state.summary.lock().unwrap().clone();
        build_snapshot(&accounts, &data, &trends, summary, now)
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
    summary: Summary,
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
        summary,
        updated_at: now,
    }
}

/// 로컬 로그에서 새로 생긴 토큰 사용량을 모으고, 누적 합계를 요약에 넣어 알린다.
///
/// - 다른 집계가 진행 중이면 그냥 돌아간다 (겹치지 않게).
/// - 마지막 집계 후 `min_gap` 이 안 지났으면 건너뛴다 (팝오버를 자주 열어도 부담 없게).
/// - 처음 한 번은 로그 전체(수 GB)를 읽어서 몇 분 걸릴 수 있다. 그동안 화면에는 "집계 중"이 보인다.
pub async fn refresh_tokens(app: &AppHandle, min_gap: std::time::Duration) {
    let state = app.state::<AppState>();
    if state.demo || state.history.is_none() {
        return;
    }
    let Ok(mut last_scan) = state.token_scan.try_lock() else {
        return;
    };
    if last_scan.is_some_and(|at| at.elapsed() < min_gap) {
        return;
    }

    state.summary.lock().unwrap().tokens_scanning = true;
    publish(app);

    let db_path = state.db_path.clone();
    let started = std::time::Instant::now();
    let result = tauri::async_runtime::spawn_blocking(move || {
        tokens::scan(&db_path, &tokens::LogDirs::default_locations())
    })
    .await
    .map_err(|error| error.to_string())
    .and_then(|result| result);
    match &result {
        Ok(report) => dev_log!(
            "토큰 집계 끝: 파일 {}개, Claude {}건, Codex {}건, {:.1}초",
            report.files_read,
            report.claude_records,
            report.codex_records,
            started.elapsed().as_secs_f64()
        ),
        Err(error) => eprintln!("[ai-usage] 토큰 집계 실패: {error}"),
    }
    *last_scan = Some(std::time::Instant::now());

    let totals = state
        .history
        .as_ref()
        .and_then(|history| {
            let today = chrono::Local::now().format("%Y-%m-%d").to_string();
            history.token_totals(&today).ok()
        })
        .unwrap_or_default();
    {
        let mut summary = state.summary.lock().unwrap();
        summary.tokens = brand_tokens(totals);
        summary.tokens_scanning = false;
    }
    publish(app);
}

/// Claude·OpenAI 상태 페이지를 확인한다. 그 제공자의 계정이 있을 때만. (5분마다)
pub async fn refresh_services(app: &AppHandle) {
    let state = app.state::<AppState>();
    if state.demo {
        return;
    }
    let providers: Vec<Provider> = state
        .accounts()
        .iter()
        .map(|account| account.provider)
        .collect();
    let mut statuses = Vec::new();
    for source in crate::services::SOURCES
        .iter()
        .filter(|source| providers.contains(&source.provider))
    {
        let report = crate::services::fetch(source).await;
        crate::alerts::service_checked(app, source.name, &report);
        statuses.push(crate::model::ServiceStatus {
            name: source.name.to_string(),
            state: report.state,
        });
    }
    state.summary.lock().unwrap().services = statuses;
    publish(app);
}

/// 이번 달 Claude API 비용을 받아 온다. Admin API 키가 없으면 비용 칸을 비운다. (1시간마다)
pub async fn refresh_api_cost(app: &AppHandle) {
    use crate::api_cost::{self, CostOutcome};

    let state = app.state::<AppState>();
    if state.demo {
        return;
    }
    let Some(key) = state.admin_key.lock().unwrap().clone() else {
        state.summary.lock().unwrap().api_cost = None;
        *state.api_cost_error.lock().unwrap() = None;
        publish(app);
        return;
    };
    match api_cost::fetch_month_cost(&key, chrono::Local::now()).await {
        CostOutcome::Ok(cost) => {
            state.summary.lock().unwrap().api_cost = Some(cost);
            *state.api_cost_error.lock().unwrap() = None;
        }
        CostOutcome::Unauthorized => {
            state.summary.lock().unwrap().api_cost = None;
            *state.api_cost_error.lock().unwrap() =
                Some("Admin API 키가 맞지 않거나 권한이 없어요".to_string());
        }
        // 일시적인 오류면 이전 값을 그대로 둔다
        CostOutcome::Failed(message) => {
            *state.api_cost_error.lock().unwrap() = Some(message);
        }
    }
    publish(app);
}

/// DB 합계를 화면용으로 바꾼다. Claude 먼저, 그다음 Codex.
fn brand_tokens(totals: Vec<crate::history::BrandTokenTotals>) -> Vec<BrandTokens> {
    let mut brands: Vec<BrandTokens> = totals
        .into_iter()
        .filter_map(|total| {
            let provider = match total.provider.as_str() {
                "claude" => Provider::Claude,
                "codex" => Provider::Codex,
                _ => return None,
            };
            Some(BrandTokens {
                provider,
                total: total.total,
                input: total.input,
                output: total.output,
                cache_write: total.cache_write,
                cache_read: total.cache_read,
                reasoning: total.reasoning,
                today: total.today,
                since: total.since,
            })
        })
        .collect();
    brands.sort_by_key(|brand| brand.provider != Provider::Claude);
    brands
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
            Summary::default(),
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
        let snapshot = build_snapshot(&[account], &data, &HashMap::new(), Summary::default(), now);
        let weekly = snapshot.accounts[0].weekly.as_ref().unwrap();
        assert!((weekly.elapsed_percent.unwrap() - 50.0).abs() < 0.1);
        assert!(weekly.ahead_of_pace);
    }
}
