//! 메뉴바 아이콘에 "무엇을" 그릴지 정한다. (운영체제와 무관한 부분)
//!
//! 실제로 "어떻게" 그리는지는 macOS 전용 파일 `macos.rs` 에 있다.
//! 이렇게 나눠 두면 그리는 내용(계산)은 일반 Rust 테스트로 확인할 수 있다.

use crate::model::{LabelKind, Snapshot};
use crate::settings::{MenubarStyle, PercentMode, Settings};

#[cfg(target_os = "macos")]
pub mod macos;

/// 메뉴바에 그릴 계정 하나.
#[derive(Debug, Clone, PartialEq)]
pub struct MenubarItem {
    pub label_kind: LabelKind,
    /// 별칭이나 이모지. 색 점이면 빈 문자열.
    pub label_text: String,
    /// 계정 색 (빨강, 초록, 파랑) 각각 0~1. 색 점 라벨에 쓴다.
    pub color: (f64, f64, f64),
    pub used_percent: f64,
    /// 시간 눈금 위치 (0~100)
    pub elapsed_percent: f64,
    /// 사용량이 시간 진행보다 5%p 이상 앞섬 → 빨강
    pub warn: bool,
}

/// 메뉴바 이미지 하나를 그리는 데 필요한 정보 전부.
#[derive(Debug, Clone, PartialEq)]
pub struct MenubarModel {
    pub items: Vec<MenubarItem>,
    pub style: MenubarStyle,
    pub percent_mode: PercentMode,
}

impl MenubarModel {
    pub fn from_snapshot(snapshot: &Snapshot, settings: &Settings) -> Self {
        // `filter_map`: 각 계정을 메뉴바 항목으로 바꾸되, 사용량 정보가 없는 계정은 건너뛴다.
        let items = snapshot
            .accounts
            .iter()
            .filter_map(|account| {
                let window = account.primary_window()?;
                Some(MenubarItem {
                    label_kind: account.label.kind,
                    label_text: account.label.text.clone(),
                    color: parse_hex_color(&account.color),
                    used_percent: window.used_percent,
                    elapsed_percent: window.elapsed_percent,
                    warn: window.ahead_of_pace,
                })
            })
            .collect();

        MenubarModel {
            items,
            style: settings.menubar_style,
            percent_mode: settings.percent_mode,
        }
    }

    /// 색이 하나도 없으면(빨강·이모지·색 점이 없으면) true.
    /// 이때는 macOS 의 "템플릿 이미지"로 그려서, macOS 가 메뉴바에 맞는 색을 직접 입히게 한다.
    pub fn is_monochrome(&self) -> bool {
        self.items
            .iter()
            .all(|item| !item.warn && item.label_kind == LabelKind::Alias)
    }

    /// 숫자 스타일에서 보여 줄 글자 (예: "58%")
    pub fn number_text(&self, item: &MenubarItem) -> String {
        format!("{:.0}%", self.percent_mode.display_value(item.used_percent))
    }
}

/// "#RRGGBB" 형식의 색을 (빨강, 초록, 파랑) 0~1 값으로 바꾼다. 형식이 틀리면 회색.
pub fn parse_hex_color(hex: &str) -> (f64, f64, f64) {
    let digits = hex.trim_start_matches('#');
    let channel = |range: std::ops::Range<usize>| {
        digits
            .get(range)
            .and_then(|part| u8::from_str_radix(part, 16).ok())
            .map(|value| value as f64 / 255.0)
    };
    match (digits.len(), channel(0..2), channel(2..4), channel(4..6)) {
        (6, Some(r), Some(g), Some(b)) => (r, g, b),
        _ => (0.5, 0.5, 0.5),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mock;
    use chrono::Utc;

    #[test]
    fn hex_colors_parse() {
        assert_eq!(parse_hex_color("#FF0000"), (1.0, 0.0, 0.0));
        assert_eq!(parse_hex_color("oops"), (0.5, 0.5, 0.5));
    }

    #[test]
    fn mock_snapshot_becomes_three_items() {
        let now = Utc::now();
        let model = MenubarModel::from_snapshot(&mock::snapshot(now, now), &Settings::default());
        assert_eq!(model.items.len(), 3);
        // 빨강 계정·이모지·색 점이 섞여 있으므로 템플릿(단색) 이미지가 아니다.
        assert!(!model.is_monochrome());
        assert_eq!(model.number_text(&model.items[0]), "58%");
    }

    #[test]
    fn remaining_mode_changes_number_text() {
        let now = Utc::now();
        let settings = Settings { percent_mode: PercentMode::Remaining, ..Settings::default() };
        let model = MenubarModel::from_snapshot(&mock::snapshot(now, now), &settings);
        assert_eq!(model.number_text(&model.items[0]), "42%");
    }
}
