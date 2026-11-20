//! 계정 웹뷰 안에서 자바스크립트를 실행하고 결과를 Rust 로 받아 온다. (macOS 전용)
//!
//! WebKit 의 `callAsyncJavaScript` 를 쓴다.
//! - 자바스크립트 `await` 결과(Promise)를 그대로 돌려받을 수 있다.
//! - "클라이언트 전용 실행 공간(content world)"에서 돌아가므로,
//!   claude.ai 페이지의 스크립트가 우리 코드(fetch 등)를 가로채거나 바꿀 수 없다.
//! - Tauri 의 화면 통신(IPC)을 claude.ai 에 열어 줄 필요가 없다.

use std::sync::{Arc, Mutex};
use std::time::Duration;

use block2::RcBlock;
use objc2::MainThreadMarker;
use objc2::rc::Retained;
use objc2::runtime::AnyObject;
use objc2_foundation::{NSDictionary, NSError, NSString};
use objc2_web_kit::{WKContentWorld, WKWebView};
use tauri::{Runtime, WebviewWindow};
use tokio::sync::oneshot;

const TIMEOUT: Duration = Duration::from_secs(20);

/// `body` 는 async 함수의 몸통이고, 마지막에 문자열을 `return` 해야 한다.
/// `arguments` 의 (이름, 값) 이 그 함수 안에서 변수로 쓰인다.
pub async fn call<R: Runtime>(
    window: &WebviewWindow<R>,
    body: &str,
    arguments: &[(&str, &str)],
) -> Result<String, String> {
    let (sender, receiver) = oneshot::channel::<Result<String, String>>();
    // 결과는 한 번만 보낼 수 있으므로 Option 으로 감싸 두고 꺼내 쓴다.
    let sender = Arc::new(Mutex::new(Some(sender)));
    let body = body.to_string();
    let arguments: Vec<(String, String)> = arguments
        .iter()
        .map(|(name, value)| (name.to_string(), value.to_string()))
        .collect();

    // with_webview: Tauri 가 메인 스레드에서 실제 WKWebView 를 건네준다.
    window
        .with_webview(move |webview| {
            let Some(mtm) = MainThreadMarker::new() else {
                return;
            };
            // SAFETY: macOS 에서 inner() 는 이 창의 WKWebView 포인터이고, 지금은 메인 스레드다.
            let web_view: &WKWebView = unsafe { &*webview.inner().cast::<WKWebView>() };

            let names: Vec<Retained<NSString>> = arguments
                .iter()
                .map(|(name, _)| NSString::from_str(name))
                .collect();
            let values: Vec<Retained<NSString>> = arguments
                .iter()
                .map(|(_, value)| NSString::from_str(value))
                .collect();
            let name_refs: Vec<&NSString> = names.iter().map(|name| &**name).collect();
            let value_refs: Vec<&AnyObject> = values
                .iter()
                .map(|value| {
                    let object: &AnyObject = value;
                    object
                })
                .collect();
            let argument_dictionary = NSDictionary::from_slices(&name_refs, &value_refs);
            let world = unsafe { WKContentWorld::defaultClientWorld(mtm) };

            let completion = RcBlock::new(move |result: *mut AnyObject, error: *mut NSError| {
                // SAFETY: WebKit 이 넘겨준 포인터는 이 블록이 실행되는 동안 유효하다.
                let outcome = if let Some(error) = unsafe { error.as_ref() } {
                    Err(error.localizedDescription().to_string())
                } else if let Some(text) =
                    unsafe { result.as_ref() }.and_then(|object| object.downcast_ref::<NSString>())
                {
                    Ok(text.to_string())
                } else {
                    Err("자바스크립트 결과가 문자열이 아니에요".to_string())
                };
                if let Some(sender) = sender.lock().unwrap().take() {
                    let _ = sender.send(outcome);
                }
            });

            unsafe {
                web_view.callAsyncJavaScript_arguments_inFrame_inContentWorld_completionHandler(
                    &NSString::from_str(&body),
                    Some(&argument_dictionary),
                    None,
                    &world,
                    Some(&completion),
                );
            }
        })
        .map_err(|error| error.to_string())?;

    match tokio::time::timeout(TIMEOUT, receiver).await {
        Ok(Ok(outcome)) => outcome,
        Ok(Err(_)) => Err("웹뷰가 응답 없이 닫혔어요".to_string()),
        Err(_) => Err("웹뷰 응답이 너무 늦어요".to_string()),
    }
}
