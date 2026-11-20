//! claude.ai 응답을 해석한다.
//!
//! 네트워크·화면과 상관없는 순수 함수만 모아 두어서, 실제 응답 예시로 테스트하기 쉽다.
//! claude.ai 내부 API 는 공개 API 가 아니라서 언제든 바뀔 수 있다.
//! 응답 형식이 바뀌면 이 파일만 고치면 되게 해석 코드를 여기에 모았다.

use chrono::{DateTime, Utc};
use serde::Deserialize;
use serde_json::Value;

use crate::model::{ExtraUsage, ModelUsage, RawWindow, UsageReading};

pub const BASE_URL: &str = "https://claude.ai";
pub const WEEK_SECONDS: i64 = 7 * 24 * 60 * 60;
pub const SESSION_SECONDS: i64 = 5 * 60 * 60;

/// 사용량 응답을 해석한다.
///
/// 응답 예시 (2026-11 기준):
/// `{"five_hour": null, "seven_day": {"utilization": 47.0, "resets_at": "..."},
///   "limits": [{"kind": "weekly_scoped", "group": "weekly", "percent": 2,
///               "scope": {"model": {"display_name": "Fable"}}, ...}]}`
pub fn parse_usage(json: &Value) -> Result<UsageReading, String> {
    let object = json
        .as_object()
        .ok_or("사용량 응답이 JSON 객체가 아니에요")?;
    let limits: &[Value] = object
        .get("limits")
        .and_then(Value::as_array)
        .map_or(&[], Vec::as_slice);

    // 예전 형식 필드 (five_hour, seven_day, seven_day_opus ...)
    let legacy = |key: &str| object.get(key).and_then(window_from_legacy);
    // 새 형식 limits[] 에서 조건에 맞는 첫 항목
    let from_limits = |matches: fn(&Value) -> bool| {
        limits
            .iter()
            .find(|limit| matches(limit))
            .and_then(window_from_limit)
    };

    let session = legacy("five_hour").or_else(|| {
        // group 이 weekly 가 아닌 항목(예: "session")이 있으면 5시간 창이 있는 플랜이다
        from_limits(|limit| text(limit, "group").is_some_and(|group| group != "weekly"))
    });
    let weekly = legacy("seven_day")
        .or_else(|| from_limits(|limit| text(limit, "kind") == Some("weekly_all")));

    let mut models: Vec<ModelUsage> = limits
        .iter()
        .filter(|limit| text(limit, "kind") == Some("weekly_scoped"))
        .filter_map(|limit| {
            let name = limit
                .pointer("/scope/model/display_name")
                .and_then(Value::as_str)
                .filter(|name| !name.is_empty())?;
            Some(ModelUsage {
                name: name.to_string(),
                used_percent: number(limit.get("percent")?)?,
            })
        })
        .collect();

    // limits[] 가 없는 예전 응답도 받아 준다
    if models.is_empty() {
        let legacy_models = [
            ("seven_day_opus", "Opus"),
            ("seven_day_sonnet", "Sonnet"),
            ("seven_day_fable", "Fable"),
            ("seven_day_omelette", "Design"),
        ];
        for (key, name) in legacy_models {
            if let Some(window) = legacy(key) {
                models.push(ModelUsage {
                    name: name.to_string(),
                    used_percent: window.percent,
                });
            }
        }
    }

    Ok(UsageReading {
        weekly,
        session,
        models,
    })
}

/// `/api/organizations/{org}/overage_spend_limit` (Extra usage) 를 해석한다.
/// 금액은 센트 단위로 오므로 100 으로 나눈다. 꺼져 있으면 `None`.
pub fn parse_overage(json: &Value) -> Option<ExtraUsage> {
    if json.get("is_enabled").and_then(Value::as_bool) != Some(true) {
        return None;
    }
    let used = json.get("used_credits").and_then(number).unwrap_or(0.0) / 100.0;
    let limit = json
        .get("monthly_credit_limit")
        .and_then(number)
        .map(|cents| cents / 100.0);
    let currency = json
        .get("currency")
        .and_then(Value::as_str)
        .unwrap_or("USD")
        .to_uppercase();
    Some(ExtraUsage {
        used,
        limit,
        currency,
    })
}

/// claude.ai 조직 (개인 계정도 조직 하나를 가진다)
#[derive(Debug, Clone, PartialEq)]
pub struct Organization {
    pub uuid: String,
    pub name: String,
    pub capabilities: Vec<String>,
    pub rate_limit_tier: Option<String>,
}

/// `/api/organizations` 응답(배열)을 해석한다.
pub fn parse_organizations(json: &Value) -> Vec<Organization> {
    let Some(items) = json.as_array() else {
        return Vec::new();
    };
    items
        .iter()
        .filter_map(|item| {
            Some(Organization {
                uuid: item.get("uuid")?.as_str()?.to_string(),
                name: text(item, "name").unwrap_or_default().to_string(),
                capabilities: item
                    .get("capabilities")
                    .and_then(Value::as_array)
                    .map(|list| {
                        list.iter()
                            .filter_map(Value::as_str)
                            .map(String::from)
                            .collect()
                    })
                    .unwrap_or_default(),
                rate_limit_tier: text(item, "rate_limit_tier").map(String::from),
            })
        })
        .collect()
}

/// 사용할 조직을 고른다.
/// 이미 고른 조직이 아직 있으면 그대로 쓰고, 없으면 채팅(chat) 기능이 있는 조직,
/// 그것도 없으면 첫 번째 조직. (API 전용 조직에는 사용량 정보가 없다)
pub fn preferred_organization<'a>(
    organizations: &'a [Organization],
    current: Option<&str>,
) -> Option<&'a Organization> {
    current
        .and_then(|current| organizations.iter().find(|org| org.uuid == current))
        .or_else(|| {
            organizations.iter().find(|org| {
                org.capabilities
                    .iter()
                    .any(|capability| capability == "chat")
            })
        })
        .or_else(|| organizations.first())
}

/// 조직 정보로 플랜 이름을 추정한다. 모르면 `None`.
///
/// 2026-10-30 에 Anthropic 이 플랜 이름을 바꿨다: Max 5x → "Max", Max 20x → "Ultra".
/// 하지만 API 가 주는 내부 등급 코드는 예전 그대로(`default_claude_max_20x` 등)라서 여기서 새 이름으로 바꿔 준다.
pub fn plan_name(organization: &Organization) -> Option<String> {
    let tier = organization
        .rate_limit_tier
        .as_deref()
        .unwrap_or_default()
        .to_lowercase();
    let has = |capability: &str| organization.capabilities.iter().any(|c| c == capability);
    let plan = if tier.contains("max_20x") || tier.contains("ultra") {
        "Ultra"
    } else if tier.contains("max_5x") || has("claude_max") || tier.contains("claude_max") {
        "Max"
    } else if has("claude_pro") || tier.contains("claude_pro") {
        "Pro"
    } else if has("enterprise") || tier.contains("enterprise") {
        "Enterprise"
    } else if has("raven") || tier.contains("raven") || tier.contains("team") {
        "Team"
    } else {
        return None;
    };
    Some(plan.to_string())
}

/// 개인 조직 이름("someone@example.com's Organization")에서 기본 계정 이름을 만든다.
pub fn default_account_name(organization_name: &str) -> Option<String> {
    let owner = organization_name
        .strip_suffix("'s Organization")
        .unwrap_or(organization_name)
        .trim();
    let base = owner.split('@').next().unwrap_or(owner).trim();
    (!base.is_empty()).then(|| base.chars().take(20).collect())
}

/// 계정 웹뷰 안에서 `fetch` 한 결과 (자바스크립트가 JSON 으로 돌려준다)
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FetchResult {
    pub status: u16,
    pub content_type: Option<String>,
    pub body: String,
    pub error: Option<String>,
}

/// fetch 결과를 어떻게 처리할지
#[derive(Debug, Clone, PartialEq)]
pub enum FetchOutcome {
    Json(Value),
    /// 로그인이 필요함 (401/403)
    NeedsLogin,
    /// Cloudflare 봇 확인 페이지가 돌아옴
    Challenge,
    Failed(String),
}

pub fn classify(result: &FetchResult) -> FetchOutcome {
    if let Some(error) = &result.error {
        return FetchOutcome::Failed(format!("네트워크 오류: {error}"));
    }
    let is_json = result
        .content_type
        .as_deref()
        .is_some_and(|value| value.contains("json"));
    let is_challenge = !is_json
        && (result.body.contains("challenge-platform")
            || result.body.contains("cf-chl")
            || result.body.contains("Just a moment"));

    match result.status {
        200..=299 if is_json => serde_json::from_str(&result.body)
            .map(FetchOutcome::Json)
            .unwrap_or_else(|error| {
                FetchOutcome::Failed(format!("응답을 해석하지 못했어요 ({error})"))
            }),
        _ if is_challenge => FetchOutcome::Challenge,
        401 | 403 => FetchOutcome::NeedsLogin,
        200..=299 => FetchOutcome::Failed("JSON 이 아닌 응답이 왔어요".into()),
        status => FetchOutcome::Failed(format!("claude.ai 가 HTTP {status} 를 돌려줬어요")),
    }
}

/// 웹뷰 안에서 실행할 자바스크립트: `url` 을 가져와서 상태 코드와 본문을 JSON 문자열로 돌려준다.
/// (`url` 은 Rust 가 인자로 넘겨준다. 코드 문자열에 직접 끼워 넣지 않으므로 안전하다.)
pub const FETCH_SCRIPT: &str = r#"
try {
  const response = await fetch(url, { credentials: "include", headers: { accept: "application/json" } });
  const body = await response.text();
  return JSON.stringify({ status: response.status, contentType: response.headers.get("content-type"), body: body.slice(0, 500000), error: null });
} catch (error) {
  return JSON.stringify({ status: 0, contentType: null, body: "", error: String(error) });
}
"#;

/// 웹뷰가 지금 어느 페이지를 얼마나 열었는지 확인하는 자바스크립트 ("claude.ai|/robots.txt|complete")
pub const READY_SCRIPT: &str =
    r#"return location.host + "|" + location.pathname + "|" + document.readyState;"#;

fn text<'a>(value: &'a Value, key: &str) -> Option<&'a str> {
    value.get(key).and_then(Value::as_str)
}

/// 숫자 또는 "47%" 같은 문자열을 숫자로 바꾼다.
fn number(value: &Value) -> Option<f64> {
    match value {
        Value::Number(number) => number.as_f64(),
        Value::String(text) => text.trim().trim_end_matches('%').trim().parse().ok(),
        _ => None,
    }
}

fn time(value: &Value) -> Option<DateTime<Utc>> {
    let text = value.as_str()?;
    DateTime::parse_from_rfc3339(text)
        .ok()
        .map(|time| time.with_timezone(&Utc))
}

fn window_from_legacy(value: &Value) -> Option<RawWindow> {
    Some(RawWindow {
        percent: number(value.get("utilization")?)?,
        resets_at: value.get("resets_at").and_then(time),
    })
}

fn window_from_limit(value: &Value) -> Option<RawWindow> {
    Some(RawWindow {
        percent: number(value.get("percent")?)?,
        resets_at: value.get("resets_at").and_then(time),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn weekly_only_plan_with_scoped_model_limits() {
        // 원본 앱 테스트에 있던 실제 응답 형식 (5시간 한도가 없는 플랜)
        let response = json!({
            "five_hour": null,
            "seven_day": { "utilization": 47.0, "resets_at": "2026-11-03T17:00:00.036205+00:00" },
            "seven_day_opus": null, "seven_day_sonnet": null, "seven_day_omelette": null,
            "limits": [
                { "kind": "weekly_all", "group": "weekly", "percent": 47, "severity": "normal",
                  "resets_at": "2026-11-03T17:00:00.036205+00:00", "scope": null, "is_active": true },
                { "kind": "weekly_scoped", "group": "weekly", "percent": 2, "severity": "normal",
                  "resets_at": "2026-11-03T17:00:00.036349+00:00",
                  "scope": { "model": { "id": null, "display_name": "Fable" }, "surface": null },
                  "is_active": false }
            ]
        });
        let usage = parse_usage(&response).unwrap();
        assert_eq!(usage.session, None);
        let weekly = usage.weekly.unwrap();
        assert_eq!(weekly.percent, 47.0);
        assert_eq!(
            weekly.resets_at.unwrap().to_rfc3339(),
            "2026-11-03T17:00:00.036205+00:00"
        );
        assert_eq!(
            usage.models,
            vec![ModelUsage {
                name: "Fable".into(),
                used_percent: 2.0
            }]
        );
    }

    #[test]
    fn session_plan_and_string_percentages() {
        let response = json!({
            "five_hour": { "utilization": "32%", "resets_at": "2026-11-02T15:00:00.000000+00:00" },
            "seven_day": { "utilization": 12.5, "resets_at": null },
            "limits": []
        });
        let usage = parse_usage(&response).unwrap();
        assert_eq!(usage.session.unwrap().percent, 32.0);
        let weekly = usage.weekly.unwrap();
        assert_eq!(weekly.percent, 12.5);
        assert_eq!(weekly.resets_at, None, "리셋 시각이 null 이면 None");
    }

    #[test]
    fn session_found_only_in_limits() {
        let response = json!({
            "five_hour": null, "seven_day": null,
            "limits": [
                { "kind": "session", "group": "session", "percent": 40, "resets_at": "2026-11-02T15:00:00Z" },
                { "kind": "weekly_all", "group": "weekly", "percent": 20, "resets_at": "2026-11-03T17:00:00Z" }
            ]
        });
        let usage = parse_usage(&response).unwrap();
        assert_eq!(usage.session.unwrap().percent, 40.0);
        assert_eq!(usage.weekly.unwrap().percent, 20.0);
    }

    #[test]
    fn legacy_model_fields_are_used_when_limits_missing() {
        let response = json!({
            "seven_day": { "utilization": 10, "resets_at": "2026-11-03T17:00:00Z" },
            "seven_day_opus": { "utilization": 7, "resets_at": null }
        });
        let usage = parse_usage(&response).unwrap();
        assert_eq!(
            usage.models,
            vec![ModelUsage {
                name: "Opus".into(),
                used_percent: 7.0
            }]
        );
    }

    #[test]
    fn non_object_usage_is_an_error() {
        assert!(parse_usage(&json!([1, 2])).is_err());
    }

    #[test]
    fn overage_is_in_cents_and_hidden_when_disabled() {
        let enabled = json!({ "is_enabled": true, "used_credits": 420, "monthly_credit_limit": 5000, "currency": "usd" });
        assert_eq!(
            parse_overage(&enabled),
            Some(ExtraUsage {
                used: 4.2,
                limit: Some(50.0),
                currency: "USD".into()
            })
        );
        assert_eq!(parse_overage(&json!({ "is_enabled": false })), None);
    }

    #[test]
    fn organization_choice_and_plan() {
        let response = json!([
            { "uuid": "api-only", "name": "API", "capabilities": ["api"] },
            { "uuid": "personal", "name": "someone@example.com's Organization",
              "capabilities": ["chat", "claude_max"], "rate_limit_tier": "default_claude_max_20x" }
        ]);
        let organizations = parse_organizations(&response);
        let chosen = preferred_organization(&organizations, None).unwrap();
        assert_eq!(chosen.uuid, "personal", "채팅 기능이 있는 조직을 고름");
        assert_eq!(
            plan_name(chosen).as_deref(),
            Some("Ultra"),
            "Max 20x 는 Ultra 로 이름이 바뀜"
        );
        let five_x = Organization {
            rate_limit_tier: Some("default_claude_max_5x".into()),
            ..chosen.clone()
        };
        assert_eq!(
            plan_name(&five_x).as_deref(),
            Some("Max"),
            "Max 5x 는 Max 로 이름이 바뀜"
        );
        assert_eq!(
            default_account_name(&chosen.name).as_deref(),
            Some("someone")
        );
        // 이미 고른 조직이 있으면 유지
        let kept = preferred_organization(&organizations, Some("api-only")).unwrap();
        assert_eq!(kept.uuid, "api-only");
    }

    #[test]
    fn fetch_results_are_classified() {
        let ok = FetchResult {
            status: 200,
            content_type: Some("application/json".into()),
            body: "{}".into(),
            error: None,
        };
        assert_eq!(classify(&ok), FetchOutcome::Json(json!({})));
        let denied = FetchResult {
            status: 403,
            content_type: Some("application/json".into()),
            body: "{}".into(),
            error: None,
        };
        assert_eq!(classify(&denied), FetchOutcome::NeedsLogin);
        let challenge = FetchResult {
            status: 403,
            content_type: Some("text/html".into()),
            body: "<title>Just a moment...</title>".into(),
            error: None,
        };
        assert_eq!(classify(&challenge), FetchOutcome::Challenge);
        let offline = FetchResult {
            status: 0,
            content_type: None,
            body: String::new(),
            error: Some("TypeError".into()),
        };
        assert!(matches!(classify(&offline), FetchOutcome::Failed(_)));
    }
}
