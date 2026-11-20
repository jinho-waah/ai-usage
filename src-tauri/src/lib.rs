//! 앱의 시작점.
//!
//! 하는 일
//! 1. 앱 상태(사용량 스냅샷, 설정)를 만든다.
//! 2. 메뉴바 아이콘을 만들고, 누르면 팝오버가 열리게 연결한다.
//! 3. 1분마다 데이터를 갱신해서 메뉴바와 팝오버 화면에 알린다.
//! 4. React 화면이 부를 수 있는 명령(`#[tauri::command]`)을 등록한다.

mod menubar;
mod mock;
mod model;
mod popover;
mod settings;

use std::path::PathBuf;
use std::sync::Mutex;
use std::time::Duration;

use chrono::{DateTime, Utc};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Emitter, Manager, State};

use crate::menubar::MenubarModel;
use crate::model::Snapshot;
use crate::settings::Settings;

const TRAY_ID: &str = "main";
/// 구독 사용량 갱신 주기 (스펙: 1분 고정)
const REFRESH_INTERVAL: Duration = Duration::from_secs(60);

/// 앱이 켜져 있는 동안 여러 곳에서 함께 쓰는 상태.
struct AppState {
    snapshot: Mutex<Snapshot>,
    settings: Mutex<Settings>,
    settings_path: PathBuf,
    /// 앱을 켠 시각. M1 가짜 데이터의 리셋 시각 기준.
    started_at: DateTime<Utc>,
}

// ───────── React 화면에서 부르는 명령 ─────────
// `#[tauri::command]` 가 붙은 함수는 화면에서 `invoke("함수이름")` 으로 부를 수 있다.
// `State<AppState>` 인자는 Tauri 가 알아서 채워 준다.

#[tauri::command]
fn get_snapshot(state: State<AppState>) -> Snapshot {
    state.snapshot.lock().unwrap().clone()
}

#[tauri::command]
fn get_settings(state: State<AppState>) -> Settings {
    state.settings.lock().unwrap().clone()
}

#[tauri::command]
fn set_settings(app: AppHandle, state: State<AppState>, settings: Settings) -> Result<(), String> {
    settings::save(&state.settings_path, &settings)?;
    *state.settings.lock().unwrap() = settings.clone();
    update_menubar(&app);
    let _ = app.emit("settings-changed", &settings);
    Ok(())
}

#[tauri::command]
fn refresh_now(app: AppHandle) -> Snapshot {
    refresh(&app)
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

// ───────── 갱신 ─────────

/// 데이터를 새로 만들고(M1 은 가짜 데이터), 메뉴바를 다시 그리고, 화면에 알린다.
fn refresh(app: &AppHandle) -> Snapshot {
    let state = app.state::<AppState>();
    let snapshot = mock::snapshot(state.started_at, Utc::now());
    *state.snapshot.lock().unwrap() = snapshot.clone();
    update_menubar(app);
    let _ = app.emit("snapshot-updated", &snapshot);
    snapshot
}

/// 현재 스냅샷과 설정으로 메뉴바 아이콘을 다시 그린다.
fn update_menubar(app: &AppHandle) {
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
                menubar::macos::apply(&status_item, &model, mtm);
            }
        });
    }

    // 툴팁을 다시 넣으면 Tauri 가 클릭 영역을 새 아이콘 크기에 맞춰 준다.
    let _ = tray.set_tooltip(Some("AI Usage"));
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .manage(popover::PopoverState::default())
        .setup(|app| {
            // Dock 아이콘 없이 메뉴바에만 있는 앱으로 동작
            #[cfg(target_os = "macos")]
            app.set_activation_policy(tauri::ActivationPolicy::Accessory);

            let settings_path = app.path().app_config_dir()?.join("settings.json");
            let started_at = Utc::now();
            app.manage(AppState {
                snapshot: Mutex::new(mock::snapshot(started_at, started_at)),
                settings: Mutex::new(settings::load(&settings_path)),
                settings_path,
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

            // 1분마다 갱신한다. interval 의 첫 tick 은 바로 끝나므로 시작하자마자 한 번 그린다.
            let handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                let mut ticker = tokio::time::interval(REFRESH_INTERVAL);
                loop {
                    ticker.tick().await;
                    refresh(&handle);
                }
            });

            Ok(())
        })
        .on_window_event(popover::on_window_event)
        .invoke_handler(tauri::generate_handler![
            get_snapshot,
            get_settings,
            set_settings,
            refresh_now,
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
    use crate::settings::{MenubarStyle, PercentMode};

    std::fs::create_dir_all(dir).map_err(|error| error.to_string())?;
    let now = Utc::now();
    let snapshot = mock::snapshot(now, now);

    let variants = [
        ("bars", MenubarStyle::Bars, PercentMode::Used),
        ("numbers-used", MenubarStyle::Numbers, PercentMode::Used),
        ("numbers-remaining", MenubarStyle::Numbers, PercentMode::Remaining),
    ];
    let mut written = Vec::new();
    for (name, style, percent_mode) in variants {
        let settings = Settings { menubar_style: style, percent_mode };
        let model = MenubarModel::from_snapshot(&snapshot, &settings);
        for (mode, dark) in [("light", false), ("dark", true)] {
            let path = dir.join(format!("menubar-{name}-{mode}.png"));
            menubar::macos::render_preview_png(&model, dark, &path)?;
            written.push(path);
        }
    }
    Ok(written)
}
