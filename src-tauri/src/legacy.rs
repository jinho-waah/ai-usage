//! 원본 Claude Usage 앱(hamed-elfayome/Claude-Usage-Tracker)이 남긴 사용률 기록을 읽는다.
//!
//! - 프로필 이름: `~/Library/Preferences/HamedElfayome.Claude-Usage.plist` 의 `profiles_v3` (JSON 데이터)
//! - 기록: `~/Library/Application Support/Claude Usage/history/usageHistory_<프로필ID>.json`
//!
//! 읽기만 하고 원본 앱의 파일은 바꾸지 않는다. 프로필 정보에서는 ID 와 이름만 꺼낸다.

use std::fs;
use std::path::PathBuf;

use chrono::{DateTime, TimeZone, Utc};
use serde::Serialize;
use serde_json::Value;

use crate::history::Sample;

/// 원본 앱은 시각을 2001-01-01 기준 초로 저장한다 (Apple 기준 시각). 유닉스 시각과의 차이.
const APPLE_EPOCH_OFFSET: f64 = 978_307_200.0;

/// 가져올 수 있는 원본 앱 프로필
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct LegacyProfile {
    pub id: String,
    pub name: String,
    pub weekly_count: usize,
    pub session_count: usize,
    pub first_at: Option<DateTime<Utc>>,
    pub last_at: Option<DateTime<Utc>>,
}

/// 원본 앱 기록 하나에서 꺼낸 주간·세션 사용률
#[derive(Debug, Clone, Default, PartialEq)]
pub struct LegacyHistory {
    pub weekly: Vec<Sample>,
    pub session: Vec<Sample>,
}

fn home() -> PathBuf {
    PathBuf::from(std::env::var("HOME").unwrap_or_default())
}

fn history_dir() -> PathBuf {
    home().join("Library/Application Support/Claude Usage/history")
}

fn preferences_path() -> PathBuf {
    home().join("Library/Preferences/HamedElfayome.Claude-Usage.plist")
}

/// 기록 파일이 있는 원본 앱 프로필 목록. 원본 앱을 안 썼으면 빈 목록.
pub fn list_profiles() -> Vec<LegacyProfile> {
    let names = profile_names();
    let Ok(entries) = fs::read_dir(history_dir()) else {
        return Vec::new();
    };
    let mut profiles: Vec<LegacyProfile> = entries
        .flatten()
        .filter_map(|entry| {
            let file_name = entry.file_name().to_string_lossy().to_string();
            let id = file_name
                .strip_prefix("usageHistory_")?
                .strip_suffix(".json")?
                .to_string();
            let history = read_history(&id)?;
            let times: Vec<DateTime<Utc>> = history
                .weekly
                .iter()
                .chain(&history.session)
                .map(|sample| sample.at)
                .collect();
            Some(LegacyProfile {
                name: names
                    .iter()
                    .find(|(profile_id, _)| profile_id.eq_ignore_ascii_case(&id))
                    .map(|(_, name)| name.clone())
                    .unwrap_or_else(|| id.chars().take(8).collect()),
                weekly_count: history.weekly.len(),
                session_count: history.session.len(),
                first_at: times.iter().min().copied(),
                last_at: times.iter().max().copied(),
                id,
            })
        })
        .collect();
    profiles.sort_by(|a, b| a.name.cmp(&b.name));
    profiles
}

/// 프로필 하나의 기록을 읽는다.
pub fn read_history(profile_id: &str) -> Option<LegacyHistory> {
    // 경로에 쓰이는 값이므로 UUID 모양(영문·숫자·하이픈)만 받는다
    if profile_id.is_empty()
        || !profile_id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-')
    {
        return None;
    }
    let text =
        fs::read_to_string(history_dir().join(format!("usageHistory_{profile_id}.json"))).ok()?;
    let json: Value = serde_json::from_str(&text).ok()?;
    Some(parse_history(&json))
}

/// `{"snapshots": [{"resetType": "weeklyReset", "timestamp": 816864387.0, "weeklyPercentage": 58}, ...]}`
pub fn parse_history(json: &Value) -> LegacyHistory {
    let mut history = LegacyHistory::default();
    let Some(snapshots) = json.get("snapshots").and_then(Value::as_array) else {
        return history;
    };
    for snapshot in snapshots {
        let Some(at) = snapshot
            .get("timestamp")
            .and_then(Value::as_f64)
            .and_then(apple_time)
        else {
            continue;
        };
        let percent = |key: &str| snapshot.get(key).and_then(Value::as_f64);
        match snapshot.get("resetType").and_then(Value::as_str) {
            Some("weeklyReset") => {
                if let Some(used_percent) = percent("weeklyPercentage") {
                    history.weekly.push(Sample { at, used_percent });
                }
            }
            Some("sessionReset") => {
                if let Some(used_percent) = percent("sessionPercentage") {
                    history.session.push(Sample { at, used_percent });
                }
            }
            _ => {} // billingCycle 등은 사용률이 아니라서 건너뛴다
        }
    }
    history
}

/// 원본 앱 설정에서 (프로필ID, 이름) 만 꺼낸다.
fn profile_names() -> Vec<(String, String)> {
    let Ok(value) = plist::Value::from_file(preferences_path()) else {
        return Vec::new();
    };
    let Some(data) = value
        .as_dictionary()
        .and_then(|dictionary| dictionary.get("profiles_v3"))
        .and_then(plist::Value::as_data)
    else {
        return Vec::new();
    };
    let Ok(profiles) = serde_json::from_slice::<Value>(data) else {
        return Vec::new();
    };
    profiles
        .as_array()
        .map(|items| {
            items
                .iter()
                .filter_map(|item| {
                    Some((
                        item.get("id")?.as_str()?.to_string(),
                        item.get("name")?.as_str()?.to_string(),
                    ))
                })
                .collect()
        })
        .unwrap_or_default()
}

fn apple_time(seconds: f64) -> Option<DateTime<Utc>> {
    Utc.timestamp_opt((seconds + APPLE_EPOCH_OFFSET) as i64, 0)
        .single()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn snapshots_split_into_weekly_and_session() {
        let history = parse_history(&json!({
            "snapshots": [
                { "resetType": "weeklyReset", "timestamp": 816864387.0, "weeklyPercentage": 58 },
                { "resetType": "sessionReset", "timestamp": 816355134.0, "sessionPercentage": 12 },
                { "resetType": "billingCycle", "timestamp": 806825463.0, "apiSpendCents": 14 }
            ]
        }));
        assert_eq!(history.weekly.len(), 1);
        assert_eq!(history.session.len(), 1);
        // 816864387 (2001 기준) = 2026-11-20 10:46:27 UTC
        assert_eq!(
            history.weekly[0].at.to_rfc3339(),
            "2026-11-20T10:46:27+00:00"
        );
        assert_eq!(history.weekly[0].used_percent, 58.0);
    }

    #[test]
    fn unsafe_profile_ids_are_rejected() {
        assert_eq!(read_history("../../etc/passwd"), None);
    }
}
