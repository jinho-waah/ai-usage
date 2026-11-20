//! 사용률 기록(히스토리)을 SQLite 파일에 저장한다.
//!
//! - 값이 바뀔 때만 한 줄씩 남긴다. 같은 값이 계속되면 기록하지 않으므로 용량이 작고, 지우지 않고 영구 보관한다.
//! - 그래프는 "다음 기록 전까지는 같은 값"으로 보고 계단 모양으로 그린다.
//! - 저장 위치: `~/Library/Application Support/io.github.jinho-waah.ai-usage/usage.db`
//!
//! Rust 메모: SQLite 연결(`Connection`)은 한 번에 한 곳에서만 써야 해서 `Mutex` 로 감싼다.

use std::path::Path;
use std::sync::Mutex;

use chrono::{DateTime, TimeZone, Utc};
use rusqlite::{Connection, OptionalExtension, params};

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
    /// 그때 알려진 다음 리셋 시각. 주기(사이클)를 정확히 나누는 데 쓴다. 모르면 `None` (예전 기록)
    pub resets_at: Option<DateTime<Utc>>,
}

/// 같은 주기인지 볼 때 리셋 시각 차이를 이만큼까지는 같은 것으로 본다 (Codex 는 몇 초씩 흔들린다)
const SAME_RESET_TOLERANCE_SECONDS: i64 = 60 * 60;

/// 브랜드 하나의 누적 토큰 (DB 합계)
#[derive(Debug, Clone, PartialEq)]
pub struct BrandTokenTotals {
    /// "claude" | "codex"
    pub provider: String,
    pub input: u64,
    pub output: u64,
    pub cache_write: u64,
    pub cache_read: u64,
    pub reasoning: u64,
    /// 캐시를 포함한 합계
    pub total: u64,
    /// 오늘(로컬 날짜) 합계
    pub today: u64,
    /// 기록이 시작된 날 ("YYYY-MM-DD")
    pub since: Option<String>,
}

/// DB 표를 만든다. 사용률 기록(History)과 토큰 집계(tokens.rs)가 같은 파일을 쓰므로 한곳에서 정의한다.
pub fn ensure_schema(connection: &Connection) -> Result<(), String> {
    connection
        .execute_batch(
            "PRAGMA journal_mode = WAL;
             PRAGMA busy_timeout = 5000;
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
             );
             -- 실제 토큰 사용량: 브랜드·날짜·시간·모델·프로젝트·세션별 합계
             CREATE TABLE IF NOT EXISTS token_usage (
                 provider    TEXT NOT NULL,       -- 'claude' | 'codex'
                 day         TEXT NOT NULL,       -- 로컬 날짜 'YYYY-MM-DD'
                 hour        INTEGER NOT NULL,    -- 로컬 시간 0~23 (히트맵용)
                 model       TEXT NOT NULL,
                 project     TEXT NOT NULL,       -- 작업 폴더
                 session     TEXT NOT NULL,       -- 대화(세션) ID
                 input       INTEGER NOT NULL DEFAULT 0,  -- 캐시를 뺀 입력
                 output      INTEGER NOT NULL DEFAULT 0,
                 cache_write INTEGER NOT NULL DEFAULT 0,
                 cache_read  INTEGER NOT NULL DEFAULT 0,
                 reasoning   INTEGER NOT NULL DEFAULT 0,  -- Codex 추론 토큰 (출력에 포함된 값)
                 total       INTEGER NOT NULL DEFAULT 0,  -- 캐시 포함 합계
                 PRIMARY KEY (provider, day, hour, model, project, session)
             );
             -- 이미 센 Claude 응답 (같은 응답을 두 번 세지 않게).
             -- 키 하나뿐인 표라 WITHOUT ROWID 로 두면 키를 한 번만 저장해서 크기가 절반이 된다.
             CREATE TABLE IF NOT EXISTS token_seen (
                 key TEXT PRIMARY KEY
             ) WITHOUT ROWID;
             -- Codex 세션별로 마지막에 본 누적 토큰 (증가분만 더하려고)
             CREATE TABLE IF NOT EXISTS codex_sessions (
                 session_id  TEXT PRIMARY KEY,
                 input       INTEGER NOT NULL,
                 cached      INTEGER NOT NULL,
                 cache_write INTEGER NOT NULL,
                 output      INTEGER NOT NULL,
                 reasoning   INTEGER NOT NULL,
                 total       INTEGER NOT NULL
             );
             -- 로그 파일마다 어디까지 읽었는지
             CREATE TABLE IF NOT EXISTS log_files (
                 path    TEXT PRIMARY KEY,
                 offset  INTEGER NOT NULL,
                 context TEXT NOT NULL DEFAULT ''   -- Codex: 이어서 읽을 때 필요한 세션·모델·폴더
             );",
        )
        .map_err(|error| error.to_string())?;
    // 예전 DB 의 token_seen 은 보통 표라서 키를 표와 인덱스에 두 번 저장한다 → 한 번 옮긴다.
    let old_shape: bool = connection
        .query_row(
            "SELECT sql NOT LIKE '%WITHOUT ROWID%' FROM sqlite_master WHERE name = 'token_seen'",
            [],
            |row| row.get(0),
        )
        .unwrap_or(false);
    if old_shape {
        let moved = connection.execute_batch(
            "BEGIN;
             CREATE TABLE token_seen_new (key TEXT PRIMARY KEY) WITHOUT ROWID;
             INSERT INTO token_seen_new SELECT key FROM token_seen;
             DROP TABLE token_seen;
             ALTER TABLE token_seen_new RENAME TO token_seen;
             COMMIT;",
        );
        if let Err(error) = moved {
            let _ = connection.execute_batch("ROLLBACK;");
            return Err(error.to_string());
        }
        // 비워진 자리를 파일에서 실제로 돌려준다 (실패해도 동작에는 문제 없다)
        let _ = connection.execute_batch("VACUUM;");
    }

    // 예전 DB 에는 리셋 시각 열이 없다 → 덧붙인다 (예전 기록은 비어 있음)
    if connection
        .prepare("SELECT resets_at FROM usage_samples LIMIT 0")
        .is_err()
    {
        connection
            .execute_batch("ALTER TABLE usage_samples ADD COLUMN resets_at INTEGER;")
            .map_err(|error| error.to_string())?;
    }
    Ok(())
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
        ensure_schema(&connection)?;
        Ok(History {
            connection: Mutex::new(connection),
        })
    }

    /// 브랜드(Claude, Codex)별 누적 토큰. 토큰 집계(tokens.rs)가 채운 표에서 합계만 읽는다.
    /// `today` 는 로컬 날짜 "YYYY-MM-DD".
    pub fn token_totals(&self, today: &str) -> Result<Vec<BrandTokenTotals>, String> {
        let connection = self.connection.lock().unwrap();
        let mut statement = connection
            .prepare(
                "SELECT provider, SUM(input), SUM(output), SUM(cache_write), SUM(cache_read),
                        SUM(reasoning), SUM(total), MIN(day),
                        SUM(CASE WHEN day = ?1 THEN total ELSE 0 END)
                 FROM token_usage GROUP BY provider ORDER BY provider",
            )
            .map_err(|error| error.to_string())?;
        let rows = statement
            .query_map(params![today], |row| {
                Ok(BrandTokenTotals {
                    provider: row.get(0)?,
                    input: row.get::<_, i64>(1)?.max(0) as u64,
                    output: row.get::<_, i64>(2)?.max(0) as u64,
                    cache_write: row.get::<_, i64>(3)?.max(0) as u64,
                    cache_read: row.get::<_, i64>(4)?.max(0) as u64,
                    reasoning: row.get::<_, i64>(5)?.max(0) as u64,
                    total: row.get::<_, i64>(6)?.max(0) as u64,
                    since: row.get(7)?,
                    today: row.get::<_, i64>(8)?.max(0) as u64,
                })
            })
            .map_err(|error| error.to_string())?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|error| error.to_string())
    }

    /// 사용률을 기록한다. 바로 앞 기록과 값이 같으면 남기지 않는다.
    /// (값이 같아도 리셋 시각이 바뀌었으면 새 주기가 시작된 것이라 남긴다) 기록했으면 true.
    pub fn record(
        &self,
        account_id: &str,
        window: WindowKind,
        sample: &Sample,
        source: &str,
    ) -> Result<bool, String> {
        let connection = self.connection.lock().unwrap();
        let previous: Option<(f64, Option<i64>)> = connection
            .query_row(
                "SELECT used_percent, resets_at FROM usage_samples
                 WHERE account_id = ?1 AND window = ?2 AND at <= ?3
                 ORDER BY at DESC LIMIT 1",
                params![account_id, window.as_str(), sample.at.timestamp()],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()
            .map_err(|error| error.to_string())?;
        if let Some((previous_percent, previous_reset)) = previous {
            let same_value = (previous_percent - sample.used_percent).abs() < SAME_VALUE_TOLERANCE;
            let reset_changed = match (previous_reset, sample.resets_at) {
                (Some(before), Some(now)) => {
                    (now.timestamp() - before).abs() > SAME_RESET_TOLERANCE_SECONDS
                }
                _ => false,
            };
            if same_value && !reset_changed {
                return Ok(false);
            }
        }
        connection
            .execute(
                "INSERT OR REPLACE INTO usage_samples (account_id, window, at, used_percent, source, resets_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![
                    account_id,
                    window.as_str(),
                    sample.at.timestamp(),
                    sample.used_percent,
                    source,
                    sample.resets_at.map(|at| at.timestamp())
                ],
            )
            .map_err(|error| error.to_string())?;
        Ok(true)
    }

    /// `from` ~ `to` 사이의 기록. `from` 이 있으면 그 직전 기록 하나도 맨 앞에 붙인다 (그래프가 기간 시작부터 이어지게).
    pub fn samples(
        &self,
        account_id: &str,
        window: WindowKind,
        from: Option<DateTime<Utc>>,
        to: DateTime<Utc>,
    ) -> Result<Vec<Sample>, String> {
        let connection = self.connection.lock().unwrap();
        let from_seconds = from.map(|from| from.timestamp()).unwrap_or(i64::MIN);
        let read = |row: &rusqlite::Row| -> rusqlite::Result<(i64, f64, Option<i64>)> {
            Ok((row.get(0)?, row.get(1)?, row.get(2)?))
        };

        let mut rows: Vec<(i64, f64, Option<i64>)> = Vec::new();
        if from.is_some()
            && let Some(before) = connection
                .query_row(
                    "SELECT at, used_percent, resets_at FROM usage_samples
                     WHERE account_id = ?1 AND window = ?2 AND at < ?3
                     ORDER BY at DESC LIMIT 1",
                    params![account_id, window.as_str(), from_seconds],
                    read,
                )
                .optional()
                .map_err(|error| error.to_string())?
        {
            rows.push(before);
        }
        let mut statement = connection
            .prepare(
                "SELECT at, used_percent, resets_at FROM usage_samples
                 WHERE account_id = ?1 AND window = ?2 AND at >= ?3 AND at <= ?4
                 ORDER BY at",
            )
            .map_err(|error| error.to_string())?;
        let found = statement
            .query_map(
                params![account_id, window.as_str(), from_seconds, to.timestamp()],
                read,
            )
            .map_err(|error| error.to_string())?;
        for row in found {
            rows.push(row.map_err(|error| error.to_string())?);
        }

        let time = |seconds: i64| Utc.timestamp_opt(seconds, 0).single();
        Ok(rows
            .into_iter()
            .filter_map(|(at, used_percent, resets_at)| {
                Some(Sample {
                    at: time(at)?,
                    used_percent,
                    resets_at: resets_at.and_then(time),
                })
            })
            .collect())
    }

    /// 지난 기록을 한꺼번에 넣는다 (Codex 로그, 원본 앱 기록 가져오기).
    /// 값이 바뀌는 지점만 남겨서 넣는다. 같은 시각 기록이 이미 있으면 비어 있는 리셋 시각만 채운다.
    /// 새로 넣거나 채운 개수를 돌려준다.
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
                    "INSERT INTO usage_samples (account_id, window, at, used_percent, source, resets_at)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6)
                     ON CONFLICT (account_id, window, at) DO UPDATE SET resets_at = excluded.resets_at
                     WHERE usage_samples.resets_at IS NULL AND excluded.resets_at IS NOT NULL",
                    params![
                        account_id,
                        window.as_str(),
                        sample.at.timestamp(),
                        sample.used_percent,
                        source,
                        sample.resets_at.map(|at| at.timestamp())
                    ],
                )
                .map_err(|error| error.to_string())?;
        }
        transaction.commit().map_err(|error| error.to_string())?;
        Ok(inserted)
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
    use chrono::Duration;

    fn sample(at: DateTime<Utc>, used_percent: f64) -> Sample {
        Sample {
            at,
            used_percent,
            resets_at: None,
        }
    }

    fn at(hours_ago: i64, now: DateTime<Utc>) -> DateTime<Utc> {
        now - Duration::hours(hours_ago)
    }

    #[test]
    fn records_only_changes() {
        let history = History::in_memory().unwrap();
        let now = Utc::now();
        assert!(
            history
                .record("a", WindowKind::Weekly, &sample(at(3, now), 10.0), "live")
                .unwrap()
        );
        assert!(
            !history
                .record("a", WindowKind::Weekly, &sample(at(2, now), 10.0), "live")
                .unwrap(),
            "같은 값은 건너뜀"
        );
        assert!(
            history
                .record("a", WindowKind::Weekly, &sample(at(1, now), 12.0), "live")
                .unwrap()
        );
        let samples = history.samples("a", WindowKind::Weekly, None, now).unwrap();
        let values: Vec<f64> = samples.iter().map(|sample| sample.used_percent).collect();
        assert_eq!(values, vec![10.0, 12.0], "같은 값(10)은 한 번만 남는다");
    }

    #[test]
    fn same_value_is_recorded_again_when_the_reset_time_changes() {
        let history = History::in_memory().unwrap();
        let now = Utc::now();
        let with_reset = |hours_ago: i64, percent: f64, reset_in_days: i64| Sample {
            at: at(hours_ago, now),
            used_percent: percent,
            resets_at: Some(now + Duration::days(reset_in_days)),
        };
        assert!(
            history
                .record("a", WindowKind::Weekly, &with_reset(3, 0.0, 1), "live")
                .unwrap()
        );
        // 같은 주기, 같은 값 → 건너뜀
        assert!(
            !history
                .record("a", WindowKind::Weekly, &with_reset(2, 0.0, 1), "live")
                .unwrap()
        );
        // 새 주기(리셋 시각이 일주일 뒤로) → 값이 같아도 남김
        assert!(
            history
                .record("a", WindowKind::Weekly, &with_reset(1, 0.0, 8), "live")
                .unwrap()
        );

        let samples = history
            .samples("a", WindowKind::Weekly, Some(at(2, now)), now)
            .unwrap();
        assert_eq!(samples.len(), 2, "기간 직전 기록 하나 + 기간 안 기록 하나");
        assert_eq!(
            samples[1].resets_at,
            Some(now + Duration::days(8)).map(|t| Utc.timestamp_opt(t.timestamp(), 0).unwrap())
        );
    }

    #[test]
    fn samples_include_the_value_before_the_range() {
        let history = History::in_memory().unwrap();
        let now = Utc::now();
        history
            .record(
                "a",
                WindowKind::Weekly,
                &sample(now - Duration::days(9), 40.0),
                "live",
            )
            .unwrap();
        history
            .record("a", WindowKind::Weekly, &sample(at(5, now), 45.0), "live")
            .unwrap();
        // 기간(최근 7일) 앞의 마지막 기록도 맨 앞에 붙여 준다 (그래프가 기간 시작부터 이어지게)
        let samples = history
            .samples("a", WindowKind::Weekly, Some(now - Duration::days(7)), now)
            .unwrap();
        assert_eq!(samples.len(), 2);
        assert_eq!(samples[0].used_percent, 40.0);
        assert_eq!(samples[1].used_percent, 45.0);
    }

    #[test]
    fn import_keeps_change_points_and_skips_duplicates() {
        let history = History::in_memory().unwrap();
        let now = Utc::now();
        let samples: Vec<Sample> = [(5, 10.0), (4, 10.0), (3, 20.0), (2, 20.0), (1, 0.0)]
            .iter()
            .map(|(hours, percent)| Sample {
                resets_at: None,
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
                .samples("a", WindowKind::Weekly, None, now)
                .unwrap()
                .is_empty()
        );
    }

    #[test]
    fn old_token_seen_table_is_moved_without_losing_keys() {
        // 예전 모양(보통 표)으로 만들어 키를 넣어 둔다
        let connection = Connection::open_in_memory().unwrap();
        connection
            .execute_batch(
                "CREATE TABLE token_seen (key TEXT PRIMARY KEY);
                 INSERT INTO token_seen (key) VALUES ('claude:m1:r1'), ('claude:m2:r2');",
            )
            .unwrap();

        ensure_schema(&connection).unwrap();

        let shape: String = connection
            .query_row(
                "SELECT sql FROM sqlite_master WHERE name = 'token_seen'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert!(shape.contains("WITHOUT ROWID"));
        let count: i64 = connection
            .query_row("SELECT COUNT(*) FROM token_seen", [], |row| row.get(0))
            .unwrap();
        assert_eq!(count, 2, "키는 그대로 남는다");
        // 이미 센 응답은 여전히 걸러진다
        let inserted = connection
            .execute(
                "INSERT OR IGNORE INTO token_seen (key) VALUES ('claude:m1:r1')",
                [],
            )
            .unwrap();
        assert_eq!(inserted, 0);
        // 두 번 불러도 다시 옮기지 않는다
        ensure_schema(&connection).unwrap();
    }

    #[test]
    fn meta_round_trip() {
        let history = History::in_memory().unwrap();
        assert_eq!(history.meta("x"), None);
        history.set_meta("x", "done").unwrap();
        assert_eq!(history.meta("x").as_deref(), Some("done"));
    }
}
