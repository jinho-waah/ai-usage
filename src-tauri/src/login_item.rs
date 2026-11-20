//! 로그인할 때 자동으로 열기 (macOS 13+ 의 SMAppService).
//!
//! - 설치한 앱(.app)에서만 켤 수 있다. 개발 모드(`npm run tauri dev`)는 앱 묶음이 아니라서 등록하지 않는다.
//! - 켜면 시스템 설정 > 일반 > 로그인 항목에 "AI Usage" 로 나온다. 사용자가 거기서 끌 수도 있으므로,
//!   켜짐 여부는 따로 저장하지 않고 매번 시스템에 물어본다.

use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LoginItemState {
    /// 이 실행 파일이 .app 안에 있어서 켤 수 있는지
    pub supported: bool,
    pub enabled: bool,
    /// 켰지만 시스템 설정에서 허용이 필요한 상태
    pub needs_approval: bool,
}

/// 설치한 앱(.app 묶음 안)에서 실행 중인지
pub fn is_bundled() -> bool {
    std::env::current_exe()
        .ok()
        .and_then(|path| {
            path.to_str()
                .map(|path| path.contains(".app/Contents/MacOS/"))
        })
        .unwrap_or(false)
}

#[cfg(target_os = "macos")]
pub fn state() -> LoginItemState {
    use objc2_service_management::{SMAppService, SMAppServiceStatus};

    if !is_bundled() {
        return LoginItemState {
            supported: false,
            enabled: false,
            needs_approval: false,
        };
    }
    // SAFETY: 이 앱 자신을 가리키는 서비스 객체를 받아 상태만 읽는다.
    let status = unsafe { SMAppService::mainAppService().status() };
    LoginItemState {
        supported: true,
        enabled: status == SMAppServiceStatus::Enabled,
        needs_approval: status == SMAppServiceStatus::RequiresApproval,
    }
}

#[cfg(target_os = "macos")]
pub fn set(enabled: bool) -> Result<LoginItemState, String> {
    use objc2_service_management::SMAppService;

    if !is_bundled() {
        return Err("설치한 앱(.app)에서만 켤 수 있어요".to_string());
    }
    // SAFETY: 이 앱 자신을 로그인 항목에 넣거나 뺀다.
    let service = unsafe { SMAppService::mainAppService() };
    let result = unsafe {
        if enabled {
            service.registerAndReturnError()
        } else {
            service.unregisterAndReturnError()
        }
    };
    let after = state();
    match result {
        Ok(()) => Ok(after),
        // 이미 원하는 상태면(예: 원래 없던 항목을 끄기) 오류가 나도 괜찮다
        Err(_) if after.enabled == enabled => Ok(after),
        Err(error) => Err(format!(
            "로그인 항목을 바꾸지 못했어요: {}",
            error.localizedDescription()
        )),
    }
}

#[cfg(not(target_os = "macos"))]
pub fn state() -> LoginItemState {
    LoginItemState {
        supported: false,
        enabled: false,
        needs_approval: false,
    }
}

#[cfg(not(target_os = "macos"))]
pub fn set(_enabled: bool) -> Result<LoginItemState, String> {
    Err("macOS 에서만 쓸 수 있어요".to_string())
}
