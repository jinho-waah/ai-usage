//! 사용률 기록(히스토리)을 SQLite 파일에 저장한다.
//!
//! - 값이 바뀔 때만 한 줄씩 남긴다. 같은 값이 계속되면 기록하지 않으므로 용량이 작고, 지우지 않고 영구 보관한다.
//! - 그래프는 "다음 기록 전까지는 같은 값"으로 보고 계단 모양으로 그린다.
//! - 저장 위치: `~/Library/Application Support/io.github.jinho-waah.ai-usage/usage.db`
//!
//! Rust 메모: SQLite 연결(`Connection`)은 한 번에 한 곳에서만 써야 해서 `Mutex` 로 감싼다.

use std::path::Path;
use std::sync::Mutex;

use chrono::{DateTime, Duration, TimeZone, Utc};
use rusqlite::{Connection, OptionalExtension, params};

use crate::model::TrendPoint;

/// 이 정도보다 작게 바뀐 값은 같은 값으로 본다 (%)
const SAME_VALUE_TOLERANCE: f64 = 0.05;

/// 어느 사용량 창의 기록인지
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WindowKind {
    Weekly,
    Session,
}

impl WindowKind {
    fn as_str(self) -> &'static str {
        match self {
            WindowKind::Weekly => "weekly",
            WindowKind::Session => "session",
        }
    }
}

/// 기록 한 줄
#[derive(Debug, Clone, PartialEq)]
pub struct Sample {
    pub at: DateTime<Utc>,
    pub used_percent: f64,
}

pub struct History {
    connection: Mutex<Connection>,
}

impl History {
    /// 기록 파일을 연다. 없으면 만든다.
    pub fn open(path: &Path) -> Result<History, String> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).map_err(|error| error.to_string())?;
        }
        Self::prepare(Connection::open(path).map_err(|error| error.to_string())?)
    }

    /// 테스트용: 메모리 안에만 있는 기록
    #[cfg(test)]
    pub fn in_memory() -> Result<History, String> {
        Self::prepare(Connection::open_in_memory().map_err(|error| error.to_string())?)
    }

    fn prepare(connection: Connection) -> Result<History, String> {
        connection
            .execute_batch(
                "PRAGMA journal_mode = WAL;
                 CREATE TABLE IF NOT EXISTS usage_samples (
                     account_id   TEXT NOT NULL,
                     window       TEXT NOT NULL,      -- 'weekly' | 'session'
                     at           INTEGER NOT NULL,   -- 유닉스 시각(초)
                     used_percent REAL NOT NULL,      -- 사용한 비율 0~100
                     source       TEXT NOT NULL,      -- 'live' | 'codex-log' | 'claude-usage-app'
                     PRIMARY KEY (account_id, window, at)
                 );
                 CREATE TABLE IF NOT EXISTS meta (
                     key   TEXT PRIMARY KEY,
                     value TEXT NOT NULL
                 );",
            )
            .map_err(|error| error.to_string())?;
        Ok(History {
            connection: Mutex::new(connection),
        })
    }

    /// 사용률을 기록한다. 바로 앞 기록과 값이 같으면 남기지 않는다. 기록했으면 true.
    pub fn record(
        &self,
        account_id: &str,
        window: WindowKind,
        at: DateTime<Utc>,
        used_percent: f64,
        source: &str,
    ) -> Result<bool, String> {
        let connection = self.connection.lock().unwrap();
        let previous: Option<f64> = connection
            .query_row(
                "SELECT used_percent FROM usage_samples
                 WHERE account_id = ?1 AND window = ?2 AND at <= ?3
                 ORDER BY at DESC LIMIT 1",
                params![account_id, window.as_str(), at.timestamp()],
                |row| row.get(0),
            )
            .optional()
            .map_err(|error| error.to_string())?;
        if previous.is_some_and(|previous| (previous - used_percent).abs() < SAME_VALUE_TOLERANCE) {
            return Ok(false);
        }
        connection
            .execute(
                "INSERT OR REPLACE INTO usage_samples (account_id, window, at, used_percent, source)
                 VALUES (?1, ?2, ?3, ?4, ?5)",
                params![account_id, window.as_str(), at.timestamp(), used_percent, source],
            )
            .map_err(|error| error.to_string())?;
        Ok(true)
    }

    /// 지난 기록을 한꺼번에 넣는다 (Codex 로그, 원본 앱 기록 가져오기).
    /// 값이 바뀌는 지점만 남겨서 넣고, 같은 시각 기록이 이미 있으면 건너뛴다. 넣은 개수를 돌려준다.
    pub fn import(
        &self,
        account_id: &str,
        window: WindowKind,
        samples: &[Sample],
        source: &str,
    ) -> Result<usize, String> {
        let mut sorted = samples.to_vec();
        sorted.sort_by_key(|sample| sample.at);

        let mut connection = self.connection.lock().unwrap();
        let transaction = connection
            .transaction()
            .map_err(|error| error.to_string())?;
        let mut inserted = 0;
        let mut previous: Option<f64> = None;
        for sample in sorted {
            if previous.is_some_and(|previous| {
                (previous - sample.used_percent).abs() < SAME_VALUE_TOLERANCE
            }) {
                continue;
            }
            previous = Some(sample.used_percent);
            inserted += transaction
                .execute(
                    "INSERT OR IGNORE INTO usage_samples (account_id, window, at, used_percent, source)
                     VALUES (?1, ?2, ?3, ?4, ?5)",
                    params![account_id, window.as_str(), sample.at.timestamp(), sample.used_percent, source],
                )
                .map_err(|error| error.to_string())?;
        }
        transaction.commit().map_err(|error| error.to_string())?;
        Ok(inserted)
    }

    /// 최근 `days` 일의 그래프 점. 기간이 시작되기 직전 값에서 출발하고, 마지막 값을 지금까지 이어 준다.
    pub fn trend(
        &self,
        account_id: &str,
        window: WindowKind,
        now: DateTime<Utc>,
        days: i64,
    ) -> Result<Vec<TrendPoint>, String> {
        let since = now - Duration::days(days);
        let connection = self.connection.lock().unwrap();

        let before: Option<f64> = connection
            .query_row(
                "SELECT used_percent FROM usage_samples
                 WHERE account_id = ?1 AND window = ?2 AND at < ?3
                 ORDER BY at DESC LIMIT 1",
                params![account_id, window.as_str(), since.timestamp()],
                |row| row.get(0),
            )
            .optional()
            .map_err(|error| error.to_string())?;

        let mut statement = connection
            .prepare(
                "SELECT at, used_percent FROM usage_samples
                 WHERE account_id = ?1 AND window = ?2 AND at >= ?3 AND at <= ?4
                 ORDER BY at",
            )
            .map_err(|error| error.to_string())?;
        let rows = statement
            .query_map(
                params![
                    account_id,
                    window.as_str(),
                    since.timestamp(),
                    now.timestamp()
                ],
                |row| Ok((row.get::<_, i64>(0)?, row.get::<_, f64>(1)?)),
            )
            .map_err(|error| error.to_string())?;

        let mut points = Vec::new();
        if let Some(percent) = before {
            points.push(TrendPoint { at: since, percent });
        }
        for row in rows {
            let (seconds, percent) = row.map_err(|error| error.to_string())?;
            if let Some(at) = Utc.timestamp_opt(seconds, 0).single() {
                points.push(TrendPoint { at, percent });
            }
        }
        if let Some(last) = points.last().cloned()
            && last.at < now
        {
            points.push(TrendPoint {
                at: now,
                percent: last.percent,
            });
        }
        Ok(points)
    }

    /// 계정을 지울 때 그 계정의 기록도 지운다.
    pub fn delete_account(&self, account_id: &str) -> Result<(), String> {
        self.connection
            .lock()
            .unwrap()
            .execute(
                "DELETE FROM usage_samples WHERE account_id = ?1",
                params![account_id],
            )
            .map(|_| ())
            .map_err(|error| error.to_string())
    }

    /// 한 번만 해야 하는 작업(예: Codex 로그 가져오기)을 했는지 기억해 두는 저장소
    pub fn meta(&self, key: &str) -> Option<String> {
        self.connection
            .lock()
            .unwrap()
            .query_row(
                "SELECT value FROM meta WHERE key = ?1",
                params![key],
                |row| row.get(0),
            )
            .optional()
            .ok()
            .flatten()
    }

    pub fn set_meta(&self, key: &str, value: &str) -> Result<(), String> {
        self.connection
            .lock()
            .unwrap()
            .execute(
                "INSERT OR REPLACE INTO meta (key, value) VALUES (?1, ?2)",
                params![key, value],
            )
            .map(|_| ())
            .map_err(|error| error.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(hours_ago: i64, now: DateTime<Utc>) -> DateTime<Utc> {
        now - Duration::hours(hours_ago)
    }

    #[test]
    fn records_only_changes() {
        let history = History::in_memory().unwrap();
        let now = Utc::now();
        assert!(
            history
                .record("a", WindowKind::Weekly, at(3, now), 10.0, "live")
                .unwrap()
        );
        assert!(
            !history
                .record("a", WindowKind::Weekly, at(2, now), 10.0, "live")
                .unwrap(),
            "같은 값은 건너뜀"
        );
        assert!(
            history
                .record("a", WindowKind::Weekly, at(1, now), 12.0, "live")
                .unwrap()
        );
        let trend = history.trend("a", WindowKind::Weekly, now, 7).unwrap();
        let values: Vec<f64> = trend.iter().map(|point| point.percent).collect();
        assert_eq!(
            values,
            vec![10.0, 12.0, 12.0],
            "마지막 값을 지금까지 이어 줌"
        );
    }

    #[test]
    fn trend_starts_from_value_before_the_window() {
        let history = History::in_memory().unwrap();
        let now = Utc::now();
        history
            .record(
                "a",
                WindowKind::Weekly,
                now - Duration::days(9),
                40.0,
                "live",
            )
            .unwrap();
        history
            .record("a", WindowKind::Weekly, at(5, now), 45.0, "live")
            .unwrap();
        let trend = history.trend("a", WindowKind::Weekly, now, 7).unwrap();
        assert_eq!(trend.first().unwrap().percent, 40.0);
        assert_eq!(trend.first().unwrap().at, now - Duration::days(7));
    }

    #[test]
    fn import_keeps_change_points_and_skips_duplicates() {
        let history = History::in_memory().unwrap();
        let now = Utc::now();
        let samples: Vec<Sample> = [(5, 10.0), (4, 10.0), (3, 20.0), (2, 20.0), (1, 0.0)]
            .iter()
            .map(|(hours, percent)| Sample {
                at: at(*hours, now),
                used_percent: *percent,
            })
            .collect();
        assert_eq!(
            history
                .import("a", WindowKind::Weekly, &samples, "codex-log")
                .unwrap(),
            3
        );
        assert_eq!(
            history
                .import("a", WindowKind::Weekly, &samples, "codex-log")
                .unwrap(),
            0,
            "두 번 넣어도 중복 없음"
        );
        history.delete_account("a").unwrap();
        assert!(
            history
                .trend("a", WindowKind::Weekly, now, 7)
                .unwrap()
                .is_empty()
        );
    }

    #[test]
    fn meta_round_trip() {
        let history = History::in_memory().unwrap();
        assert_eq!(history.meta("x"), None);
        history.set_meta("x", "done").unwrap();
        assert_eq!(history.meta("x").as_deref(), Some("done"));
    }
}
