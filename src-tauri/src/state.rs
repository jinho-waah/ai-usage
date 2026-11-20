//! 앱이 켜져 있는 동안 여러 곳에서 함께 쓰는 상태.
//!
//! Rust 메모: `Mutex` 는 여러 스레드가 같은 값을 동시에 고치지 못하게 막는 자물쇠다.
//! `lock()` 으로 열고, 그 줄(또는 블록)이 끝나면 자동으로 다시 잠긴다.
//! 자물쇠를 연 채로 `.await`(네트워크 기다리기)를 하면 안 되므로, 필요한 값만 복사해서 쓴다.

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::Mutex;

use chrono::{DateTime, Utc};

use crate::accounts::{self, Account};
use crate::history::History;
use crate::model::{AccountStatus, ExtraUsage, Snapshot, UsageReading};
use crate::settings::Settings;

/// 계정 하나의 마지막 조회 결과
#[derive(Debug, Clone, Default)]
pub struct AccountData {
    pub usage: Option<UsageReading>,
    pub extra: Option<ExtraUsage>,
    /// 조회할 때 알게 된 플랜 이름 (Codex 처럼 응답에 플랜이 같이 오는 경우)
    pub plan: Option<String>,
    /// `None` 이면 아직 한 번도 조회하지 않음
    pub status: Option<AccountStatus>,
    pub updated_at: Option<DateTime<Utc>>,
    /// Extra usage 를 마지막으로 확인한 시각 (10분마다만 확인)
    pub extra_checked_at: Option<DateTime<Utc>>,
    /// 플랜(조직 정보)을 마지막으로 확인한 시각 (6시간마다만 확인)
    pub plan_checked_at: Option<DateTime<Utc>>,
}

pub struct AppState {
    pub snapshot: Mutex<Snapshot>,
    pub settings: Mutex<Settings>,
    pub settings_path: PathBuf,
    pub accounts: Mutex<Vec<Account>>,
    pub accounts_path: PathBuf,
    pub account_data: Mutex<HashMap<String, AccountData>>,
    /// 지금 로그인 창이 열려 있는 계정 ID. 이 계정은 갱신하지 않는다.
    pub logging_in: Mutex<HashSet<String>>,
    /// 갱신이 겹치지 않게 한 번에 하나만 실행한다. (tokio 자물쇠는 `.await` 중에도 잡고 있을 수 있다)
    pub refresh_lock: tokio::sync::Mutex<()>,
    /// 사용률 기록 DB. 열지 못했으면 `None` (기록·그래프 없이 동작)
    pub history: Option<History>,
    /// 데모 모드(`AI_USAGE_DEMO=1`): 실제 계정 대신 가짜 데이터를 보여 준다.
    pub demo: bool,
    pub started_at: DateTime<Utc>,
}

impl AppState {
    /// 계정 목록 복사본
    pub fn accounts(&self) -> Vec<Account> {
        self.accounts.lock().unwrap().clone()
    }

    /// 계정 목록을 고치고 파일에 저장한다. 고친 뒤의 목록을 돌려준다.
    pub fn update_accounts(
        &self,
        change: impl FnOnce(&mut Vec<Account>),
    ) -> Result<Vec<Account>, String> {
        let mut list = self.accounts.lock().unwrap();
        change(&mut list);
        accounts::save(&self.accounts_path, &list)?;
        Ok(list.clone())
    }

    /// 계정 하나의 조회 결과를 고친다.
    pub fn update_data(&self, account_id: &str, change: impl FnOnce(&mut AccountData)) {
        let mut data = self.account_data.lock().unwrap();
        change(data.entry(account_id.to_string()).or_default());
    }

    pub fn data(&self, account_id: &str) -> AccountData {
        self.account_data
            .lock()
            .unwrap()
            .get(account_id)
            .cloned()
            .unwrap_or_default()
    }

    pub fn is_logging_in(&self, account_id: &str) -> bool {
        self.logging_in.lock().unwrap().contains(account_id)
    }
}
