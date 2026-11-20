//! 개발용 도구: 메뉴바 아이콘을 가짜 데이터로 그려서 PNG 파일로 저장한다.
//! 앱을 띄우지 않고도 아이콘 모양을 눈으로 확인할 수 있다.
//!
//! 실행: (src-tauri 폴더에서) cargo run --example render_menubar -- <저장할 폴더>

fn main() {
    let dir = std::env::args().nth(1).unwrap_or_else(|| "menubar-previews".to_string());

    #[cfg(target_os = "macos")]
    match ai_usage_lib::dev_render_menubar_previews(std::path::Path::new(&dir)) {
        Ok(paths) => {
            for path in paths {
                println!("{}", path.display());
            }
        }
        Err(error) => {
            eprintln!("실패: {error}");
            std::process::exit(1);
        }
    }

    #[cfg(not(target_os = "macos"))]
    eprintln!("macOS 에서만 동작합니다. ({dir})");
}
