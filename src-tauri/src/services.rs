//! Claude·OpenAI 서비스 상태 (status.claude.com, status.openai.com).
//!
//! 두 곳 모두 Atlassian Statuspage 형식의 `/api/v2/summary.json` 을 준다.
//! 전체 상태 대신 이 앱 사용자와 관련 있는 항목(구성 요소)만 본다.
//! 예: "Claude for Government" 에 문제가 생겨도 claude.ai·Claude Code 가 멀쩡하면 정상으로 본다.
//! 항목 이름이 바뀌어서 하나도 못 찾으면 페이지 전체 상태를 쓴다.

use std::time::Duration;

use serde_json::Value;

use crate::model::{Provider, ServiceState};

/// 상태 페이지 하나
pub struct ServiceSource {
    /// 화면과 알림에 쓰는 이름
    pub name: &'static str,
    /// 이 제공자의 계정이 있을 때만 확인한다
    pub provider: Provider,
    pub url: &'static str,
    /// 관련 있는 구성 요소 이름에 들어 있는 글자 (소문자)
    pub components: &'static [&'static str],
}

pub const SOURCES: [ServiceSource; 2] = [
    ServiceSource {
        name: "Claude",
        provider: Provider::Claude,
        url: "https://status.claude.com/api/v2/summary.json",
        components: &["claude.ai", "claude code", "claude api", "claude console"],
    },
    ServiceSource {
        name: "OpenAI",
        provider: Provider::Codex,
        url: "https://status.openai.com/api/v2/summary.json",
        // Codex CLI 는 Responses API 와 ChatGPT 로그인을 쓴다
        components: &["codex", "responses", "login"],
    },
];

/// 상태 페이지를 읽은 결과
#[derive(Debug, Clone, PartialEq)]
pub struct ServiceReport {
    pub state: ServiceState,
    /// 알림에 쓸 설명: 진행 중인 장애 이름, 없으면 문제가 있는 항목 이름
    pub detail: Option<String>,
}

impl ServiceReport {
    fn unknown() -> ServiceReport {
        ServiceReport {
            state: ServiceState::Unknown,
            detail: None,
        }
    }
}

/// 상태 페이지를 받아 와서 해석한다. 실패하면 "알 수 없음".
pub async fn fetch(source: &ServiceSource) -> ServiceReport {
    let Ok(client) = reqwest::Client::builder()
        .timeout(Duration::from_secs(15))
        .user_agent(concat!("ai-usage/", env!("CARGO_PKG_VERSION")))
        .build()
    else {
        return ServiceReport::unknown();
    };
    let Ok(response) = client.get(source.url).send().await else {
        return ServiceReport::unknown();
    };
    if !response.status().is_success() {
        return ServiceReport::unknown();
    }
    match response.json::<Value>().await {
        Ok(json) => parse_summary(&json, source.components),
        Err(_) => ServiceReport::unknown(),
    }
}

/// `summary.json` 을 해석한다.
pub fn parse_summary(json: &Value, relevant: &[&str]) -> ServiceReport {
    // 1. 관련 있는 구성 요소 중 가장 나쁜 상태
    let mut found_any = false;
    let mut worst = ServiceState::Operational;
    let mut troubled: Vec<String> = Vec::new();
    for component in json
        .get("components")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        let name = component
            .get("name")
            .and_then(Value::as_str)
            .unwrap_or_default();
        let lowered = name.to_lowercase();
        if !relevant.iter().any(|pattern| lowered.contains(pattern)) {
            continue;
        }
        found_any = true;
        let state = component_state(component.get("status").and_then(Value::as_str));
        if state != ServiceState::Operational && state != ServiceState::Unknown {
            if !troubled.iter().any(|existing| existing == name) {
                troubled.push(name.to_string());
            }
            worst = worse(worst, state);
        }
    }

    // 2. 관련 항목을 하나도 못 찾았으면 페이지 전체 상태
    let state = if found_any {
        worst
    } else {
        match json.pointer("/status/indicator").and_then(Value::as_str) {
            Some("none") => ServiceState::Operational,
            Some("minor") | Some("maintenance") => ServiceState::Degraded,
            Some("major") | Some("critical") => ServiceState::Outage,
            _ => ServiceState::Unknown,
        }
    };

    // 3. 설명: 진행 중인 장애 이름(Claude 만 제공) → 문제 있는 항목 → 페이지 설명
    let incidents: Vec<String> = json
        .get("incidents")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter(|incident| incident.get("status").and_then(Value::as_str) != Some("resolved"))
        .filter_map(|incident| incident.get("name").and_then(Value::as_str))
        .map(String::from)
        .collect();
    let detail = if state == ServiceState::Operational || state == ServiceState::Unknown {
        None
    } else if !incidents.is_empty() {
        Some(incidents.join(", "))
    } else if !troubled.is_empty() {
        Some(troubled.join(", "))
    } else {
        json.pointer("/status/description")
            .and_then(Value::as_str)
            .map(String::from)
    };

    ServiceReport { state, detail }
}

fn component_state(status: Option<&str>) -> ServiceState {
    match status {
        Some("operational") => ServiceState::Operational,
        Some("degraded_performance") | Some("under_maintenance") => ServiceState::Degraded,
        Some("partial_outage") | Some("major_outage") => ServiceState::Outage,
        _ => ServiceState::Unknown,
    }
}

/// 둘 중 더 나쁜 상태
fn worse(a: ServiceState, b: ServiceState) -> ServiceState {
    let rank = |state: ServiceState| match state {
        ServiceState::Outage => 3,
        ServiceState::Degraded => 2,
        ServiceState::Operational => 1,
        ServiceState::Unknown => 0,
    };
    if rank(b) > rank(a) { b } else { a }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    const CLAUDE: &[&str] = &["claude.ai", "claude code", "claude api", "claude console"];

    #[test]
    fn unrelated_component_problems_are_ignored() {
        let report = parse_summary(
            &json!({
                "status": { "indicator": "minor", "description": "Minor Service Outage" },
                "components": [
                    { "name": "claude.ai", "status": "operational" },
                    { "name": "Claude Code", "status": "operational" },
                    { "name": "Claude for Government", "status": "partial_outage" }
                ],
                "incidents": []
            }),
            CLAUDE,
        );
        assert_eq!(report.state, ServiceState::Operational);
        assert_eq!(report.detail, None);
    }

    #[test]
    fn worst_relevant_component_wins_and_incident_name_is_the_detail() {
        let report = parse_summary(
            &json!({
                "status": { "indicator": "major", "description": "Partial System Outage" },
                "components": [
                    { "name": "claude.ai", "status": "degraded_performance" },
                    { "name": "Claude API (api.anthropic.com)", "status": "major_outage" }
                ],
                "incidents": [
                    { "name": "Elevated errors on Claude Opus", "status": "investigating" },
                    { "name": "Old issue", "status": "resolved" }
                ]
            }),
            CLAUDE,
        );
        assert_eq!(report.state, ServiceState::Outage);
        assert_eq!(
            report.detail.as_deref(),
            Some("Elevated errors on Claude Opus")
        );
    }

    #[test]
    fn falls_back_to_page_status_when_components_are_unknown() {
        // OpenAI 페이지에는 incidents 가 없다 → 문제 있는 항목 이름이 설명이 된다
        let openai = parse_summary(
            &json!({
                "status": { "indicator": "minor", "description": "Degraded" },
                "components": [ { "name": "Codex in ChatGPT Desktop", "status": "degraded_performance" } ]
            }),
            &["codex", "responses", "login"],
        );
        assert_eq!(openai.state, ServiceState::Degraded);
        assert_eq!(openai.detail.as_deref(), Some("Codex in ChatGPT Desktop"));

        let renamed = parse_summary(
            &json!({ "status": { "indicator": "critical", "description": "Major Outage" }, "components": [] }),
            CLAUDE,
        );
        assert_eq!(renamed.state, ServiceState::Outage);
        assert_eq!(renamed.detail.as_deref(), Some("Major Outage"));
    }
}
