//! OpenAI Codex 사용량.
//!
//! Codex CLI 가 로그인할 때 저장한 `~/.codex/auth.json`(또는 `$CODEX_HOME/auth.json`)을
//! **읽기만** 해서 사용량 API 를 부른다. 토큰을 갱신하거나 파일을 고치지 않는다.
//! 토큰이 만료돼 API 를 못 부르면, Codex 세션 로그에 남은 마지막 한도 정보로 대신 보여 준다.
//! (토큰 갱신은 Codex CLI 를 한 번 실행하면 Codex 가 알아서 한다)

use std::fs;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::time::Duration;

use base64::Engine;
use chrono::{DateTime, TimeZone, Utc};
use serde_json::Value;

use crate::model::{RawWindow, UsageReading};

const USAGE_URL: &str = "https://chatgpt.com/backend-api/wham/usage";
const SESSION_WINDOW_SECONDS: i64 = 5 * 60 * 60;
const WEEK_WINDOW_SECONDS: i64 = 7 * 24 * 60 * 60;
/// 세션 로그 파일에서 끝부분만 읽는다 (파일이 클 수 있어서)
const LOG_TAIL_BYTES: u64 = 512 * 1024;

/// Codex 로그인 정보 중 사용량 조회에 필요한 것
#[derive(Debug, Clone, PartialEq)]
pub struct Credentials {
    pub access_token: String,
    pub account_id: Option<String>,
}

/// Codex 에서 받아 온 사용량과 플랜
#[derive(Debug, Clone, PartialEq, Default)]
pub struct CodexUsage {
    pub reading: UsageReading,
    pub plan: Option<String>,
}

/// 사용량 API 호출 결과
#[derive(Debug, Clone, PartialEq)]
pub enum ApiOutcome {
    Ok(CodexUsage),
    /// 토큰 만료 등 (401/403)
    Unauthorized,
    Failed(String),
}

/// Codex 설정 폴더: `$CODEX_HOME` 이 있으면 그것, 없으면 `~/.codex`
pub fn codex_home() -> PathBuf {
    if let Ok(home) = std::env::var("CODEX_HOME") {
        let home = home.trim();
        if !home.is_empty() {
            return PathBuf::from(home);
        }
    }
    let user_home = std::env::var("HOME").unwrap_or_default();
    PathBuf::from(user_home).join(".codex")
}

/// `auth.json` 에서 토큰과 계정 ID 를 읽는다.
pub fn read_credentials(home: &Path) -> Result<Credentials, String> {
    let text = fs::read_to_string(home.join("auth.json")).map_err(|_| {
        "Codex CLI 로그인 정보가 없어요. 터미널에서 `codex login` 을 먼저 해 주세요.".to_string()
    })?;
    let json: Value =
        serde_json::from_str(&text).map_err(|_| "Codex 로그인 파일을 읽지 못했어요".to_string())?;
    parse_credentials(&json)
}

pub fn parse_credentials(json: &Value) -> Result<Credentials, String> {
    let tokens = json.get("tokens").ok_or(
        "Codex 가 ChatGPT 계정으로 로그인돼 있지 않아요 (API 키 로그인은 사용량 정보가 없어요)",
    )?;
    let field = |snake: &str, camel: &str| {
        tokens
            .get(snake)
            .or_else(|| tokens.get(camel))
            .and_then(Value::as_str)
            .filter(|value| !value.is_empty())
            .map(String::from)
    };
    let access_token = field("access_token", "accessToken").ok_or("Codex 로그인 토큰이 없어요")?;
    let account_id = field("account_id", "accountId").or_else(|| {
        // account_id 가 없으면 로그인 토큰(JWT) 안의 chatgpt_account_id 를 쓴다
        [field("id_token", "idToken"), Some(access_token.clone())]
            .into_iter()
            .flatten()
            .find_map(|token| account_id_from_jwt(&token))
    });
    Ok(Credentials {
        access_token,
        account_id,
    })
}

/// JWT 의 가운데 부분(payload)을 풀어서 chatgpt_account_id 를 찾는다. (서명 검증은 하지 않음)
fn account_id_from_jwt(token: &str) -> Option<String> {
    let payload = token.split('.').nth(1)?;
    let bytes = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .decode(payload.trim_end_matches('='))
        .ok()?;
    let claims: Value = serde_json::from_slice(&bytes).ok()?;
    claims
        .get("chatgpt_account_id")
        .or_else(|| claims.pointer("/https:~1~1api.openai.com~1auth/chatgpt_account_id"))
        .and_then(Value::as_str)
        .map(String::from)
}

/// 사용량 API 를 부른다.
pub async fn fetch_usage(credentials: &Credentials) -> ApiOutcome {
    let client = match reqwest::Client::builder()
        .timeout(Duration::from_secs(20))
        .user_agent(concat!("ai-usage/", env!("CARGO_PKG_VERSION")))
        .build()
    {
        Ok(client) => client,
        Err(error) => return ApiOutcome::Failed(error.to_string()),
    };
    let mut request = client
        .get(USAGE_URL)
        .bearer_auth(&credentials.access_token)
        .header("Accept", "application/json");
    if let Some(account_id) = &credentials.account_id {
        request = request.header("ChatGPT-Account-Id", account_id);
    }

    let response = match request.send().await {
        Ok(response) => response,
        Err(error) => return ApiOutcome::Failed(format!("네트워크 오류: {error}")),
    };
    let status = response.status();
    if status.as_u16() == 401 || status.as_u16() == 403 {
        return ApiOutcome::Unauthorized;
    }
    if !status.is_success() {
        return ApiOutcome::Failed(format!(
            "Codex 사용량 API 가 HTTP {} 를 돌려줬어요",
            status.as_u16()
        ));
    }
    match response.json::<Value>().await {
        Ok(json) => ApiOutcome::Ok(parse_usage_response(&json)),
        Err(error) => ApiOutcome::Failed(format!("응답을 해석하지 못했어요 ({error})")),
    }
}

/// `wham/usage` 응답을 해석한다.
/// `{"plan_type": "pro", "rate_limit": {"primary_window": {"used_percent": 42, "reset_at": 1789000000,
///   "limit_window_seconds": 18000}, "secondary_window": {...}}, "credits": {...}}`
pub fn parse_usage_response(json: &Value) -> CodexUsage {
    let window = |key: &str| {
        let value = json.pointer(&format!("/rate_limit/{key}"))?;
        Some((
            RawWindow {
                percent: number(value.get("used_percent")?)?,
                resets_at: value.get("reset_at").and_then(epoch_seconds),
            },
            value.get("limit_window_seconds").and_then(Value::as_i64),
        ))
    };
    let (session, weekly) = split_windows(window("primary_window"), window("secondary_window"));
    CodexUsage {
        reading: UsageReading {
            weekly,
            session,
            models: Vec::new(),
        },
        plan: json
            .get("plan_type")
            .and_then(Value::as_str)
            .map(plan_display),
    }
}

/// 세션 로그의 `rate_limits` 를 해석한다.
/// `{"primary": {"used_percent": 55.0, "window_minutes": 10080, "resets_at": 1795051365}, "secondary": null, "plan_type": "pro"}`
pub fn parse_log_rate_limits(rate_limits: &Value) -> Option<CodexUsage> {
    let window = |key: &str| {
        let value = rate_limits.get(key).filter(|value| value.is_object())?;
        Some((
            RawWindow {
                percent: number(value.get("used_percent")?)?,
                resets_at: value.get("resets_at").and_then(epoch_seconds),
            },
            value
                .get("window_minutes")
                .and_then(Value::as_i64)
                .map(|minutes| minutes * 60),
        ))
    };
    let (primary, secondary) = (window("primary"), window("secondary"));
    if primary.is_none() && secondary.is_none() {
        return None;
    }
    let (session, weekly) = split_windows(primary, secondary);
    Some(CodexUsage {
        reading: UsageReading {
            weekly,
            session,
            models: Vec::new(),
        },
        plan: rate_limits
            .get("plan_type")
            .and_then(Value::as_str)
            .map(plan_display),
    })
}

/// 창 두 개를 길이로 구분해 (세션, 주간) 으로 나눈다. 순서가 아니라 길이(5시간/7일)로 정한다.
fn split_windows(
    first: Option<(RawWindow, Option<i64>)>,
    second: Option<(RawWindow, Option<i64>)>,
) -> (Option<RawWindow>, Option<RawWindow>) {
    let mut session = None;
    let mut weekly = None;
    let mut unknown = Vec::new();
    for (window, seconds) in [first, second].into_iter().flatten() {
        match seconds {
            Some(SESSION_WINDOW_SECONDS) => session = Some(window),
            Some(WEEK_WINDOW_SECONDS) => weekly = Some(window),
            _ => unknown.push(window),
        }
    }
    // 길이를 모르는 창은 빈 자리부터 채운다 (세션 먼저)
    for window in unknown {
        if session.is_none() {
            session = Some(window);
        } else if weekly.is_none() {
            weekly = Some(window);
        }
    }
    (session, weekly)
}

/// 가장 최근 Codex 세션 로그에서 마지막 한도 정보를 찾는다. (토큰 만료 시 대신 쓴다)
/// 돌려주는 시각은 그 기록이 남은 시각이다.
pub fn latest_logged_usage(home: &Path) -> Option<(CodexUsage, DateTime<Utc>)> {
    let mut files = Vec::new();
    collect_jsonl_files(&home.join("sessions"), &mut files);
    files.sort_by_key(|(_, modified)| std::cmp::Reverse(*modified));

    for (path, _) in files.into_iter().take(5) {
        if let Some(found) = last_rate_limits_in(&path) {
            return Some(found);
        }
    }
    None
}

/// 모든 Codex 세션 로그에서 한도 기록을 시각과 함께 꺼낸다. (그래프용 지난 기록 가져오기)
/// 로그가 클 수 있으므로 별도 스레드에서 부를 것.
pub fn all_logged_usage(home: &Path) -> Vec<(DateTime<Utc>, CodexUsage)> {
    use std::io::BufRead;

    let mut files = Vec::new();
    collect_jsonl_files(&home.join("sessions"), &mut files);
    let mut found = Vec::new();
    for (path, _) in files {
        let Ok(file) = fs::File::open(&path) else {
            continue;
        };
        for line in std::io::BufReader::new(file).lines().map_while(Result::ok) {
            // 줄 전체를 해석하기 전에 글자로 먼저 걸러서 빠르게 훑는다
            if !line.contains("\"rate_limits\":{") {
                continue;
            }
            let Ok(event) = serde_json::from_str::<Value>(&line) else {
                continue;
            };
            let at = event
                .get("timestamp")
                .and_then(Value::as_str)
                .and_then(|text| DateTime::parse_from_rfc3339(text).ok())
                .map(|time| time.with_timezone(&Utc));
            let usage = event
                .pointer("/payload/rate_limits")
                .and_then(parse_log_rate_limits);
            if let (Some(at), Some(usage)) = (at, usage) {
                found.push((at, usage));
            }
        }
    }
    found.sort_by_key(|(at, _)| *at);
    found
}

fn collect_jsonl_files(dir: &Path, out: &mut Vec<(PathBuf, std::time::SystemTime)>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_jsonl_files(&path, out);
        } else if path.extension().is_some_and(|ext| ext == "jsonl")
            && let Ok(modified) = entry.metadata().and_then(|meta| meta.modified())
        {
            out.push((path, modified));
        }
    }
}

fn last_rate_limits_in(path: &Path) -> Option<(CodexUsage, DateTime<Utc>)> {
    let mut file = fs::File::open(path).ok()?;
    let length = file.metadata().ok()?.len();
    file.seek(SeekFrom::Start(length.saturating_sub(LOG_TAIL_BYTES)))
        .ok()?;
    let mut tail = String::new();
    // 잘린 첫 줄 때문에 UTF-8 이 깨질 수 있으므로 바이트로 읽고 손실 변환한다
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes).ok()?;
    tail.push_str(&String::from_utf8_lossy(&bytes));

    tail.lines()
        .rev()
        .filter(|line| line.contains("\"rate_limits\""))
        .find_map(|line| {
            let event: Value = serde_json::from_str(line).ok()?;
            let rate_limits = event.pointer("/payload/rate_limits")?;
            let usage = parse_log_rate_limits(rate_limits)?;
            let at = event
                .get("timestamp")
                .and_then(Value::as_str)
                .and_then(|text| DateTime::parse_from_rfc3339(text).ok())
                .map(|time| time.with_timezone(&Utc))?;
            Some((usage, at))
        })
}

/// "pro" → "Pro", "plus" → "Plus"
fn plan_display(plan: &str) -> String {
    let mut chars = plan.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
        None => String::new(),
    }
}

fn number(value: &Value) -> Option<f64> {
    match value {
        Value::Number(number) => number.as_f64(),
        Value::String(text) => text.trim().trim_end_matches('%').parse().ok(),
        _ => None,
    }
}

fn epoch_seconds(value: &Value) -> Option<DateTime<Utc>> {
    let seconds = value
        .as_i64()
        .or_else(|| value.as_f64().map(|v| v as i64))?;
    Utc.timestamp_opt(seconds, 0).single()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn usage_response_windows_are_split_by_length() {
        // 원본 앱 테스트에 있던 응답 형식
        let response = json!({
            "plan_type": "plus",
            "rate_limit": {
                "primary_window": {"used_percent": 42, "reset_at": 1789000000, "limit_window_seconds": 18000},
                "secondary_window": {"used_percent": 17, "reset_at": 1789500000, "limit_window_seconds": 604800}
            },
            "credits": {"has_credits": true, "unlimited": false, "balance": "12.50"}
        });
        let usage = parse_usage_response(&response);
        assert_eq!(usage.plan.as_deref(), Some("Plus"));
        assert_eq!(usage.reading.session.as_ref().unwrap().percent, 42.0);
        let weekly = usage.reading.weekly.unwrap();
        assert_eq!(weekly.percent, 17.0);
        assert_eq!(weekly.resets_at.unwrap().timestamp(), 1789500000);
    }

    #[test]
    fn weekly_only_plan_from_session_log() {
        // 이 Mac 의 실제 Codex 로그 형식 (Pro 플랜, 주간 창만 있음)
        let rate_limits = json!({
            "limit_id": "codex", "limit_name": null,
            "primary": {"used_percent": 55.0, "window_minutes": 10080, "resets_at": 1795051365},
            "secondary": null,
            "credits": {"has_credits": false, "unlimited": false, "balance": "0"},
            "plan_type": "pro"
        });
        let usage = parse_log_rate_limits(&rate_limits).unwrap();
        assert_eq!(usage.reading.session, None);
        assert_eq!(usage.reading.weekly.unwrap().percent, 55.0);
        assert_eq!(usage.plan.as_deref(), Some("Pro"));
        assert_eq!(
            parse_log_rate_limits(&json!({"primary": null, "secondary": null})),
            None
        );
    }

    #[test]
    fn credentials_snake_or_camel_case_and_jwt_fallback() {
        let snake = json!({"tokens": {"access_token": "at", "account_id": "acc-1"}});
        assert_eq!(
            parse_credentials(&snake).unwrap(),
            Credentials {
                access_token: "at".into(),
                account_id: Some("acc-1".into())
            }
        );

        // account_id 가 없으면 id_token(JWT) 의 chatgpt_account_id 를 쓴다
        let claims = json!({"https://api.openai.com/auth": {"chatgpt_account_id": "acc-jwt"}});
        let payload = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(claims.to_string());
        let camel =
            json!({"tokens": {"accessToken": "at2", "idToken": format!("header.{payload}.sig")}});
        assert_eq!(
            parse_credentials(&camel).unwrap().account_id.as_deref(),
            Some("acc-jwt")
        );

        assert!(
            parse_credentials(&json!({"OPENAI_API_KEY": "sk"})).is_err(),
            "API 키 로그인은 지원하지 않음"
        );
    }
}
