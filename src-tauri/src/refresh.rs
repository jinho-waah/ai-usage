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
        crate::alerts::check_limits(app);
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

/// 팝오버 미니 그래프가 보여 주는 기간 (일)
const TREND_DAYS: i64 = 7;

/// 팝오버 미니 그래프 하나: 점들과 가로축 범위
#[derive(Debug, Clone, Default)]
pub struct CycleTrend {
    pub points: Vec<TrendPoint>,
    /// 가로축 시작과 끝 (최근 7일)
    pub start: Option<DateTime<Utc>>,
    pub end: Option<DateTime<Utc>>,
}

/// 팝오버 미니 그래프: 최근 7일 동안 쓴 양(누적). 리셋 때 0% 로 떨어졌다가 다시 쌓이는 모양이 보인다.
/// 주간 한도가 있으면 주간, 없으면 5시간 세션.
fn trends(
    state: &AppState,
    accounts: &[Account],
    data: &HashMap<String, AccountData>,
    now: DateTime<Utc>,
) -> HashMap<String, CycleTrend> {
    let Some(history) = &state.history else {
        return HashMap::new();
    };
    let from = now - chrono::Duration::days(TREND_DAYS);
    accounts
        .iter()
        .map(|account| {
            let usage = data.get(&account.id).and_then(|data| data.usage.as_ref());
            let (window, live) = match usage {
                Some(usage) if usage.weekly.is_some() => {
                    (WindowKind::Weekly, usage.weekly.as_ref())
                }
                Some(usage) if usage.session.is_some() => {
                    (WindowKind::Session, usage.session.as_ref())
                }
                _ => (WindowKind::Weekly, None),
            };
            // 리셋 시각을 정확히 나누려고 그래프 기간보다 2주 더 앞부터 읽는다
            let samples = history
                .samples(
                    &account.id,
                    window,
                    Some(from - chrono::Duration::days(14)),
                    now,
                )
                .unwrap_or_default();
            let trend = CycleTrend {
                points: recent_points(&samples, window, live, from, now),
                start: Some(from),
                end: Some(now),
            };
            (account.id.clone(), trend)
        })
        .collect()
}

/// 최근 기간(`from` ~ 지금)의 그래프 점.
/// - 주간 한도는 주기를 나눠서 리셋 시각에 0% 로 떨어지게 그린다.
/// - 끝은 지금의 실제 값 (막대와 같은 값을 쓴 양으로).
pub fn recent_points(
    samples: &[crate::history::Sample],
    window: WindowKind,
    live: Option<&crate::model::RawWindow>,
    from: DateTime<Utc>,
    now: DateTime<Utc>,
) -> Vec<TrendPoint> {
    let mut points = match window {
        WindowKind::Weekly => {
            let live_reset = live.and_then(|window| window.resets_at);
            let spans = crate::history_view::split_cycles(samples, live_reset, now);
            crate::history_view::weekly_series(&spans, from, now).0
        }
        WindowKind::Session => crate::history_view::clip(
            samples
                .iter()
                .map(|sample| TrendPoint {
                    at: sample.at,
                    percent: sample.used_percent,
                })
                .collect(),
            from,
            now,
        ),
    };
    if let Some(live) = live {
        match points.last_mut() {
            Some(last) if last.at == now => last.percent = live.percent,
            _ => points.push(TrendPoint {
                at: now,
                percent: live.percent,
            }),
        }
    }
    points
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
    emit_to_popover(app, "snapshot-updated", &snapshot);
    snapshot
}

/// 팝오버가 화면에 보일 때만 이벤트를 보낸다.
///
/// 닫힌 팝오버의 웹뷰는 macOS 가 재워 둔다. 거기에 이벤트를 보내면 매번 깨어나 화면을 다시 그리려 하고,
/// 그 뒤로 WebKit 의 화면 갱신 타이머가 멈추지 않아 앱이 쉬지 못한다 (쉬는 동안 CPU 의 대부분이 이것이었다).
/// 닫혀 있던 팝오버는 열릴 때 화면 쪽에서 새 데이터를 받아 간다 (App.tsx 의 onPopoverShown).
pub fn emit_to_popover<S: serde::Serialize + Clone>(app: &AppHandle, event: &str, payload: S) {
    let visible = app
        .get_webview_window(crate::popover::LABEL)
        .is_some_and(|window| window.is_visible().unwrap_or(false));
    if visible {
        let _ = app.emit(event, payload);
    }
}

/// 계정 목록과 조회 결과로 화면에 보낼 스냅샷을 만든다.
pub fn build_snapshot(
    accounts: &[Account],
    data: &HashMap<String, AccountData>,
    trends: &HashMap<String, CycleTrend>,
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
                api_cost: data.api_cost,
                trend: trends
                    .get(&account.id)
                    .map(|trend| trend.points.clone())
                    .unwrap_or_default(),
                trend_start: trends.get(&account.id).and_then(|trend| trend.start),
                trend_end: trends.get(&account.id).and_then(|trend| trend.end),
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

    let local_now = chrono::Local::now();
    let (totals, model_sums) = state
        .history
        .as_ref()
        .map(|history| {
            let today = local_now.format("%Y-%m-%d").to_string();
            let month_start = local_now.format("%Y-%m-01").to_string();
            (
                history.token_totals(&today).unwrap_or_default(),
                history
                    .token_sums_by_model(&month_start)
                    .unwrap_or_default(),
            )
        })
        .unwrap_or_default();
    {
        let mut summary = state.summary.lock().unwrap();
        summary.tokens = brand_tokens(totals, &model_sums);
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

/// Admin API 키가 있는 계정마다 이번 달 Claude API 비용을 받아 온다. (1시간마다, 키를 넣었을 때)
pub async fn refresh_api_cost(app: &AppHandle) {
    let state = app.state::<AppState>();
    if state.demo {
        return;
    }
    let keys: Vec<(String, String)> = state
        .admin_keys
        .lock()
        .unwrap()
        .iter()
        .map(|(id, key)| (id.clone(), key.clone()))
        .collect();
    for (account_id, key) in keys {
        refresh_account_api_cost(&state, &account_id, &key).await;
    }
    publish(app);
}

/// 계정 하나의 API 비용 조회
pub async fn refresh_account_api_cost(state: &AppState, account_id: &str, key: &str) {
    use crate::api_cost::{self, CostOutcome};

    let outcome = api_cost::fetch_month_cost(key, chrono::Local::now()).await;
    state.update_data(account_id, |data| match outcome {
        CostOutcome::Ok(cost) => {
            data.api_cost = Some(cost);
            data.api_cost_error = None;
        }
        CostOutcome::Unauthorized => {
            data.api_cost = None;
            data.api_cost_error = Some("Admin API 키가 맞지 않거나 권한이 없어요".to_string());
        }
        // 일시적인 오류면 이전 값을 그대로 둔다
        CostOutcome::Failed(message) => data.api_cost_error = Some(message),
    });
}

/// DB 합계를 화면용으로 바꾼다. Claude 먼저, 그다음 Codex.
/// 모델별 합계에 가격표를 곱해 "API 로 냈다면" 금액도 붙인다.
pub fn brand_tokens(
    totals: Vec<crate::history::BrandTokenTotals>,
    model_sums: &[crate::history::ModelTokenSums],
) -> Vec<BrandTokens> {
    use crate::pricing::{self, TokenCounts};

    let mut brands: Vec<BrandTokens> = totals
        .into_iter()
        .filter_map(|total| {
            let provider = match total.provider.as_str() {
                "claude" => Provider::Claude,
                "codex" => Provider::Codex,
                _ => return None,
            };
            // 이 브랜드의 모델들에 가격을 매긴다. 가격표에 없는 모델은 빼고 토큰 수만 따로 센다.
            let mut cost_total = 0.0;
            let mut cost_month = 0.0;
            let mut priced_any = false;
            let mut unpriced_tokens = 0;
            for sums in model_sums
                .iter()
                .filter(|sums| sums.provider == total.provider)
            {
                let Some(price) = pricing::price_for(&sums.model) else {
                    unpriced_tokens += sums.total;
                    continue;
                };
                let cost = pricing::cost_usd(
                    price,
                    &TokenCounts {
                        input: sums.input,
                        output: sums.output,
                        cache_write_5m: sums.cache_write_5m,
                        cache_write_1h: sums.cache_write_1h,
                        cache_read: sums.cache_read,
                    },
                );
                priced_any = true;
                cost_total += cost;
                if sums.this_month {
                    cost_month += cost;
                }
            }
            Some(BrandTokens {
                api_cost_total_usd: priced_any.then_some(cost_total),
                api_cost_month_usd: priced_any.then_some(cost_month),
                unpriced_tokens,
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

    // 그릴 내용이 지난번과 같으면 아무것도 하지 않는다.
    // (메뉴바 항목을 한 번 바꿀 때마다 macOS 가 메뉴바용 사본을 다시 그려서 수십 ms 가 든다)
    {
        let mut last = state.menubar_model.lock().unwrap();
        if last.as_ref() == Some(&model) {
            return;
        }
        *last = Some(model.clone());
    }

    #[cfg(target_os = "macos")]
    {
        // Tauri 가 감싸고 있는 실제 macOS 메뉴바 객체를 메인 스레드에서 꺼내 그림을 바꿔 끼운다.
        let _ = tray.with_inner_tray_icon(move |inner| {
            if let (Some(status_item), Some(mtm)) =
                (inner.ns_status_item(), objc2::MainThreadMarker::new())
            {
                crate::menubar::macos::apply(&status_item, &model, mtm);
            }
            // 툴팁을 다시 넣으면 클릭 영역이 새 아이콘 크기에 맞춰진다. (같은 자리에서 해서 메인 스레드를 한 번만 다녀온다)
            let _ = inner.set_tooltip(Some("AI Usage"));
        });
    }

    #[cfg(not(target_os = "macos"))]
    let _ = tray.set_tooltip(Some("AI Usage"));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{RawWindow, UsageReading};
    use chrono::Duration;

    #[test]
    fn mini_trend_shows_the_last_week_with_resets_dropping_to_zero() {
        use crate::history::Sample;
        use crate::model::RawWindow;
        let now = Utc::now();
        let reset = now - Duration::days(3);
        let next_reset = reset + Duration::days(7);
        let sample = |days_ago: i64, percent: f64, resets_at| Sample {
            at: now - Duration::days(days_ago),
            used_percent: percent,
            resets_at: Some(resets_at),
        };
        let samples = [
            sample(6, 70.0, reset),
            sample(4, 98.0, reset),
            sample(2, 20.0, next_reset),
            sample(1, 35.0, next_reset),
        ];
        let live = RawWindow {
            percent: 41.0,
            resets_at: Some(next_reset),
        };
        let points = recent_points(
            &samples,
            WindowKind::Weekly,
            Some(&live),
            now - Duration::days(7),
            now,
        );
        let values: Vec<f64> = points.iter().map(|point| point.percent).collect();
        // 쌓이다가(70 → 98) 리셋 시각에 0%, 다시 쌓여(20 → 35) 지금 값(41)으로 끝난다
        assert_eq!(values, vec![70.0, 98.0, 0.0, 20.0, 35.0, 41.0]);
        assert_eq!(points[2].at, reset);
        assert_eq!(points.last().unwrap().at, now);
    }

    #[test]
    fn api_equivalent_cost_prices_known_models_and_counts_the_rest() {
        use crate::history::{BrandTokenTotals, ModelTokenSums};
        let totals = vec![BrandTokenTotals {
            provider: "claude".into(),
            input: 0,
            output: 3_000_000,
            cache_write: 0,
            cache_read: 0,
            reasoning: 0,
            total: 3_000_500,
            today: 0,
            since: None,
        }];
        let sums = |model: &str, this_month: bool, output: u64, total: u64| ModelTokenSums {
            provider: "claude".into(),
            model: model.into(),
            this_month,
            input: 0,
            output,
            cache_write_5m: 0,
            cache_write_1h: 0,
            cache_read: 0,
            total,
        };
        let brands = brand_tokens(
            totals,
            &[
                sums("claude-opus-5-5", true, 1_000_000, 1_000_000), // 이번 달 $20
                sums("claude-opus-5-5", false, 2_000_000, 2_000_000), // 지난달 $40
                sums("some-new-model", true, 500, 500),              // 가격 모름
            ],
        );
        assert_eq!(brands.len(), 1);
        assert_eq!(brands[0].api_cost_month_usd, Some(20.0));
        assert_eq!(brands[0].api_cost_total_usd, Some(60.0));
        assert_eq!(brands[0].unpriced_tokens, 500);
    }

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
