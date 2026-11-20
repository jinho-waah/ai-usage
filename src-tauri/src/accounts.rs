//! 등록한 계정 목록을 `accounts.json` 에 저장하고 불러온다.
//!
//! 이 파일에는 비밀값이 없다. claude.ai 로그인 쿠키는 계정마다 따로 만든
//! WebKit 저장소(macOS 가 관리)에만 있고, 계정의 `id` 가 그 저장소 이름이다.

use std::fs;
use std::path::Path;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::model::{AccountLabel, LabelKind, Provider};

/// 계정 색 고르기 목록. 새 계정에는 아직 안 쓴 색을 이 순서로 붙여 준다.
/// 그래프에서 색각 이상이 있어도 서로 구분되도록 검사한 8색이다 (순서가 구분을 돕는다).
/// 다크 모드 그래프에서는 화면(history/chartColors.ts)이 짝이 되는 어두운 배경용 색으로 바꿔 그린다.
pub const PALETTE: [&str; 8] = [
    "#EB6834", "#2A78D6", "#1BAF7A", "#EDA100", "#E87BA4", "#008300", "#6250D6", "#E34948",
];

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Account {
    /// UUID. 계정별 WebKit 저장소 이름과 로그인 창 이름에도 쓴다.
    pub id: String,
    pub provider: Provider,
    /// 사용자가 붙인 이름 (예: "개인")
    pub name: String,
    pub label: AccountLabel,
    /// 계정 색 "#RRGGBB"
    pub color: String,
    /// 사용량을 조회할 claude.ai 조직(organization) ID. 로그인하면 채워진다.
    #[serde(default)]
    pub org_id: Option<String>,
    /// 조직 이름. 설정 화면에서 어느 계정인지 알아보는 용도.
    #[serde(default)]
    pub org_name: Option<String>,
    /// 플랜 이름 (예: "Ultra", "Max", "Pro")
    #[serde(default)]
    pub plan: Option<String>,
}

/// 다른 계정과 비슷해 보이지 않는 색. `preferred` 가 비어 있으면 그것, 아니면 목록에서 아직 안 쓴 첫 색.
fn unused_color(existing: &[Account], preferred: Option<&str>) -> String {
    let taken = |candidate: &str| {
        existing
            .iter()
            .any(|account| colors_look_alike(&account.color, candidate))
    };
    preferred
        .into_iter()
        .chain(PALETTE)
        .find(|candidate| !taken(candidate))
        .unwrap_or(PALETTE[existing.len() % PALETTE.len()])
        .to_string()
}

/// 두 색이 거의 같아 보이는지 (RGB 거리로 대략 판단)
fn colors_look_alike(a: &str, b: &str) -> bool {
    let rgb = |hex: &str| -> Option<(f64, f64, f64)> {
        let hex = hex.strip_prefix('#')?;
        if hex.len() != 6 {
            return None;
        }
        let channel =
            |range: std::ops::Range<usize>| u8::from_str_radix(&hex[range], 16).ok().map(f64::from);
        Some((channel(0..2)?, channel(2..4)?, channel(4..6)?))
    };
    match (rgb(a), rgb(b)) {
        (Some((r1, g1, b1)), Some((r2, g2, b2))) => {
            ((r1 - r2).powi(2) + (g1 - g2).powi(2) + (b1 - b2).powi(2)).sqrt() < 80.0
        }
        _ => a.eq_ignore_ascii_case(b),
    }
}

impl Account {
    /// 새 Claude 계정. 이름·라벨·색은 기본값으로 채우고, 로그인 뒤 사용자가 바꿀 수 있다.
    pub fn new_claude(existing: &[Account]) -> Account {
        let number = existing.len() + 1;
        Account {
            id: Uuid::new_v4().to_string(),
            provider: Provider::Claude,
            name: format!("Claude {number}"),
            label: AccountLabel {
                kind: LabelKind::Name,
                text: String::new(),
            },
            color: unused_color(existing, None),
            org_id: None,
            org_name: None,
            plan: None,
        }
    }

    /// 새 Codex 계정. Codex CLI 로그인 하나를 읽으므로 하나만 둔다.
    /// 색은 OpenAI 초록을 쓰되, 비슷한 색을 쓰는 계정이 이미 있으면 다른 색.
    pub fn new_codex(existing: &[Account]) -> Account {
        Account {
            id: Uuid::new_v4().to_string(),
            provider: Provider::Codex,
            name: "Codex".to_string(),
            label: AccountLabel {
                kind: LabelKind::Name,
                text: String::new(),
            },
            color: unused_color(existing, Some("#10A37F")),
            org_id: None,
            org_name: None,
            plan: None,
        }
    }

    /// 이 계정의 WebKit 저장소 ID (UUID 16바이트)
    pub fn data_store_id(&self) -> Option<[u8; 16]> {
        Uuid::parse_str(&self.id).ok().map(|uuid| uuid.into_bytes())
    }

    /// 이 계정의 로그인·조회용 웹뷰 창 이름
    pub fn window_label(&self) -> String {
        window_label(&self.id)
    }
}

pub const WINDOW_LABEL_PREFIX: &str = "account-";

pub fn window_label(account_id: &str) -> String {
    format!("{WINDOW_LABEL_PREFIX}{account_id}")
}

/// 화면에서 고칠 수 있는 항목만 모은 것 (이름, 라벨, 색)
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountEdit {
    pub id: String,
    pub name: String,
    pub label: AccountLabel,
    pub color: String,
}

impl AccountEdit {
    /// 값을 다듬어서 계정에 반영한다. 비어 있는 이름, 비어 있는 이모지는 기존 값을 유지한다.
    pub fn apply_to(&self, account: &mut Account) {
        let name = self.name.trim();
        if !name.is_empty() {
            account.name = name.chars().take(30).collect();
        }
        match self.label.kind {
            LabelKind::Emoji => {
                let emoji: String = self.label.text.trim().chars().take(4).collect();
                if !emoji.is_empty() {
                    account.label = AccountLabel {
                        kind: LabelKind::Emoji,
                        text: emoji,
                    };
                }
            }
            kind => {
                account.label = AccountLabel {
                    kind,
                    text: String::new(),
                }
            }
        }
        if is_hex_color(&self.color) {
            account.color = self.color.clone();
        }
    }
}

fn is_hex_color(value: &str) -> bool {
    value.len() == 7 && value.starts_with('#') && value[1..].chars().all(|c| c.is_ascii_hexdigit())
}

/// 계정 목록 파일을 읽는다. 없거나 깨져 있으면 빈 목록.
pub fn load(path: &Path) -> Vec<Account> {
    fs::read_to_string(path)
        .ok()
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or_default()
}

pub fn save(path: &Path, accounts: &[Account]) -> Result<(), String> {
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    let text = serde_json::to_string_pretty(accounts).map_err(|e| e.to_string())?;
    fs::write(path, text).map_err(|e| e.to_string())
}

/// 목록에서 계정 하나를 위(-1) 또는 아래(+1)로 옮긴다.
pub fn move_account(accounts: &mut [Account], id: &str, offset: i32) {
    let Some(index) = accounts.iter().position(|a| a.id == id) else {
        return;
    };
    let target = index as i64 + offset as i64;
    if (0..accounts.len() as i64).contains(&target) {
        accounts.swap(index, target as usize);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_accounts_get_distinct_defaults() {
        let first = Account::new_claude(&[]);
        let second = Account::new_claude(std::slice::from_ref(&first));
        assert_eq!(first.name, "Claude 1");
        assert_eq!(second.name, "Claude 2");
        assert_eq!(first.label.kind, LabelKind::Name);
        assert_ne!(first.color, second.color);
        assert!(first.data_store_id().is_some());
    }

    #[test]
    fn default_colors_avoid_look_alikes() {
        // 예전 기본색(주황·파랑·초록)을 쓰는 계정들이 있으면, 새 계정은 비슷한 색을 피한다
        let mut existing = vec![
            Account::new_claude(&[]),
            Account::new_claude(&[]),
            Account::new_claude(&[]),
        ];
        existing[0].color = "#D97757".into();
        existing[1].color = "#6A9BCC".into();
        existing[2].color = "#10A37F".into();
        let next = Account::new_claude(&existing);
        assert_eq!(next.color, "#EDA100", "주황·파랑·초록과 비슷한 색은 건너뜀");

        // Codex 는 OpenAI 초록이 비어 있으면 그것, 비슷한 초록이 이미 있으면 다른 색
        assert_eq!(Account::new_codex(&[]).color, "#10A37F");
        assert_ne!(Account::new_codex(&existing).color, "#10A37F");
    }

    #[test]
    fn edits_are_trimmed_and_validated() {
        let mut account = Account::new_claude(&[]);
        AccountEdit {
            id: account.id.clone(),
            name: "  개인  ".into(),
            label: AccountLabel {
                kind: LabelKind::Emoji,
                text: " 💼 ".into(),
            },
            color: "not-a-color".into(),
        }
        .apply_to(&mut account);
        assert_eq!(account.name, "개인");
        assert_eq!(
            account.label,
            AccountLabel {
                kind: LabelKind::Emoji,
                text: "💼".into()
            }
        );
        assert_eq!(account.color, PALETTE[0], "잘못된 색은 무시");
    }

    #[test]
    fn move_swaps_neighbours_and_ignores_edges() {
        let a = Account::new_claude(&[]);
        let b = Account::new_claude(std::slice::from_ref(&a));
        let mut list = vec![a.clone(), b.clone()];
        move_account(&mut list, &b.id, -1);
        assert_eq!(list[0].id, b.id);
        move_account(&mut list, &b.id, -1);
        assert_eq!(list[0].id, b.id, "맨 위에서 더 올라가지 않음");
    }
}
