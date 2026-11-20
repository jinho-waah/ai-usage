//! 계정별 claude.ai 웹뷰를 관리한다.
//!
//! 계정 하나에 웹뷰 창 하나를 둔다.
//! - 로그인할 때는 창을 보여 주고 claude.ai 로그인 페이지를 연다.
//! - 로그인이 끝나면 창을 숨기고, 숨은 채로 1분마다 사용량을 조회한다.
//! - 웹뷰마다 저장소(쿠키)를 따로 써서 계정끼리 로그인이 섞이지 않는다. (macOS 14+)
//!
//! 사용량 조회는 그 웹뷰 안에서 `fetch` 로 한다. 실제 브라우저 엔진이 요청을 보내므로
//! 원본 앱이 겪은 Cloudflare 오류(앱이 직접 요청을 보낼 때 생김)를 줄일 수 있다.

use std::time::Duration;

use chrono::Utc;
use tauri::{
    AppHandle, Emitter, Manager, Runtime, WebviewUrl, WebviewWindow, WebviewWindowBuilder,
};

use crate::accounts::Account;
use crate::claude::{self, FetchOutcome, Organization};
use crate::model::{AccountStatus, Provider};
use crate::state::AppState;
use crate::{popover, refresh};

/// 로그에 쓸 짧은 계정 ID (앞 8글자)
fn short_id(id: &str) -> &str {
    id.get(..8).unwrap_or(id)
}

const LOGIN_URL: &str = "https://claude.ai/login";
/// 로그인 뒤 조회용으로 띄워 두는 가벼운 페이지. claude.ai 주소여야 같은 출처로 fetch 할 수 있다.
const IDLE_URL: &str = "https://claude.ai/robots.txt";
/// 로그인 창이 열려 있는 동안 로그인됐는지 확인하는 간격
const LOGIN_POLL_INTERVAL: Duration = Duration::from_secs(2);
/// 로그인 창을 이만큼 오래 열어 두면 확인을 멈춘다
const LOGIN_POLL_LIMIT: u32 = 900; // 2초 × 900 = 30분
/// Extra usage 는 자주 바뀌지 않으므로 10분마다만 확인한다
const EXTRA_USAGE_INTERVAL_MINUTES: i64 = 10;
/// 플랜(업그레이드·이름 변경)은 6시간마다 다시 확인한다
const PLAN_CHECK_INTERVAL_HOURS: i64 = 6;

/// 계정의 웹뷰 창을 가져온다. 없으면 만든다. (`for_login` 이면 보이는 로그인 창으로)
pub fn ensure_window(
    app: &AppHandle,
    account: &Account,
    for_login: bool,
) -> Result<WebviewWindow, String> {
    if let Some(window) = app.get_webview_window(&account.window_label()) {
        return Ok(window);
    }
    let url = if for_login { LOGIN_URL } else { IDLE_URL };
    let builder = WebviewWindowBuilder::new(
        app,
        account.window_label(),
        WebviewUrl::External(url.parse().map_err(|error| format!("{error}"))?),
    )
    .title(format!("Claude 로그인 — {}", account.name))
    .inner_size(480.0, 720.0)
    .center()
    .visible(for_login);

    // 계정마다 쿠키 저장소를 분리한다. 계정 ID(UUID)가 저장소 이름이다.
    #[cfg(target_os = "macos")]
    let builder = match account.data_store_id() {
        Some(id) => builder.data_store_identifier(id),
        None => builder,
    };

    builder.build().map_err(|error| error.to_string())
}

/// 계정 하나의 사용량을 조회해서 상태에 저장한다. (메뉴바·화면 알림은 호출한 쪽이 한다)
pub async fn refresh_account(app: &AppHandle, account: &Account) {
    if account.provider != Provider::Claude {
        return;
    }
    let state = app.state::<AppState>();
    // 로그인 창이 열려 있는 동안에는 사용자가 쓰는 창을 건드리지 않는다.
    if state.is_logging_in(&account.id) {
        return;
    }
    let Some(org_id) = account.org_id.clone() else {
        state.update_data(&account.id, |data| {
            data.status = Some(AccountStatus::NeedsLogin)
        });
        return;
    };

    let window = match ensure_window(app, account, false) {
        Ok(window) => window,
        Err(message) => return set_error(&state, &account.id, message),
    };
    if let Err(message) = ensure_ready(&window).await {
        return set_error(&state, &account.id, message);
    }

    let now = Utc::now();
    let usage_path = format!("/api/organizations/{org_id}/usage");
    let mut outcome = fetch(&window, &usage_path).await;
    // 페이지가 막 바뀌는 중이라 요청이 끊겼으면 한 번만 다시 시도한다.
    if matches!(&outcome, FetchOutcome::Failed(message) if message.contains("Load failed")) {
        tokio::time::sleep(Duration::from_millis(1500)).await;
        if ensure_ready(&window).await.is_ok() {
            outcome = fetch(&window, &usage_path).await;
        }
    }
    dev_log!("usage {}: {}", short_id(&account.id), describe(&outcome));
    match outcome {
        FetchOutcome::Json(value) => match claude::parse_usage(&value) {
            Ok(usage) => state.update_data(&account.id, |data| {
                dev_log!(
                    "usage {}: weekly={:?} session={:?} models={:?}",
                    short_id(&account.id),
                    usage.weekly.as_ref().map(|w| w.percent),
                    usage.session.as_ref().map(|w| w.percent),
                    usage
                        .models
                        .iter()
                        .map(|m| m.name.as_str())
                        .collect::<Vec<_>>()
                );
                data.usage = Some(usage);
                data.status = Some(AccountStatus::Ok);
                data.updated_at = Some(now);
            }),
            Err(message) => return set_error(&state, &account.id, message),
        },
        FetchOutcome::NeedsLogin => {
            return state.update_data(&account.id, |data| {
                data.status = Some(AccountStatus::NeedsLogin)
            });
        }
        FetchOutcome::Challenge => {
            return state.update_data(&account.id, |data| {
                data.status = Some(AccountStatus::Challenge)
            });
        }
        FetchOutcome::Failed(message) => return set_error(&state, &account.id, message),
    }

    // 플랜 다시 확인 (6시간마다, 앱을 켠 뒤 첫 조회에서도)
    let plan_due = state
        .data(&account.id)
        .plan_checked_at
        .is_none_or(|checked_at| {
            now - checked_at >= chrono::Duration::hours(PLAN_CHECK_INTERVAL_HOURS)
        });
    if plan_due {
        if let FetchOutcome::Json(value) = fetch(&window, "/api/organizations").await {
            let organizations = claude::parse_organizations(&value);
            let current = organizations
                .iter()
                .find(|organization| organization.uuid == org_id);
            if let Some(plan) = current.map(claude::plan_name)
                && plan != account.plan
                && let Ok(accounts) = state.update_accounts(|accounts| {
                    if let Some(stored) = accounts.iter_mut().find(|stored| stored.id == account.id)
                    {
                        stored.plan = plan.clone();
                    }
                })
            {
                let _ = app.emit("accounts-changed", &accounts);
            }
        }
        state.update_data(&account.id, |data| data.plan_checked_at = Some(now));
    }

    // Extra usage (10분마다)
    let extra_due = state
        .data(&account.id)
        .extra_checked_at
        .is_none_or(|checked_at| {
            now - checked_at >= chrono::Duration::minutes(EXTRA_USAGE_INTERVAL_MINUTES)
        });
    if extra_due {
        let extra = match fetch(
            &window,
            &format!("/api/organizations/{org_id}/overage_spend_limit"),
        )
        .await
        {
            FetchOutcome::Json(value) => claude::parse_overage(&value),
            _ => None,
        };
        state.update_data(&account.id, |data| {
            data.extra = extra;
            data.extra_checked_at = Some(now);
        });
    }
}

/// 로그용 요약. 응답 본문은 남기지 않는다.
fn describe(outcome: &FetchOutcome) -> String {
    match outcome {
        FetchOutcome::Json(_) => "ok (json)".to_string(),
        FetchOutcome::NeedsLogin => "needs login (401/403)".to_string(),
        FetchOutcome::Challenge => "cloudflare challenge".to_string(),
        FetchOutcome::Failed(message) => format!("failed: {message}"),
    }
}

fn set_error(state: &AppState, account_id: &str, message: String) {
    state.update_data(account_id, |data| {
        data.status = Some(AccountStatus::Error { message })
    });
}

/// 로그인 창을 열고, 로그인이 끝날 때까지 2초마다 확인한다.
pub fn start_login(app: AppHandle, account_id: String) {
    tauri::async_runtime::spawn(async move {
        let state = app.state::<AppState>();
        let Some(account) = state
            .accounts()
            .into_iter()
            .find(|account| account.id == account_id)
        else {
            return;
        };
        // Codex 는 앱 안 로그인 창이 없다 (Codex CLI 로그인을 읽기만 한다)
        if account.provider != Provider::Claude {
            return;
        }
        let window = match ensure_window(&app, &account, true) {
            Ok(window) => window,
            Err(message) => return set_error(&state, &account_id, message),
        };

        state.logging_in.lock().unwrap().insert(account_id.clone());
        let _ = window.set_title(&format!("Claude 로그인 — {}", account.name));
        if let Ok(url) = LOGIN_URL.parse() {
            let _ = window.navigate(url);
        }
        let _ = window.show();
        let _ = window.set_focus();

        let mut logged_in = false;
        for _ in 0..LOGIN_POLL_LIMIT {
            tokio::time::sleep(LOGIN_POLL_INTERVAL).await;
            // 창을 닫으면(=숨기면) 로그인을 그만둔 것으로 본다.
            if !window.is_visible().unwrap_or(false) {
                break;
            }
            // 계정이 삭제됐으면 멈춘다.
            if !state
                .accounts()
                .iter()
                .any(|account| account.id == account_id)
            {
                break;
            }
            let outcome = fetch(&window, "/api/organizations").await;
            dev_log!("login {}: {}", short_id(&account_id), describe(&outcome));
            if let FetchOutcome::Json(value) = outcome {
                let organizations = claude::parse_organizations(&value);
                dev_log!(
                    "login {}: {} organization(s), capabilities={:?}, tiers={:?}",
                    short_id(&account_id),
                    organizations.len(),
                    organizations
                        .iter()
                        .map(|o| o.capabilities.clone())
                        .collect::<Vec<_>>(),
                    organizations
                        .iter()
                        .map(|o| o.rate_limit_tier.clone())
                        .collect::<Vec<_>>()
                );
                if !organizations.is_empty() {
                    logged_in = complete_login(&app, &account_id, &organizations);
                    break;
                }
            }
        }

        state.logging_in.lock().unwrap().remove(&account_id);
        if !logged_in {
            refresh::publish(&app);
            return;
        }

        // 로그인 창을 숨기고 조회용 페이지로 바꾼 뒤, 바로 한 번 조회해서 팝오버로 보여 준다.
        let _ = window.hide();
        if let Ok(url) = IDLE_URL.parse() {
            let _ = window.navigate(url);
        }
        state.update_data(&account_id, |data| {
            data.status = Some(AccountStatus::Loading)
        });
        refresh::publish(&app);
        if let Some(account) = state
            .accounts()
            .into_iter()
            .find(|account| account.id == account_id)
        {
            refresh_account(&app, &account).await;
        }
        refresh::publish(&app);
        popover::show_again(&app);
    });
}

/// 로그인 확인: 조직을 고르고 계정 정보를 저장한다. 처음 로그인하면 이름·라벨도 채워 준다.
fn complete_login(app: &AppHandle, account_id: &str, organizations: &[Organization]) -> bool {
    let state = app.state::<AppState>();
    let result = state.update_accounts(|accounts| {
        let Some(account) = accounts.iter_mut().find(|account| account.id == account_id) else {
            return;
        };
        let Some(organization) =
            claude::preferred_organization(organizations, account.org_id.as_deref())
        else {
            return;
        };
        let first_login = account.org_id.is_none();
        account.org_id = Some(organization.uuid.clone());
        account.org_name = Some(organization.name.clone());
        account.plan = claude::plan_name(organization);

        // 사용자가 아직 이름을 바꾸지 않았다면 조직 이름으로 기본 이름을 만든다 (예: "jinho").
        if first_login
            && account.name.starts_with("Claude ")
            && let Some(name) = claude::default_account_name(&organization.name)
        {
            account.name = name;
        }
    });

    match result {
        Ok(accounts) => {
            let saved = accounts
                .iter()
                .any(|account| account.id == account_id && account.org_id.is_some());
            let _ = app.emit("accounts-changed", &accounts);
            saved
        }
        Err(message) => {
            set_error(
                &state,
                account_id,
                format!("계정을 저장하지 못했어요: {message}"),
            );
            false
        }
    }
}

/// 조회용 페이지(claude.ai/robots.txt)가 다 열릴 때까지 기다린다. 다른 페이지면 그리로 옮긴다.
///
/// 페이지를 옮기는 중에 조회를 보내면 이전 페이지가 닫히면서 요청이 끊긴다(TypeError: Load failed).
/// 그래서 주소의 경로까지 확인해서, 새 페이지가 완전히 열린 뒤에만 조회한다.
async fn ensure_ready<R: Runtime>(window: &WebviewWindow<R>) -> Result<(), String> {
    const READY: &str = "claude.ai|/robots.txt|complete";
    if run_js(window, claude::READY_SCRIPT, &[])
        .await
        .ok()
        .as_deref()
        == Some(READY)
    {
        return Ok(());
    }
    let url = IDLE_URL.parse().map_err(|error| format!("{error}"))?;
    window.navigate(url).map_err(|error| error.to_string())?;
    for _ in 0..40 {
        tokio::time::sleep(Duration::from_millis(500)).await;
        if run_js(window, claude::READY_SCRIPT, &[])
            .await
            .ok()
            .as_deref()
            == Some(READY)
        {
            return Ok(());
        }
    }
    Err("claude.ai 페이지가 열리지 않아요".to_string())
}

/// 웹뷰 안에서 claude.ai API 를 가져온다.
async fn fetch<R: Runtime>(window: &WebviewWindow<R>, path: &str) -> FetchOutcome {
    let url = format!("{}{path}", claude::BASE_URL);
    match run_js(window, claude::FETCH_SCRIPT, &[("url", &url)]).await {
        Ok(text) => match serde_json::from_str::<claude::FetchResult>(&text) {
            Ok(result) => claude::classify(&result),
            Err(error) => FetchOutcome::Failed(format!("웹뷰 결과를 읽지 못했어요 ({error})")),
        },
        Err(message) => FetchOutcome::Failed(message),
    }
}

#[cfg(target_os = "macos")]
async fn run_js<R: Runtime>(
    window: &WebviewWindow<R>,
    body: &str,
    arguments: &[(&str, &str)],
) -> Result<String, String> {
    crate::webview_js::call(window, body, arguments).await
}

#[cfg(not(target_os = "macos"))]
async fn run_js<R: Runtime>(
    _window: &WebviewWindow<R>,
    _body: &str,
    _arguments: &[(&str, &str)],
) -> Result<String, String> {
    Err("macOS 에서만 지원해요".to_string())
}

/// 계정을 지울 때: 웹뷰 창을 닫고 그 계정의 쿠키 저장소를 지운다.
pub async fn forget_account(app: &AppHandle, account: &Account) {
    if let Some(window) = app.get_webview_window(&account.window_label()) {
        let _ = window.destroy();
    }
    #[cfg(target_os = "macos")]
    if let Some(id) = account.data_store_id() {
        // 웹뷰가 완전히 닫힌 뒤에야 저장소를 지울 수 있어서 잠깐 기다린다.
        tokio::time::sleep(Duration::from_millis(500)).await;
        let _ = app.remove_data_store(id).await;
    }
}
