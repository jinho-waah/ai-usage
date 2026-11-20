//! Claude Code 와 Codex 가 이 Mac 에 남긴 로그에서 실제 토큰 사용량을 모아 기록 DB 에 저장한다.
//!
//! - Claude Code: `~/.claude/projects/**/*.jsonl` (하위 에이전트 로그 포함)
//!   응답(assistant) 줄의 `message.usage` 를 센다. 같은 응답이 여러 줄·여러 파일에 나와도
//!   메시지 ID 로 한 번만 센다.
//! - Codex: `~/.codex/sessions/**/*.jsonl`
//!   `token_count` 이벤트의 세션 누적값에서 **늘어난 만큼만** 더한다.
//! - 파일마다 어디까지 읽었는지 기억해 두고, 다음에는 새로 추가된 줄만 읽는다.
//! - 원본 로그가 지워져도(Claude Code 는 기본 30일 뒤 삭제) DB 에 남은 합계는 그대로다.
//!
//! 처음 한 번은 로그 전체(수 GB)를 읽으므로 반드시 별도 스레드에서 부를 것.

use std::fs;
use std::io::{BufRead, BufReader, Seek, SeekFrom};
use std::path::{Path, PathBuf};

use chrono::{DateTime, Local, Timelike, Utc};
use rusqlite::{Connection, OptionalExtension, Transaction, params};
use serde_json::Value;

use crate::history::ensure_schema;

/// 토큰 종류별 값
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Tokens {
    /// 캐시를 뺀 입력
    pub input: u64,
    pub output: u64,
    pub cache_write: u64,
    pub cache_read: u64,
    /// 추론 토큰 (Codex, 출력에 포함된 값이라 합계에 따로 더하지 않음)
    pub reasoning: u64,
    /// 캐시 포함 합계
    pub total: u64,
}

/// 한 번 훑은 결과 요약
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ScanReport {
    pub files_read: usize,
    pub claude_records: usize,
    pub codex_records: usize,
}

/// 로그를 찾을 폴더들
#[derive(Debug, Clone)]
pub struct LogDirs {
    pub claude: Vec<PathBuf>,
    pub codex: Vec<PathBuf>,
}

impl LogDirs {
    /// 이 Mac 의 기본 위치
    pub fn default_locations() -> LogDirs {
        let home = PathBuf::from(std::env::var("HOME").unwrap_or_default());
        let claude = [
            home.join(".claude/projects"),
            home.join(".config/claude/projects"),
        ]
        .into_iter()
        .filter(|dir| dir.is_dir())
        .collect();
        let codex_home = crate::codex::codex_home();
        let codex = [
            codex_home.join("sessions"),
            codex_home.join("archived_sessions"),
        ]
        .into_iter()
        .filter(|dir| dir.is_dir())
        .collect();
        LogDirs { claude, codex }
    }
}

/// DB 파일을 열고 로그를 훑어서 새로 생긴 토큰 사용량을 더한다.
pub fn scan(db_path: &Path, dirs: &LogDirs) -> Result<ScanReport, String> {
    let mut connection = Connection::open(db_path).map_err(|error| error.to_string())?;
    scan_with(&mut connection, dirs)
}

pub fn scan_with(connection: &mut Connection, dirs: &LogDirs) -> Result<ScanReport, String> {
    ensure_schema(connection)?;
    let mut report = ScanReport::default();
    for path in jsonl_files(&dirs.claude) {
        if let Some(count) = scan_file(connection, &path, FileKind::Claude)? {
            report.files_read += 1;
            report.claude_records += count;
        }
    }
    for path in jsonl_files(&dirs.codex) {
        if let Some(count) = scan_file(connection, &path, FileKind::Codex)? {
            report.files_read += 1;
            report.codex_records += count;
        }
    }
    Ok(report)
}

#[derive(Clone, Copy)]
enum FileKind {
    Claude,
    Codex,
}

/// Codex 파일을 이어서 읽을 때 필요한 정보
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
struct CodexContext {
    session_id: String,
    model: String,
    cwd: String,
}

/// 파일 하나에서 새로 추가된 부분만 읽는다. 읽을 게 없으면 `None`, 읽었으면 센 기록 수.
fn scan_file(
    connection: &mut Connection,
    path: &Path,
    kind: FileKind,
) -> Result<Option<usize>, String> {
    let key = path.to_string_lossy().to_string();
    let Ok(size) = fs::metadata(path).map(|meta| meta.len()) else {
        return Ok(None);
    };
    let stored: Option<(i64, String)> = connection
        .query_row(
            "SELECT offset, context FROM log_files WHERE path = ?1",
            params![key],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()
        .map_err(|error| error.to_string())?;
    let (mut offset, context_text) = stored.unwrap_or((0, String::new()));
    let mut context: CodexContext = serde_json::from_str(&context_text).unwrap_or_default();
    // 파일이 줄었으면(새로 쓰인 파일) 처음부터 다시 읽는다. 중복은 위의 규칙으로 걸러진다.
    if (size as i64) < offset {
        offset = 0;
        context = CodexContext::default();
    }
    if size as i64 == offset {
        return Ok(None);
    }

    let mut file = fs::File::open(path).map_err(|error| error.to_string())?;
    file.seek(SeekFrom::Start(offset as u64))
        .map_err(|error| error.to_string())?;
    let mut reader = BufReader::new(file);

    let transaction = connection
        .transaction()
        .map_err(|error| error.to_string())?;
    let mut counted = 0;
    let mut line = Vec::new();
    loop {
        line.clear();
        let read = reader
            .read_until(b'\n', &mut line)
            .map_err(|error| error.to_string())?;
        // 마지막 줄이 아직 다 쓰이지 않았으면(줄바꿈 없음) 다음 번에 다시 읽는다
        if read == 0 || line.last() != Some(&b'\n') {
            break;
        }
        offset += read as i64;
        let Ok(text) = std::str::from_utf8(&line) else {
            continue;
        };
        let added = match kind {
            FileKind::Claude => claude_line(&transaction, text)?,
            FileKind::Codex => codex_line(&transaction, text, &mut context)?,
        };
        if added {
            counted += 1;
        }
    }

    let context_text = serde_json::to_string(&context).unwrap_or_default();
    transaction
        .execute(
            "INSERT OR REPLACE INTO log_files (path, offset, context) VALUES (?1, ?2, ?3)",
            params![key, offset, context_text],
        )
        .map_err(|error| error.to_string())?;
    transaction.commit().map_err(|error| error.to_string())?;
    Ok(Some(counted))
}

/// Claude Code 로그 한 줄. 새로 센 응답이면 true.
fn claude_line(transaction: &Transaction, text: &str) -> Result<bool, String> {
    // 전체를 해석하기 전에 글자로 먼저 걸러서 빠르게 넘긴다
    if !text.contains("\"usage\"") || !text.contains("\"assistant\"") {
        return Ok(false);
    }
    let Ok(event) = serde_json::from_str::<Value>(text) else {
        return Ok(false);
    };
    if event.get("type").and_then(Value::as_str) != Some("assistant") {
        return Ok(false);
    }
    let Some(message) = event.get("message") else {
        return Ok(false);
    };
    let model = message
        .get("model")
        .and_then(Value::as_str)
        .unwrap_or("unknown");
    if model == "<synthetic>" {
        return Ok(false);
    }
    let (Some(usage), Some(message_id)) = (
        message.get("usage"),
        message.get("id").and_then(Value::as_str),
    ) else {
        return Ok(false);
    };
    let Some(at) = timestamp(&event) else {
        return Ok(false);
    };

    let count = |key: &str| usage.get(key).and_then(Value::as_u64).unwrap_or(0);
    let tokens = Tokens {
        input: count("input_tokens"),
        output: count("output_tokens"),
        cache_write: count("cache_creation_input_tokens"),
        cache_read: count("cache_read_input_tokens"),
        reasoning: 0,
        total: count("input_tokens")
            + count("output_tokens")
            + count("cache_creation_input_tokens")
            + count("cache_read_input_tokens"),
    };
    if tokens.total == 0 {
        return Ok(false);
    }

    // 같은 응답은 한 번만: 메시지 ID + 요청 ID
    let request_id = event
        .get("requestId")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let inserted = transaction
        .execute(
            "INSERT OR IGNORE INTO token_seen (key) VALUES (?1)",
            params![format!("claude:{message_id}:{request_id}")],
        )
        .map_err(|error| error.to_string())?;
    if inserted == 0 {
        return Ok(false);
    }

    let project = event.get("cwd").and_then(Value::as_str).unwrap_or_default();
    let session = event
        .get("sessionId")
        .and_then(Value::as_str)
        .unwrap_or_default();
    add_usage(transaction, "claude", at, model, project, session, tokens)?;
    Ok(true)
}

/// Codex 로그 한 줄. 새로 늘어난 토큰을 더했으면 true.
fn codex_line(
    transaction: &Transaction,
    text: &str,
    context: &mut CodexContext,
) -> Result<bool, String> {
    let is_interesting = text.contains("\"session_meta\"")
        || text.contains("\"turn_context\"")
        || text.contains("\"token_count\"");
    if !is_interesting {
        return Ok(false);
    }
    let Ok(event) = serde_json::from_str::<Value>(text) else {
        return Ok(false);
    };
    let payload = event.get("payload").cloned().unwrap_or(Value::Null);
    let string =
        |value: &Value, key: &str| value.get(key).and_then(Value::as_str).map(String::from);

    match event.get("type").and_then(Value::as_str) {
        Some("session_meta") => {
            if let Some(id) = string(&payload, "id") {
                context.session_id = id;
            }
            if let Some(cwd) = string(&payload, "cwd") {
                context.cwd = cwd;
            }
            return Ok(false);
        }
        Some("turn_context") => {
            if let Some(model) = string(&payload, "model") {
                context.model = model;
            }
            if let Some(cwd) = string(&payload, "cwd") {
                context.cwd = cwd;
            }
            return Ok(false);
        }
        Some("event_msg") if payload.get("type").and_then(Value::as_str) == Some("token_count") => {
        }
        _ => return Ok(false),
    }

    let Some(total_usage) = payload.pointer("/info/total_token_usage") else {
        return Ok(false);
    };
    let (Some(at), false) = (timestamp(&event), context.session_id.is_empty()) else {
        return Ok(false);
    };
    let count = |key: &str| total_usage.get(key).and_then(Value::as_u64).unwrap_or(0);
    let current = [
        count("input_tokens"),
        count("cached_input_tokens"),
        count("cache_write_input_tokens"),
        count("output_tokens"),
        count("reasoning_output_tokens"),
        count("total_tokens"),
    ];

    let previous: Option<[u64; 6]> = transaction
        .query_row(
            "SELECT input, cached, cache_write, output, reasoning, total FROM codex_sessions WHERE session_id = ?1",
            params![context.session_id],
            |row| {
                Ok([
                    row.get::<_, i64>(0)? as u64,
                    row.get::<_, i64>(1)? as u64,
                    row.get::<_, i64>(2)? as u64,
                    row.get::<_, i64>(3)? as u64,
                    row.get::<_, i64>(4)? as u64,
                    row.get::<_, i64>(5)? as u64,
                ])
            },
        )
        .optional()
        .map_err(|error| error.to_string())?;
    // 누적값이 줄었으면 새로 시작한 것으로 보고 지금 값을 그대로 쓴다
    let previous = previous
        .filter(|previous| current[5] >= previous[5])
        .unwrap_or([0; 6]);
    let delta: Vec<u64> = current
        .iter()
        .zip(previous)
        .map(|(now, before)| now.saturating_sub(before))
        .collect();
    if delta[5] == 0 {
        return Ok(false);
    }

    transaction
        .execute(
            "INSERT OR REPLACE INTO codex_sessions (session_id, input, cached, cache_write, output, reasoning, total)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                context.session_id,
                current[0] as i64,
                current[1] as i64,
                current[2] as i64,
                current[3] as i64,
                current[4] as i64,
                current[5] as i64
            ],
        )
        .map_err(|error| error.to_string())?;

    // Codex 의 input 은 캐시 입력을 포함하므로 캐시만큼 뺀다. total 은 입력(캐시 포함) + 출력.
    let tokens = Tokens {
        input: delta[0].saturating_sub(delta[1]),
        cache_read: delta[1],
        cache_write: delta[2],
        output: delta[3],
        reasoning: delta[4],
        total: delta[5],
    };
    let model = if context.model.is_empty() {
        "unknown"
    } else {
        context.model.as_str()
    };
    add_usage(
        transaction,
        "codex",
        at,
        model,
        &context.cwd,
        &context.session_id,
        tokens,
    )?;
    Ok(true)
}

/// 날짜·시간·모델·프로젝트·세션별 합계에 더한다.
fn add_usage(
    transaction: &Transaction,
    provider: &str,
    at: DateTime<Utc>,
    model: &str,
    project: &str,
    session: &str,
    tokens: Tokens,
) -> Result<(), String> {
    let local = at.with_timezone(&Local);
    transaction
        .execute(
            "INSERT INTO token_usage
                 (provider, day, hour, model, project, session, input, output, cache_write, cache_read, reasoning, total)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)
             ON CONFLICT (provider, day, hour, model, project, session) DO UPDATE SET
                 input = input + excluded.input,
                 output = output + excluded.output,
                 cache_write = cache_write + excluded.cache_write,
                 cache_read = cache_read + excluded.cache_read,
                 reasoning = reasoning + excluded.reasoning,
                 total = total + excluded.total",
            params![
                provider,
                local.format("%Y-%m-%d").to_string(),
                local.hour() as i64,
                model,
                project,
                session,
                tokens.input as i64,
                tokens.output as i64,
                tokens.cache_write as i64,
                tokens.cache_read as i64,
                tokens.reasoning as i64,
                tokens.total as i64
            ],
        )
        .map(|_| ())
        .map_err(|error| error.to_string())
}

fn timestamp(event: &Value) -> Option<DateTime<Utc>> {
    let text = event.get("timestamp")?.as_str()?;
    DateTime::parse_from_rfc3339(text)
        .ok()
        .map(|time| time.with_timezone(&Utc))
}

/// 폴더들 아래의 모든 .jsonl 파일 (하위 폴더 포함)
fn jsonl_files(dirs: &[PathBuf]) -> Vec<PathBuf> {
    fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
        let Ok(entries) = fs::read_dir(dir) else {
            return;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                walk(&path, out);
            } else if path.extension().is_some_and(|ext| ext == "jsonl") {
                out.push(path);
            }
        }
    }
    let mut files = Vec::new();
    for dir in dirs {
        walk(dir, &mut files);
    }
    files.sort();
    files
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    /// 테스트마다 새 임시 폴더
    fn temp_dir(name: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("ai-usage-test-{name}-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn append(path: &Path, lines: &[String]) {
        let mut file = fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
            .unwrap();
        for line in lines {
            writeln!(file, "{line}").unwrap();
        }
    }

    fn claude_line(id: &str, request: &str, input: u64, output: u64, cache_read: u64) -> String {
        serde_json::json!({
            "type": "assistant", "timestamp": "2026-11-20T10:00:00Z", "cwd": "/work/app",
            "sessionId": "s1", "requestId": request,
            "message": { "id": id, "model": "claude-opus-5-5",
                "usage": { "input_tokens": input, "output_tokens": output,
                           "cache_creation_input_tokens": 0, "cache_read_input_tokens": cache_read } }
        })
        .to_string()
    }

    fn totals(connection: &Connection, provider: &str) -> (u64, u64) {
        connection
            .query_row(
                "SELECT COALESCE(SUM(total), 0), COALESCE(SUM(cache_read), 0) FROM token_usage WHERE provider = ?1",
                params![provider],
                |row| Ok((row.get::<_, i64>(0)? as u64, row.get::<_, i64>(1)? as u64)),
            )
            .unwrap()
    }

    #[test]
    fn claude_counts_each_response_once_and_reads_only_new_lines() {
        let dir = temp_dir("claude");
        let file = dir.join("session.jsonl");
        append(
            &file,
            &[
                claude_line("m1", "r1", 10, 5, 100),
                // 같은 응답이 여러 줄로 기록된 경우 → 한 번만
                claude_line("m1", "r1", 10, 5, 100),
                "{\"type\":\"user\",\"message\":{\"content\":\"hi\"}}".to_string(),
            ],
        );
        let dirs = LogDirs {
            claude: vec![dir.clone()],
            codex: vec![],
        };
        let mut connection = Connection::open_in_memory().unwrap();

        let report = scan_with(&mut connection, &dirs).unwrap();
        assert_eq!(report.claude_records, 1);
        assert_eq!(totals(&connection, "claude"), (115, 100));

        // 새 줄이 붙으면 그 줄만 읽는다
        append(&file, &[claude_line("m2", "r2", 1, 1, 0)]);
        let report = scan_with(&mut connection, &dirs).unwrap();
        assert_eq!(report.claude_records, 1);
        assert_eq!(totals(&connection, "claude").0, 117);

        // 바뀐 게 없으면 파일을 읽지 않는다
        assert_eq!(scan_with(&mut connection, &dirs).unwrap().files_read, 0);
    }

    #[test]
    fn codex_adds_only_increases_of_session_totals() {
        let dir = temp_dir("codex");
        let file = dir.join("rollout.jsonl");
        let token_count = |total_input: u64, cached: u64, output: u64| {
            serde_json::json!({
                "timestamp": "2026-11-20T10:00:00Z", "type": "event_msg",
                "payload": { "type": "token_count", "info": { "total_token_usage": {
                    "input_tokens": total_input, "cached_input_tokens": cached, "cache_write_input_tokens": 0,
                    "output_tokens": output, "reasoning_output_tokens": 0, "total_tokens": total_input + output } } }
            })
            .to_string()
        };
        append(
            &file,
            &[
                serde_json::json!({"type": "session_meta", "payload": {"id": "sess-1", "cwd": "/work/app"}}).to_string(),
                serde_json::json!({"type": "turn_context", "payload": {"model": "gpt-6-astra"}}).to_string(),
                token_count(100, 60, 10),
                token_count(100, 60, 10), // 같은 누적값 반복 → 0
                token_count(250, 150, 30),
            ],
        );
        let dirs = LogDirs {
            claude: vec![],
            codex: vec![dir.clone()],
        };
        let mut connection = Connection::open_in_memory().unwrap();
        let report = scan_with(&mut connection, &dirs).unwrap();
        assert_eq!(report.codex_records, 2);
        // 마지막 누적값만큼: 입력 250 + 출력 30 = 280, 그중 캐시 150
        assert_eq!(totals(&connection, "codex"), (280, 150));

        let model: String = connection
            .query_row(
                "SELECT model FROM token_usage WHERE provider = 'codex' LIMIT 1",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(model, "gpt-6-astra");
    }

    #[test]
    fn partial_last_line_waits_for_the_next_scan() {
        let dir = temp_dir("partial");
        let file = dir.join("session.jsonl");
        let line = claude_line("m1", "r1", 3, 3, 0);
        fs::write(&file, &line[..line.len() / 2]).unwrap(); // 줄이 반만 쓰인 상태
        let dirs = LogDirs {
            claude: vec![dir.clone()],
            codex: vec![],
        };
        let mut connection = Connection::open_in_memory().unwrap();
        scan_with(&mut connection, &dirs).unwrap();
        assert_eq!(totals(&connection, "claude").0, 0);

        fs::write(&file, format!("{line}\n")).unwrap(); // 마저 쓰임
        scan_with(&mut connection, &dirs).unwrap();
        assert_eq!(totals(&connection, "claude").0, 6);
    }
}
