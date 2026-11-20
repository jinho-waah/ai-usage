//! 사용자 설정을 JSON 파일로 저장하고 불러온다.
//!
//! 저장 위치: `~/Library/Application Support/io.github.jinho-waah.ai-usage/settings.json`
//! (Tauri 가 앱 식별자(identifier)로 폴더를 정해 준다.)

use std::fs;
use std::path::Path;

use serde::{Deserialize, Serialize};

/// 메뉴바 표시 방식.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "camelCase")]
pub enum MenubarStyle {
    /// 계정별 미니 막대 + 시간 눈금 (기본)
    #[default]
    Bars,
    /// 계정별 숫자 (예: J 58%)
    Numbers,
}

/// 퍼센트를 무엇 기준으로 보여 줄지.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "camelCase")]
pub enum PercentMode {
    /// 사용한 % (기본)
    #[default]
    Used,
    /// 남은 %
    Remaining,
}

/// `#[serde(default)]`: 파일에 없는 항목은 기본값으로 채운다.
/// 나중에 설정 항목이 늘어나도 예전 설정 파일을 그대로 읽을 수 있다.
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
#[serde(rename_all = "camelCase", default)]
pub struct Settings {
    pub menubar_style: MenubarStyle,
    pub percent_mode: PercentMode,
}

impl PercentMode {
    /// 사용한 % 를 이 기준에 맞는 값으로 바꾼다.
    pub fn display_value(self, used_percent: f64) -> f64 {
        match self {
            PercentMode::Used => used_percent,
            PercentMode::Remaining => 100.0 - used_percent,
        }
    }
}

/// 설정 파일을 읽는다. 파일이 없거나 깨져 있으면 기본값을 쓴다.
pub fn load(path: &Path) -> Settings {
    fs::read_to_string(path)
        .ok()
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or_default()
}

/// 설정 파일을 쓴다. 폴더가 없으면 만든다.
pub fn save(path: &Path, settings: &Settings) -> Result<(), String> {
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    let text = serde_json::to_string_pretty(settings).map_err(|e| e.to_string())?;
    fs::write(path, text).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_fields_fall_back_to_defaults() {
        let parsed: Settings = serde_json::from_str(r#"{"percentMode":"remaining"}"#).unwrap();
        assert_eq!(parsed.menubar_style, MenubarStyle::Bars);
        assert_eq!(parsed.percent_mode, PercentMode::Remaining);
    }

    #[test]
    fn remaining_mode_flips_the_percentage() {
        assert_eq!(PercentMode::Remaining.display_value(58.0), 42.0);
        assert_eq!(PercentMode::Used.display_value(58.0), 58.0);
    }
}
