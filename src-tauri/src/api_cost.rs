//! Claude API(Console) 이번 달 비용. Admin API 키로 공식 Usage and Cost Analytics API 를 부른다.
//!
//! - 키는 macOS 키체인에만 저장한다 (파일에 쓰지 않는다).
//! - `GET /v1/organizations/analytics/cost` 에 이번 달 1일 0시(내 시간대)부터 지금까지를 주면
//!   합계 한 줄이 온다. 금액은 센트 단위 문자열이다 ("41280.000000" = $412.80).
//! - 데이터는 보통 4시간(최대 24시간) 늦게 반영된다. 응답의 `data_refreshed_at` 이 그 기준 시각이다.
//! - 문서: https://platform.claude.com/docs/en/manage-claude/usage-cost-analytics-api

use std::time::Duration;

use chrono::{DateTime, Datelike, Local, SecondsFormat, TimeZone, Utc};
use serde::Serialize;
use serde_json::Value;

const COST_URL: &str = "https://api.anthropic.com/v1/organizations/analytics/cost";
const API_VERSION: &str = "2026-10-21";
const ANALYTICS_BETA: &str = "analytics-usage-cost-2026-08-24";
/// 혹시 결과가 여러 쪽으로 나뉘어 와도 이만큼까지만 따라간다
const MAX_PAGES: usize = 10;

#[cfg(target_os = "macos")]
const KEYCHAIN_SERVICE: &str = "io.github.jinho-waah.ai-usage";
#[cfg(target_os = "macos")]
const KEYCHAIN_ACCOUNT: &str = "anthropic-admin-api-key";

/// 이번 달 비용
#[derive(Debug, Clone, Serialize, serde::Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct MonthCost {
    pub usd: f64,
    /// 이 시각까지의 사용분만 반영돼 있다 (`None`: 아직 집계된 데이터 없음)
    pub refreshed_at: Option<DateTime<Utc>>,
}

pub enum CostOutcome {
    Ok(MonthCost),
    /// 키가 틀렸거나 권한이 없음
    Unauthorized,
    Failed(String),
}

/// 키가 Admin API 키 모양인지
pub fn looks_like_admin_key(key: &str) -> bool {
    key.starts_with("sk-ant-admin") && key.len() > 20 && !key.contains(char::is_whitespace)
}

/// 화면에 보여 줄 가린 키 ("sk-ant-admin…abcd")
pub fn mask(key: &str) -> String {
    let tail: String = key
        .chars()
        .rev()
        .take(4)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect();
    format!("sk-ant-admin…{tail}")
}

#[cfg(target_os = "macos")]
pub fn load_key() -> Option<String> {
    let bytes =
        security_framework::passwords::get_generic_password(KEYCHAIN_SERVICE, KEYCHAIN_ACCOUNT)
            .ok()?;
    String::from_utf8(bytes).ok().filter(|key| !key.is_empty())
}

#[cfg(target_os = "macos")]
pub fn save_key(key: &str) -> Result<(), String> {
    security_framework::passwords::set_generic_password(
        KEYCHAIN_SERVICE,
        KEYCHAIN_ACCOUNT,
        key.as_bytes(),
    )
    .map_err(|error| format!("키체인에 저장하지 못했어요: {error}"))
}

#[cfg(target_os = "macos")]
pub fn delete_key() -> Result<(), String> {
    match security_framework::passwords::delete_generic_password(KEYCHAIN_SERVICE, KEYCHAIN_ACCOUNT)
    {
        Ok(()) => Ok(()),
        // 원래 없던 키는 지운 것과 같다 (errSecItemNotFound)
        Err(error) if error.code() == -25300 => Ok(()),
        Err(error) => Err(format!("키체인에서 지우지 못했어요: {error}")),
    }
}

#[cfg(not(target_os = "macos"))]
pub fn load_key() -> Option<String> {
    None
}

#[cfg(not(target_os = "macos"))]
pub fn save_key(_key: &str) -> Result<(), String> {
    Err("이 운영체제에서는 키를 저장할 수 없어요".to_string())
}

#[cfg(not(target_os = "macos"))]
pub fn delete_key() -> Result<(), String> {
    Ok(())
}

/// 이번 달(내 시간대 기준) 1일 0시. API 가 정시 단위로 내림하는데, 시간대 차이는 대부분 정시라 문제없다.
pub fn month_start(now: DateTime<Local>) -> DateTime<Utc> {
    Local
        .with_ymd_and_hms(now.year(), now.month(), 1, 0, 0, 0)
        .earliest()
        .map(|start| start.with_timezone(&Utc))
        .unwrap_or_else(|| now.with_timezone(&Utc))
}

/// 이번 달 비용을 받아 온다.
pub async fn fetch_month_cost(key: &str, now: DateTime<Local>) -> CostOutcome {
    let starting_at = month_start(now);
    let ending_at = now.with_timezone(&Utc);
    // 새 달이 시작되고 1시간이 안 지났으면 아직 집계할 범위가 없다
    if ending_at - starting_at < chrono::Duration::hours(1) {
        return CostOutcome::Ok(MonthCost {
            usd: 0.0,
            refreshed_at: None,
        });
    }

    let client = match reqwest::Client::builder()
        .timeout(Duration::from_secs(30))
        .user_agent(concat!("ai-usage/", env!("CARGO_PKG_VERSION")))
        .build()
    {
        Ok(client) => client,
        Err(error) => return CostOutcome::Failed(error.to_string()),
    };

    let mut pages = Vec::new();
    let mut next_page: Option<String> = None;
    for _ in 0..MAX_PAGES {
        let mut query = vec![
            (
                "starting_at",
                starting_at.to_rfc3339_opts(SecondsFormat::Secs, true),
            ),
            (
                "ending_at",
                ending_at.to_rfc3339_opts(SecondsFormat::Secs, true),
            ),
        ];
        if let Some(page) = &next_page {
            query.push(("page", page.clone()));
        }
        let response = match client
            .get(COST_URL)
            .query(&query)
            .header("x-api-key", key)
            .header("anthropic-version", API_VERSION)
            .header("anthropic-beta", ANALYTICS_BETA)
            .send()
            .await
        {
            Ok(response) => response,
            Err(error) => return CostOutcome::Failed(format!("네트워크 오류: {error}")),
        };
        let status = response.status().as_u16();
        if status == 401 || status == 403 {
            return CostOutcome::Unauthorized;
        }
        let json: Value = match response.json().await {
            Ok(json) => json,
            Err(error) => return CostOutcome::Failed(format!("응답을 읽지 못했어요: {error}")),
        };
        if status != 200 {
            let message = json
                .pointer("/error/message")
                .and_then(Value::as_str)
                .unwrap_or("알 수 없는 오류");
            return CostOutcome::Failed(format!("Admin API 오류 {status}: {message}"));
        }
        let has_more = json.get("has_more").and_then(Value::as_bool) == Some(true);
        next_page = json
            .get("next_page")
            .and_then(Value::as_str)
            .map(String::from);
        pages.push(json);
        if !has_more || next_page.is_none() {
            break;
        }
    }
    CostOutcome::Ok(parse_cost_pages(&pages))
}

/// 응답 쪽들의 `cost.amount`(센트) 를 모두 더해 달러로 바꾼다.
pub fn parse_cost_pages(pages: &[Value]) -> MonthCost {
    let cents: f64 = pages
        .iter()
        .flat_map(|page| {
            page.get("data")
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default()
        })
        .filter_map(|row| {
            let amount = row.pointer("/cost/amount")?;
            amount
                .as_str()
                .and_then(|text| text.parse::<f64>().ok())
                .or_else(|| amount.as_f64())
        })
        .sum();
    let refreshed_at = pages
        .first()
        .and_then(|page| page.get("data_refreshed_at"))
        .and_then(Value::as_str)
        .and_then(|text| DateTime::parse_from_rfc3339(text).ok())
        .map(|time| time.with_timezone(&Utc));
    MonthCost {
        usd: cents / 100.0,
        refreshed_at,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn amounts_are_cents_strings() {
        let cost = parse_cost_pages(&[json!({
            "data": [
                { "starting_at": "2026-11-01T00:00:00Z", "ending_at": "2026-11-20T00:00:00Z",
                  "group": {}, "cost": { "currency": "USD", "amount": "41280.000000", "list_amount": "51600.000000" } }
            ],
            "has_more": false,
            "next_page": null,
            "data_refreshed_at": "2026-11-20T08:02:11Z"
        })]);
        assert!((cost.usd - 412.80).abs() < 1e-9);
        assert_eq!(
            cost.refreshed_at.unwrap().to_rfc3339(),
            "2026-11-20T08:02:11+00:00"
        );
    }

    #[test]
    fn empty_month_is_zero() {
        let cost = parse_cost_pages(&[
            json!({ "data": [], "has_more": false, "data_refreshed_at": null }),
        ]);
        assert_eq!(cost.usd, 0.0);
        assert_eq!(cost.refreshed_at, None);
    }

    #[test]
    fn keys_are_checked_and_masked() {
        assert!(looks_like_admin_key("sk-ant-admin01-abcdefghijklmnop-1234"));
        assert!(!looks_like_admin_key("sk-ant-api03-abcdefghijklmnop"));
        assert!(!looks_like_admin_key("sk-ant-admin01 with space"));
        assert_eq!(
            mask("sk-ant-admin01-abcdefghijklmnop-1234"),
            "sk-ant-admin…1234"
        );
    }
}
