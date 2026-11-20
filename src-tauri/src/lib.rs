//! 앱의 시작점.
//!
//! 하는 일
//! 1. 앱 상태(계정 목록, 설정, 조회 결과)를 만든다.
//! 2. 메뉴바 아이콘을 만들고, 누르면 팝오버가 열리게 연결한다.
//! 3. 1분마다 모든 계정의 사용량을 새로 조회해서 메뉴바와 팝오버에 알린다.
//! 4. React 화면이 부를 수 있는 명령(`#[tauri::command]`)을 등록한다.

/// 개발 빌드(`npm run tauri dev`)에서만 터미널에 진단 로그를 남긴다. 이메일·조직 이름 같은 개인정보는 넣지 않는다.
/// (여기, 모듈 선언보다 위에 두면 모든 모듈에서 `dev_log!` 로 쓸 수 있다)
macro_rules! dev_log {
    ($($arg:tt)*) => {
        if cfg!(debug_assertions) {
            eprintln!("[ai-usage] {}", format!($($arg)*));
        }
    };
}

mod accounts;
mod alerts;
mod api_cost;
mod claude;
mod claude_web;
mod codex;
mod history;
mod history_view;
mod legacy;
mod menubar;
mod mock;
mod model;
mod popover;
mod refresh;
mod services;
mod settings;
mod state;
mod tokens;
#[cfg(target_os = "macos")]
mod webview_js;

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::Duration;

use chrono::Utc;
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Emitter, Manager, Window, WindowEvent};

use crate::accounts::{Account, AccountEdit};
use crate::history::{History, WindowKind};
use crate::model::{Provider, Snapshot, Summary};
use crate::refresh::TRAY_ID;
use crate::settings::Settings;
use crate::state::AppState;

/// 구독 사용량 갱신 주기 (스펙: 1분 고정)
const REFRESH_INTERVAL: Duration = Duration::from_secs(60);
/// 로컬 로그 토큰 집계 간격
const TOKEN_SCAN_INTERVAL: Duration = Duration::from_secs(5 * 60);
/// 서비스 상태 페이지 확인 간격
const SERVICE_INTERVAL: Duration = Duration::from_secs(5 * 60);
/// API 비용 조회 간격 (데이터가 몇 시간 늦게 반영되므로 자주 볼 필요 없다)
const API_COST_INTERVAL: Duration = Duration::from_secs(60 * 60);

// ───────── React 화면에서 부르는 명령 ─────────
// `#[tauri::command]` 가 붙은 함수는 화면에서 `invoke("함수이름")` 으로 부를 수 있다.
// `async` 명령은 화면을 멈추지 않고 뒤에서 실행된다.

#[tauri::command]
fn get_snapshot(app: AppHandle) -> Snapshot {
    app.state::<AppState>().snapshot.lock().unwrap().clone()
}

#[tauri::command]
async fn refresh_now(app: AppHandle) -> Snapshot {
    refresh::refresh_all(&app).await
}

#[tauri::command]
fn get_settings(app: AppHandle) -> Settings {
    app.state::<AppState>().settings.lock().unwrap().clone()
}

#[tauri::command]
fn set_settings(app: AppHandle, settings: Settings) -> Result<(), String> {
    let state = app.state::<AppState>();
    settings::save(&state.settings_path, &settings)?;
    *state.settings.lock().unwrap() = settings.clone();
    refresh::update_menubar(&app);
    let _ = app.emit("settings-changed", &settings);
    Ok(())
}

#[tauri::command]
fn get_accounts(app: AppHandle) -> Vec<Account> {
    app.state::<AppState>().accounts()
}

/// Claude 계정을 새로 만들고 로그인 창을 연다.
#[tauri::command]
fn add_claude_account(app: AppHandle) -> Result<Account, String> {
    let state = app.state::<AppState>();
    let mut added = None;
    let accounts = state.update_accounts(|accounts| {
        let account = Account::new_claude(accounts);
        added = Some(account.clone());
        accounts.push(account);
    })?;
    let account = added.ok_or("계정을 만들지 못했어요")?;
    accounts_changed(&app, &accounts);
    claude_web::start_login(app.clone(), account.id.clone());
    Ok(account)
}

/// Codex 를 추가한다. Codex CLI 로그인(~/.codex/auth.json)이 있어야 한다.
#[tauri::command]
async fn add_codex_account(app: AppHandle) -> Result<Account, String> {
    let state = app.state::<AppState>();
    if state
        .accounts()
        .iter()
        .any(|account| account.provider == Provider::Codex)
    {
        return Err("Codex 는 이미 추가돼 있어요".to_string());
    }
    codex::read_credentials(&codex::codex_home())?;

    let account = Account::new_codex(&state.accounts());
    let accounts = state.update_accounts(|accounts| accounts.push(account.clone()))?;
    accounts_changed(&app, &accounts);
    refresh::refresh_codex_account(&app, &account).await;
    refresh::record_history(&state);
    refresh::publish(&app);
    backfill_codex_history(app.clone());
    Ok(account)
}

/// Codex 로그 가져오기를 했는지 기억하는 키. v2: 리셋 시각까지 채워서 다시 가져온다.
const CODEX_IMPORT_KEY: &str = "codex-log-import-v2:";

/// Codex 세션 로그에 남은 지난 한도 기록을 그래프용 기록 DB 로 가져온다. 계정마다 한 번만 한다.
fn backfill_codex_history(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        let state = app.state::<AppState>();
        let Some(history) = &state.history else {
            return;
        };
        let pending: Vec<Account> = state
            .accounts()
            .into_iter()
            .filter(|account| account.provider == Provider::Codex)
            .filter(|account| {
                history
                    .meta(&format!("{CODEX_IMPORT_KEY}{}", account.id))
                    .is_none()
            })
            .collect();
        if pending.is_empty() {
            return;
        }
        // 로그 파일이 클 수 있어서 별도 스레드에서 읽는다
        let logged =
            tauri::async_runtime::spawn_blocking(|| codex::all_logged_usage(&codex::codex_home()))
                .await
                .unwrap_or_default();
        let weekly: Vec<history::Sample> = logged
            .iter()
            .filter_map(|(at, usage)| {
                usage.reading.weekly.as_ref().map(|w| history::Sample {
                    at: *at,
                    used_percent: w.percent,
                    resets_at: w.resets_at,
                })
            })
            .collect();
        let session: Vec<history::Sample> = logged
            .iter()
            .filter_map(|(at, usage)| {
                usage.reading.session.as_ref().map(|w| history::Sample {
                    at: *at,
                    used_percent: w.percent,
                    resets_at: w.resets_at,
                })
            })
            .collect();
        for account in pending {
            let _ = history.import(&account.id, WindowKind::Weekly, &weekly, "codex-log");
            let _ = history.import(&account.id, WindowKind::Session, &session, "codex-log");
            let _ = history.set_meta(
                &format!("{CODEX_IMPORT_KEY}{}", account.id),
                &Utc::now().to_rfc3339(),
            );
        }
        refresh::publish(&app);
    });
}

/// 원본 Claude Usage 앱의 프로필 목록 (기록을 가져올 수 있는 것만)
#[tauri::command]
async fn list_legacy_profiles() -> Vec<legacy::LegacyProfile> {
    tauri::async_runtime::spawn_blocking(legacy::list_profiles)
        .await
        .unwrap_or_default()
}

/// 원본 앱 프로필 하나의 기록을 우리 계정 하나로 가져온다. 가져온 기록 수를 돌려준다.
#[tauri::command]
async fn import_legacy_history(
    app: AppHandle,
    profile_id: String,
    account_id: String,
) -> Result<usize, String> {
    let state = app.state::<AppState>();
    if !state
        .accounts()
        .iter()
        .any(|account| account.id == account_id)
    {
        return Err("계정을 찾지 못했어요".to_string());
    }
    let legacy_history =
        tauri::async_runtime::spawn_blocking(move || legacy::read_history(&profile_id))
            .await
            .map_err(|error| error.to_string())?
            .ok_or("원본 앱 기록을 읽지 못했어요")?;
    let history = state
        .history
        .as_ref()
        .ok_or("기록 저장소를 열지 못했어요")?;
    let mut imported = history.import(
        &account_id,
        WindowKind::Weekly,
        &legacy_history.weekly,
        "claude-usage-app",
    )?;
    imported += history.import(
        &account_id,
        WindowKind::Session,
        &legacy_history.session,
        "claude-usage-app",
    )?;
    refresh::publish(&app);
    Ok(imported)
}

/// 이름·라벨·색을 고친다.
#[tauri::command]
fn update_account(app: AppHandle, edit: AccountEdit) -> Result<(), String> {
    let accounts = app.state::<AppState>().update_accounts(|accounts| {
        if let Some(account) = accounts.iter_mut().find(|account| account.id == edit.id) {
            edit.apply_to(account);
        }
    })?;
    accounts_changed(&app, &accounts);
    Ok(())
}

/// 계정 순서를 위(-1)·아래(+1)로 바꾼다.
#[tauri::command]
fn move_account(app: AppHandle, id: String, offset: i32) -> Result<(), String> {
    let accounts = app
        .state::<AppState>()
        .update_accounts(|accounts| accounts::move_account(accounts, &id, offset))?;
    accounts_changed(&app, &accounts);
    Ok(())
}

/// 로그인 창을 다시 연다. (세션 만료, Cloudflare 확인이 필요할 때)
#[tauri::command]
fn relogin_account(app: AppHandle, id: String) {
    claude_web::start_login(app, id);
}

/// 계정을 지운다. 그 계정의 로그인 쿠키 저장소도 함께 지운다.
#[tauri::command]
async fn remove_account(app: AppHandle, id: String) -> Result<(), String> {
    let state = app.state::<AppState>();
    let removed = state
        .accounts()
        .into_iter()
        .find(|account| account.id == id);
    let accounts = state.update_accounts(|accounts| accounts.retain(|account| account.id != id))?;
    state.account_data.lock().unwrap().remove(&id);
    if let Some(history) = &state.history {
        let _ = history.delete_account(&id);
    }
    accounts_changed(&app, &accounts);
    if let Some(account) = removed {
        claude_web::forget_account(&app, &account).await;
    }
    Ok(())
}

/// 설정 화면의 Claude API 칸에 보여 줄 정보
#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct ApiCostInfo {
    /// 키가 저장돼 있는지
    configured: bool,
    /// 가린 키 ("sk-ant-admin…abcd")
    key_hint: Option<String>,
    error: Option<String>,
    cost: Option<api_cost::MonthCost>,
}

fn api_cost_info(state: &AppState) -> ApiCostInfo {
    let key = state.admin_key.lock().unwrap().clone();
    ApiCostInfo {
        configured: key.is_some(),
        key_hint: key.as_deref().map(api_cost::mask),
        error: state.api_cost_error.lock().unwrap().clone(),
        cost: state.summary.lock().unwrap().api_cost.clone(),
    }
}

#[tauri::command]
fn get_api_cost_info(state: tauri::State<AppState>) -> ApiCostInfo {
    api_cost_info(&state)
}

/// Admin API 키를 확인하고 키체인에 저장한다. 키가 틀리면 저장하지 않는다.
#[tauri::command]
async fn set_admin_key(app: AppHandle, key: String) -> Result<ApiCostInfo, String> {
    let key = key.trim().to_string();
    if !api_cost::looks_like_admin_key(&key) {
        return Err(
            "Admin API 키(sk-ant-admin…)를 넣어 주세요. 일반 API 키로는 비용을 볼 수 없어요"
                .to_string(),
        );
    }
    if let api_cost::CostOutcome::Unauthorized =
        api_cost::fetch_month_cost(&key, chrono::Local::now()).await
    {
        return Err("키가 맞지 않거나 권한이 없어요".to_string());
    }
    api_cost::save_key(&key)?;
    let state = app.state::<AppState>();
    *state.admin_key.lock().unwrap() = Some(key);
    refresh::refresh_api_cost(&app).await;
    Ok(api_cost_info(&state))
}

/// Admin API 키를 키체인에서 지운다.
#[tauri::command]
fn remove_admin_key(app: AppHandle) -> Result<ApiCostInfo, String> {
    api_cost::delete_key()?;
    let state = app.state::<AppState>();
    *state.admin_key.lock().unwrap() = None;
    *state.api_cost_error.lock().unwrap() = None;
    state.summary.lock().unwrap().api_cost = None;
    refresh::publish(&app);
    Ok(api_cost_info(&state))
}

/// 히스토리 창 데이터. `range`: "cycle" | "7d" | "30d" | "all"
#[tauri::command]
fn get_history(
    state: tauri::State<AppState>,
    range: String,
) -> Result<history_view::HistoryData, String> {
    let Some(history) = &state.history else {
        return Err("기록 DB 를 열지 못했어요".to_string());
    };
    // 지금 조회 중인 주간 리셋 시각 (아직 기록에 안 남았어도 이번 주기를 정확히 나누려고)
    let live_resets: HashMap<String, chrono::DateTime<Utc>> = state
        .account_data
        .lock()
        .unwrap()
        .iter()
        .filter_map(|(id, data)| {
            let reset = data.usage.as_ref()?.weekly.as_ref()?.resets_at?;
            Some((id.clone(), reset))
        })
        .collect();
    history_view::build(
        history,
        &state.accounts(),
        &live_resets,
        history_view::Range::parse(&range),
        Utc::now(),
    )
}

/// 히스토리 창 이름
const HISTORY_LABEL: &str = "history";

/// 히스토리 창을 연다. 이미 열려 있으면 앞으로 가져온다.
#[tauri::command]
fn open_history(app: AppHandle) -> Result<(), String> {
    popover::hide(&app);
    if let Some(window) = app.get_webview_window(HISTORY_LABEL) {
        let _ = window.unminimize();
        let _ = window.show();
        let _ = window.set_focus();
        return Ok(());
    }
    let title = if alerts::prefers_korean() {
        "AI Usage 기록"
    } else {
        "AI Usage History"
    };
    let window = tauri::WebviewWindowBuilder::new(
        &app,
        HISTORY_LABEL,
        tauri::WebviewUrl::App("index.html#history".into()),
    )
    .title(title)
    .inner_size(980.0, 800.0)
    .min_inner_size(680.0, 520.0)
    .resizable(true)
    .center()
    .build()
    .map_err(|error| error.to_string())?;
    let _ = window.set_focus();
    Ok(())
}

#[tauri::command]
fn resize_popover(app: AppHandle, height: f64) {
    popover::resize(&app, height);
}

#[tauri::command]
fn hide_popover(app: AppHandle) {
    popover::hide(&app);
}

#[tauri::command]
fn quit_app(app: AppHandle) {
    app.exit(0);
}

/// 계정 목록이 바뀌었음을 화면에 알리고, 메뉴바·팝오버를 다시 그린다.
fn accounts_changed(app: &AppHandle, accounts: &[Account]) {
    let _ = app.emit("accounts-changed", accounts);
    refresh::publish(app);
}

/// 창 이벤트 처리
fn on_window_event(window: &Window, event: &WindowEvent) {
    popover::on_window_event(window, event);

    // 계정 로그인 창은 닫아도 없애지 않고 숨긴다. 숨은 채로 사용량 조회에 계속 쓴다.
    if window.label().starts_with(accounts::WINDOW_LABEL_PREFIX)
        && let WindowEvent::CloseRequested { api, .. } = event
    {
        api.prevent_close();
        let _ = window.hide();
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_notification::init())
        .manage(popover::PopoverState::default())
        .setup(|app| {
            // Dock 아이콘 없이 메뉴바에만 있는 앱으로 동작
            #[cfg(target_os = "macos")]
            app.set_activation_policy(tauri::ActivationPolicy::Accessory);

            let config_dir = app.path().app_config_dir()?;
            let settings_path: PathBuf = config_dir.join("settings.json");
            let accounts_path: PathBuf = config_dir.join("accounts.json");
            let db_path = config_dir.join("usage.db");
            let history = match History::open(&db_path) {
                Ok(history) => Some(history),
                Err(error) => {
                    eprintln!("[ai-usage] 기록 DB 를 열지 못했어요: {error}");
                    None
                }
            };
            let started_at = Utc::now();
            let demo = std::env::var("AI_USAGE_DEMO").is_ok_and(|value| value == "1");

            app.manage(AppState {
                snapshot: Mutex::new(mock::snapshot(started_at, started_at)),
                settings: Mutex::new(settings::load(&settings_path)),
                settings_path,
                accounts: Mutex::new(accounts::load(&accounts_path)),
                accounts_path,
                account_data: Mutex::new(HashMap::new()),
                logging_in: Mutex::new(HashSet::new()),
                refresh_lock: tokio::sync::Mutex::new(()),
                history,
                db_path,
                summary: Mutex::new(Summary::default()),
                token_scan: tokio::sync::Mutex::new(None),
                login_misses: Mutex::new(HashMap::new()),
                // 데모 모드에서는 키체인을 읽지 않는다
                admin_key: Mutex::new(if demo { None } else { api_cost::load_key() }),
                api_cost_error: Mutex::new(None),
                demo,
                started_at,
            });

            TrayIconBuilder::with_id(TRAY_ID)
                .tooltip("AI Usage")
                .on_tray_icon_event(|tray, event| {
                    if let TrayIconEvent::Click {
                        button: MouseButton::Left,
                        button_state: MouseButtonState::Up,
                        rect,
                        ..
                    } = event
                    {
                        popover::toggle(tray.app_handle(), rect);
                    }
                })
                .build(app)?;

            #[cfg(target_os = "macos")]
            if let Some(window) = app.get_webview_window(popover::LABEL) {
                popover::configure_macos(&window);
            }

            // 실제 데이터를 받기 전까지는 빈 화면(계정만 보이는 상태)으로 바로 그린다.
            refresh::publish(app.handle());
            // Codex 지난 기록을 처음 한 번 가져온다 (이미 했으면 아무것도 안 함)
            backfill_codex_history(app.handle().clone());

            // 로컬 로그 토큰 집계: 시작하자마자 한 번, 그 뒤 5분마다
            let handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                let mut ticker = tokio::time::interval(TOKEN_SCAN_INTERVAL);
                loop {
                    ticker.tick().await;
                    refresh::refresh_tokens(&handle, TOKEN_SCAN_INTERVAL / 2).await;
                }
            });

            // 서비스 상태: 시작하자마자 한 번, 그 뒤 5분마다
            let handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                let mut ticker = tokio::time::interval(SERVICE_INTERVAL);
                loop {
                    ticker.tick().await;
                    refresh::refresh_services(&handle).await;
                }
            });

            // Claude API 비용: 시작하자마자 한 번, 그 뒤 1시간마다
            let handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                let mut ticker = tokio::time::interval(API_COST_INTERVAL);
                loop {
                    ticker.tick().await;
                    refresh::refresh_api_cost(&handle).await;
                }
            });

            // 1분마다 갱신한다. interval 의 첫 tick 은 바로 끝나므로 시작하자마자 한 번 조회한다.
            let handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                let mut ticker = tokio::time::interval(REFRESH_INTERVAL);
                loop {
                    ticker.tick().await;
                    refresh::refresh_all(&handle).await;
                }
            });

            Ok(())
        })
        .on_window_event(on_window_event)
        .invoke_handler(tauri::generate_handler![
            get_snapshot,
            refresh_now,
            get_settings,
            set_settings,
            get_accounts,
            add_claude_account,
            add_codex_account,
            list_legacy_profiles,
            import_legacy_history,
            update_account,
            move_account,
            relogin_account,
            remove_account,
            get_api_cost_info,
            set_admin_key,
            remove_admin_key,
            get_history,
            open_history,
            resize_popover,
            hide_popover,
            quit_app
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

/// 개발용: 메뉴바 아이콘을 여러 설정으로 그려 PNG 로 저장한다.
/// `cargo run --example render_menubar -- <폴더>` 에서 쓴다.
#[cfg(target_os = "macos")]
#[doc(hidden)]
pub fn dev_render_menubar_previews(dir: &std::path::Path) -> Result<Vec<PathBuf>, String> {
    use crate::menubar::MenubarModel;
    use crate::settings::{MenubarStyle, PercentMode};

    std::fs::create_dir_all(dir).map_err(|error| error.to_string())?;
    let now = Utc::now();
    let snapshot = mock::snapshot(now, now);

    let variants = [
        ("bars-remaining", MenubarStyle::Bars, PercentMode::Remaining),
        ("bars", MenubarStyle::Bars, PercentMode::Used),
        ("numbers-used", MenubarStyle::Numbers, PercentMode::Used),
        (
            "numbers-remaining",
            MenubarStyle::Numbers,
            PercentMode::Remaining,
        ),
    ];
    let mut written = Vec::new();
    for (name, style, percent_mode) in variants {
        let settings = Settings {
            menubar_style: style,
            percent_mode,
        };
        let model = MenubarModel::from_snapshot(&snapshot, &settings);
        for (mode, dark) in [("light", false), ("dark", true)] {
            let path = dir.join(format!("menubar-{name}-{mode}.png"));
            menubar::macos::render_preview_png(&model, dark, &path)?;
            written.push(path);
        }
    }
    Ok(written)
}
