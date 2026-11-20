//! 메뉴바 아이콘 아래에 뜨는 팝오버 창을 보이고 숨긴다.
//!
//! 팝오버 창은 `tauri.conf.json` 에 "popover" 라는 이름으로 미리 만들어 두고(처음엔 숨김),
//! 아이콘을 누를 때마다 아이콘 바로 아래로 옮겨서 보이거나 숨긴다.

use std::sync::Mutex;
use std::time::{Duration, Instant};

use tauri::{AppHandle, LogicalSize, Manager, PhysicalPosition, Rect, WebviewWindow, Window, WindowEvent};

pub const LABEL: &str = "popover";
/// 팝오버 너비 (pt)
pub const WIDTH: f64 = 360.0;
/// 메뉴바와 팝오버 사이 간격 (pt)
const GAP_BELOW_MENUBAR: f64 = 6.0;
/// 화면 가장자리와 띄울 최소 간격 (pt)
const SCREEN_MARGIN: f64 = 8.0;
/// 바깥을 눌러 닫힌 직후의 클릭이 아이콘에 닿아도 다시 열리지 않게 막는 시간.
/// (팝오버가 열린 상태에서 아이콘을 누르면 "닫기"로 동작해야 하기 때문)
const REOPEN_GUARD: Duration = Duration::from_millis(300);

/// 팝오버 관련 상태. `Mutex` 는 여러 스레드가 동시에 고치지 못하게 막는 자물쇠다.
#[derive(Default)]
pub struct PopoverState {
    /// 마지막으로 알게 된 메뉴바 아이콘 위치. 창 높이가 바뀌면 위치를 다시 맞출 때 쓴다.
    tray_rect: Mutex<Option<Rect>>,
    /// 마지막으로 숨긴 시각
    hidden_at: Mutex<Option<Instant>>,
}

/// 메뉴바 아이콘을 눌렀을 때: 열려 있으면 닫고, 닫혀 있으면 아이콘 아래에 연다.
pub fn toggle(app: &AppHandle, tray_rect: Rect) {
    let Some(window) = app.get_webview_window(LABEL) else {
        return;
    };
    let state = app.state::<PopoverState>();
    *state.tray_rect.lock().unwrap() = Some(tray_rect);

    if window.is_visible().unwrap_or(false) {
        hide(app);
        return;
    }

    let just_hidden = state
        .hidden_at
        .lock()
        .unwrap()
        .is_some_and(|hidden_at| hidden_at.elapsed() < REOPEN_GUARD);
    if just_hidden {
        return;
    }

    place_below_tray(&window, tray_rect);
    let _ = window.show();
    let _ = window.set_focus();
}

pub fn hide(app: &AppHandle) {
    if let Some(window) = app.get_webview_window(LABEL) {
        let _ = window.hide();
    }
    *app.state::<PopoverState>().hidden_at.lock().unwrap() = Some(Instant::now());
}

/// 화면(React)이 내용 높이에 맞게 창 크기를 바꿔 달라고 할 때 부른다.
pub fn resize(app: &AppHandle, content_height: f64) {
    let Some(window) = app.get_webview_window(LABEL) else {
        return;
    };
    // 화면보다 길어지지 않게 자른다. 넘치는 내용은 화면(React) 쪽에서 스크롤한다.
    let max_height = window
        .current_monitor()
        .ok()
        .flatten()
        .map(|monitor| monitor.size().height as f64 / monitor.scale_factor() - 80.0)
        .unwrap_or(800.0);
    let height = content_height.clamp(120.0, max_height.max(120.0));
    let _ = window.set_size(LogicalSize::new(WIDTH, height));

    let tray_rect = *app.state::<PopoverState>().tray_rect.lock().unwrap();
    if let Some(tray_rect) = tray_rect {
        place_below_tray(&window, tray_rect);
    }
}

/// 메뉴바 아이콘 바로 아래 가운데에 창을 놓는다. 화면 밖으로 나가지 않게 맞춘다.
fn place_below_tray(window: &WebviewWindow, tray_rect: Rect) {
    let scale = window.scale_factor().unwrap_or(2.0);
    let tray_position = tray_rect.position.to_physical::<f64>(scale);
    let tray_size = tray_rect.size.to_physical::<f64>(scale);
    let window_width = window
        .outer_size()
        .map(|size| size.width as f64)
        .unwrap_or(WIDTH * scale);

    let mut x = tray_position.x + tray_size.width / 2.0 - window_width / 2.0;
    let y = tray_position.y + tray_size.height + GAP_BELOW_MENUBAR * scale;

    // 아이콘이 있는 모니터 안으로 x 를 맞춘다.
    if let Ok(monitors) = window.available_monitors() {
        let tray_monitor = monitors.iter().find(|monitor| {
            let left = monitor.position().x as f64;
            let right = left + monitor.size().width as f64;
            (left..right).contains(&tray_position.x)
        });
        if let Some(monitor) = tray_monitor {
            let min_x = monitor.position().x as f64 + SCREEN_MARGIN * scale;
            let max_x = monitor.position().x as f64 + monitor.size().width as f64
                - window_width
                - SCREEN_MARGIN * scale;
            x = x.clamp(min_x, max_x.max(min_x));
        }
    }

    let _ = window.set_position(PhysicalPosition::new(x, y));
}

/// 창 이벤트 처리: 팝오버가 포커스를 잃으면(바깥을 클릭하면) 숨긴다.
pub fn on_window_event(window: &Window, event: &WindowEvent) {
    if window.label() == LABEL && matches!(event, WindowEvent::Focused(false)) {
        hide(window.app_handle());
    }
}

/// macOS 전용 창 설정: 모든 데스크톱(Space)과 전체 화면 앱 위에서도 뜨게 하고,
/// Mission Control 에는 나타나지 않게 한다.
#[cfg(target_os = "macos")]
pub fn configure_macos(window: &WebviewWindow) {
    use objc2_app_kit::{NSWindow, NSWindowCollectionBehavior};

    let Ok(pointer) = window.ns_window() else {
        return;
    };
    // SAFETY: Tauri 가 돌려준 포인터는 이 창의 NSWindow 이고, 이 함수는 메인 스레드(setup)에서 불린다.
    let ns_window: &NSWindow = unsafe { &*pointer.cast::<NSWindow>() };
    ns_window.setCollectionBehavior(
        NSWindowCollectionBehavior::CanJoinAllSpaces
            | NSWindowCollectionBehavior::FullScreenAuxiliary
            | NSWindowCollectionBehavior::Transient,
    );
}
