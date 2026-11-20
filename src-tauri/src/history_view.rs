//! 히스토리 창에 보낼 데이터를 만든다: 주간·5시간 사용률 추이, 주간 주기(사이클)별 소진 곡선.
//!
//! 주기 나누기 (`split_cycles`)
//! - 리셋 지점(기준점)을 모은다: 기록에 남은 리셋 시각(정확), 리셋 시각이 없는 예전 기록의 큰 폭 하락(대략).
//! - 주간 한도는 매주 같은 시각에 리셋되므로, 하락 구간 안에 정확한 리셋 시각(없으면 바로 앞 기준점)의
//!   7일 간격 시각이 있으면 그 시각으로 맞춘다. 앱이 꺼져 있던 사이의 리셋은 다시 켠 뒤에야 하락으로 보이기 때문.
//! - 기준점 사이가 7일보다 길면(기록이 끊긴 기간) 7일 간격으로 경계를 채운다.
//! - 리셋 직후 사용률은 0% 이므로, 첫 주기가 아니면 시작 시각에 0% 점을 넣는다.

use chrono::{DateTime, Duration, Utc};
use serde::Serialize;

use crate::accounts::Account;
use crate::claude::WEEK_SECONDS;
use crate::history::{History, Sample, WindowKind};
use crate::model::{AccountLabel, Provider, TrendPoint};

/// 리셋 시각을 모르는 예전 기록에서 이만큼 넘게 떨어지면 리셋으로 본다 (%p)
const RESET_DROP: f64 = 10.0;
/// 리셋 시각 차이가 이만큼 안이면 같은 리셋으로 본다 (Codex 는 몇 초씩 흔들린다)
const RESET_TOLERANCE_SECONDS: i64 = 60 * 60;
/// 사이클 비교에 보여 줄 계정당 최대 주기 수 (최근 것부터)
const MAX_CYCLES_PER_ACCOUNT: usize = 12;

/// 히스토리 창 위쪽의 기간 선택
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Range {
    /// 이번 주기: 계정들의 이번 주 리셋 이후부터
    CurrentCycle,
    Days(i64),
    All,
}

impl Range {
    pub fn parse(text: &str) -> Range {
        match text {
            "cycle" => Range::CurrentCycle,
            "7d" => Range::Days(7),
            "30d" => Range::Days(30),
            _ => Range::All,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoryData {
    pub accounts: Vec<HistoryAccount>,
    pub range_start: DateTime<Utc>,
    pub range_end: DateTime<Utc>,
    pub weekly: Vec<HistorySeries>,
    pub session: Vec<HistorySeries>,
    pub cycles: Vec<HistoryCycle>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoryAccount {
    pub id: String,
    pub provider: Provider,
    pub name: String,
    pub label: AccountLabel,
    pub color: String,
}

/// 계정 하나의 사용률 추이
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HistorySeries {
    pub account_id: String,
    pub points: Vec<TrendPoint>,
    /// 기간 안의 리셋 시각
    pub resets: Vec<DateTime<Utc>>,
}

/// 주기 하나의 소진 곡선
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoryCycle {
    pub account_id: String,
    pub start: DateTime<Utc>,
    pub end: DateTime<Utc>,
    /// 지금 진행 중인 주기
    pub current: bool,
    /// 시작 시각을 리셋 시각으로 정확히 아는지 (모르면 첫 기록 시각)
    pub start_known: bool,
    pub points: Vec<CyclePoint>,
}

/// 주기 시작 후 지난 시간(시간 단위)과 그때의 사용률
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CyclePoint {
    pub hours: f64,
    pub percent: f64,
}

/// 기록을 주기로 나눈 한 덩어리
#[derive(Debug, Clone, PartialEq)]
pub struct Span {
    pub start: DateTime<Utc>,
    pub end: DateTime<Utc>,
    /// 시작이 정확한 리셋 시각인지 (아니면 하락을 본 시각 등 추정)
    pub start_known: bool,
    /// 앞에 주기가 있어서 시작 시각이 리셋 시각인지. 첫 주기는 기록이 주기 중간부터 시작됐을 수 있다.
    pub after_reset: bool,
    pub samples: Vec<Sample>,
}

/// 주기 경계 (리셋 시각)
#[derive(Debug, Clone, Copy)]
struct Boundary {
    at: DateTime<Utc>,
    exact: bool,
}

/// 히스토리 창 데이터 전체를 만든다.
pub fn build(
    history: &History,
    accounts: &[Account],
    live_resets: &std::collections::HashMap<String, DateTime<Utc>>,
    range: Range,
    now: DateTime<Utc>,
) -> Result<HistoryData, String> {
    // 1. 계정마다 주간 기록 전체를 주기로 나눈다 (수천 줄 정도라 금방 끝난다)
    let mut weekly_spans = Vec::new();
    for account in accounts {
        let samples = history.samples(&account.id, WindowKind::Weekly, None, now)?;
        let live_reset = live_resets.get(&account.id).copied();
        weekly_spans.push((account, split_cycles(&samples, live_reset, now)));
    }

    // 2. 기간 시작
    let week_ago = now - Duration::days(7);
    let range_start = match range {
        Range::CurrentCycle => weekly_spans
            .iter()
            .filter_map(|(_, spans)| spans.last())
            .filter(|span| span.end > now)
            .map(|span| span.start)
            .min()
            .unwrap_or(week_ago),
        Range::Days(days) => now - Duration::days(days),
        Range::All => {
            let mut earliest = weekly_spans
                .iter()
                .filter_map(|(_, spans)| spans.first())
                .filter_map(|span| span.samples.first())
                .map(|sample| sample.at)
                .min();
            for account in accounts {
                if let Some(first) = history
                    .samples(&account.id, WindowKind::Session, None, now)?
                    .first()
                {
                    earliest = Some(earliest.map_or(first.at, |at| at.min(first.at)));
                }
            }
            earliest.unwrap_or(week_ago).min(week_ago)
        }
    };

    // 3. 계정별 추이와 주기
    let mut weekly = Vec::new();
    let mut session = Vec::new();
    let mut cycles = Vec::new();
    for (account, spans) in &weekly_spans {
        let (points, resets) = weekly_series(spans, range_start, now);
        if !points.is_empty() {
            weekly.push(HistorySeries {
                account_id: account.id.clone(),
                points,
                resets,
            });
        }

        let session_samples =
            history.samples(&account.id, WindowKind::Session, Some(range_start), now)?;
        let session_points = clip(
            session_samples
                .iter()
                .map(|sample| TrendPoint {
                    at: sample.at,
                    percent: sample.used_percent,
                })
                .collect(),
            range_start,
            now,
        );
        if !session_points.is_empty() {
            session.push(HistorySeries {
                account_id: account.id.clone(),
                points: session_points,
                resets: Vec::new(),
            });
        }

        let shown: Vec<&Span> = spans
            .iter()
            .filter(|span| span.end > range_start && span.start < now && !span.samples.is_empty())
            .collect();
        let skip = shown.len().saturating_sub(MAX_CYCLES_PER_ACCOUNT);
        for span in shown.into_iter().skip(skip) {
            let current = span.start <= now && span.end > now;
            cycles.push(HistoryCycle {
                account_id: account.id.clone(),
                start: span.start,
                end: span.end,
                current,
                start_known: span.start_known,
                points: cycle_points(span, now),
            });
        }
    }

    Ok(HistoryData {
        accounts: accounts
            .iter()
            .map(|account| HistoryAccount {
                id: account.id.clone(),
                provider: account.provider,
                name: account.name.clone(),
                label: account.label.clone(),
                color: account.color.clone(),
            })
            .collect(),
        range_start,
        range_end: now,
        weekly,
        session,
        cycles,
    })
}

/// 시간순 기록을 주기로 나눈다. (방법은 맨 위 설명 참고)
/// `live_reset`: 지금 조회 중인 이번 주기의 리셋 시각 (기록에 아직 없을 수 있어서 따로 받는다)
pub fn split_cycles(
    samples: &[Sample],
    live_reset: Option<DateTime<Utc>>,
    now: DateTime<Utc>,
) -> Vec<Span> {
    let Some(first) = samples.first() else {
        return Vec::new();
    };
    let week = Duration::seconds(WEEK_SECONDS);
    let near =
        |a: DateTime<Utc>, b: DateTime<Utc>| (a - b).num_seconds().abs() <= RESET_TOLERANCE_SECONDS;

    // 1. 기록에 남은 정확한 리셋 시각
    let mut exact: Vec<DateTime<Utc>> = Vec::new();
    let recorded = samples
        .iter()
        .filter_map(|sample| sample.resets_at.filter(|reset| *reset > sample.at));
    for reset in recorded.chain(live_reset.filter(|reset| *reset > now)) {
        if !exact.iter().any(|known| near(*known, reset)) {
            exact.push(reset);
        }
    }
    exact.sort();
    let mut anchors: Vec<Boundary> = exact
        .iter()
        .map(|at| Boundary {
            at: *at,
            exact: true,
        })
        .collect();

    // 2. 리셋 시각이 없는 기록의 큰 폭 하락. 리셋은 (앞 기록, 하락 기록] 사이 어딘가에 있었다.
    for pair in samples.windows(2) {
        let (before, after) = (&pair[0], &pair[1]);
        if after.resets_at.is_some() || after.used_percent >= before.used_percent - RESET_DROP {
            continue;
        }
        let inside = |at: DateTime<Utc>| at > before.at && at <= after.at;
        if exact
            .iter()
            .any(|reset| inside(*reset) || near(*reset, after.at))
        {
            continue;
        }
        // 기준 시각에서 7일씩 옮긴 시각 중 이 구간에 드는 것
        let grid_point = |reference: DateTime<Utc>| {
            let weeks = (after.at - reference)
                .num_seconds()
                .div_euclid(WEEK_SECONDS);
            let candidate = reference + Duration::seconds(weeks * WEEK_SECONDS);
            inside(candidate).then_some(candidate)
        };
        // 정확한 리셋 시각에 맞추고, 없으면 바로 앞 기준점에 맞추고, 그것도 안 되면 하락을 본 시각
        let snapped = exact
            .iter()
            .find_map(|reset| grid_point(*reset))
            .map(|at| Boundary { at, exact: true })
            .or_else(|| {
                let previous = anchors
                    .iter()
                    .filter(|anchor| anchor.at <= before.at)
                    .max_by_key(|anchor| anchor.at)?;
                grid_point(previous.at).map(|at| Boundary {
                    at,
                    exact: previous.exact,
                })
            });
        anchors.push(snapped.unwrap_or(Boundary {
            at: after.at,
            exact: false,
        }));
    }
    anchors.sort_by_key(|anchor| anchor.at);

    // 하루 안에 붙은 기준점은 하나로 (정확한 것을 남긴다)
    let mut merged: Vec<Boundary> = Vec::new();
    for anchor in anchors {
        match merged.last_mut() {
            Some(last) if anchor.at - last.at < Duration::days(1) => {
                if anchor.exact && !last.exact {
                    *last = anchor;
                }
            }
            _ => merged.push(anchor),
        }
    }
    if merged.is_empty() {
        merged.push(Boundary {
            at: first.at,
            exact: false,
        });
    }

    // 3. 경계: 첫 기준점에서 첫 기록까지 거꾸로, 기준점 사이 빈 기간, 마지막 기준점에서 지금까지 7일 간격
    let mut boundaries: Vec<Boundary> = Vec::new();
    let mut back = merged[0];
    while back.at > first.at {
        back.at -= week;
        boundaries.push(back);
    }
    boundaries.reverse();
    for (index, anchor) in merged.iter().enumerate() {
        boundaries.push(*anchor);
        let mut next = Boundary {
            at: anchor.at + week,
            exact: anchor.exact,
        };
        match merged.get(index + 1) {
            Some(following) => {
                while next.at < following.at - Duration::days(1) {
                    boundaries.push(next);
                    next.at += week;
                }
            }
            None => {
                while boundaries.last().is_some_and(|last| last.at <= now) {
                    boundaries.push(next);
                    next.at += week;
                }
            }
        }
    }

    // 4. 경계 사이에 기록을 나눠 담는다. 기록이 없는 주기는 뺀다.
    let mut spans: Vec<Span> = Vec::new();
    for pair in boundaries.windows(2) {
        let (start, end) = (pair[0], pair[1]);
        let inside: Vec<Sample> = samples
            .iter()
            .filter(|sample| sample.at >= start.at && sample.at < end.at)
            .cloned()
            .collect();
        if inside.is_empty() {
            continue;
        }
        let after_reset = !spans.is_empty();
        spans.push(Span {
            start: start.at,
            end: end.at,
            start_known: start.exact,
            after_reset,
            samples: inside,
        });
    }
    spans
}

/// 주기 하나의 소진 곡선. 다음 기록 전까지 같은 값으로 보고, 주기 끝(진행 중이면 지금)까지 잇는다.
pub fn cycle_points(span: &Span, now: DateTime<Utc>) -> Vec<CyclePoint> {
    let hours = |at: DateTime<Utc>| ((at - span.start).num_seconds() as f64 / 3600.0).max(0.0);
    let mut points = Vec::new();
    if starts_at_zero(span) {
        points.push(CyclePoint {
            hours: 0.0,
            percent: 0.0,
        });
    }
    for sample in &span.samples {
        points.push(CyclePoint {
            hours: hours(sample.at),
            percent: sample.used_percent,
        });
    }
    let end = hours(span.end.min(now));
    if let Some(last) = points.last().cloned()
        && end > last.hours
    {
        points.push(CyclePoint {
            hours: end,
            percent: last.percent,
        });
    }
    points
}

/// 리셋으로 시작한 주기면 시작 시각에 0% 점을 넣는다. (리셋 직후 사용률은 0%)
/// 첫 주기는 기록이 주기 중간부터 시작됐을 수 있어서 넣지 않는다.
fn starts_at_zero(span: &Span) -> bool {
    span.after_reset
        && span
            .samples
            .first()
            .is_some_and(|first| first.at > span.start)
}

/// 주간 사용률 추이 (기간 안). 리셋 시각에 0% 로 떨어지게 그린다.
pub fn weekly_series(
    spans: &[Span],
    from: DateTime<Utc>,
    now: DateTime<Utc>,
) -> (Vec<TrendPoint>, Vec<DateTime<Utc>>) {
    let mut points = Vec::new();
    let mut resets = Vec::new();
    for span in spans {
        if span.after_reset {
            resets.push(span.start);
        }
        if starts_at_zero(span) {
            points.push(TrendPoint {
                at: span.start,
                percent: 0.0,
            });
        }
        for sample in &span.samples {
            points.push(TrendPoint {
                at: sample.at,
                percent: sample.used_percent,
            });
        }
    }
    // 마지막 주기가 이미 끝났으면(앱이 꺼져 있던 사이 리셋) 리셋 시각에 0% 로 떨어뜨린다
    if let Some(last) = spans.last()
        && last.end <= now
        && last.start_known
    {
        points.push(TrendPoint {
            at: last.end,
            percent: 0.0,
        });
        resets.push(last.end);
    }
    resets.retain(|at| *at >= from && *at <= now);
    (clip(points, from, now), resets)
}

/// 기간으로 자른다: 기간 시작 직전 값에서 출발하고, 마지막 값을 지금까지 잇는다.
fn clip(points: Vec<TrendPoint>, from: DateTime<Utc>, now: DateTime<Utc>) -> Vec<TrendPoint> {
    let mut clipped = Vec::new();
    if let Some(before) = points.iter().rfind(|point| point.at < from) {
        clipped.push(TrendPoint {
            at: from,
            percent: before.percent,
        });
    }
    clipped.extend(
        points
            .into_iter()
            .filter(|point| point.at >= from && point.at <= now),
    );
    if let Some(last) = clipped.last().cloned()
        && last.at < now
    {
        clipped.push(TrendPoint {
            at: now,
            percent: last.percent,
        });
    }
    clipped
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn time(day: u32, hour: u32) -> DateTime<Utc> {
        Utc.with_ymd_and_hms(2026, 11, day, hour, 0, 0).unwrap()
    }

    fn sample(at: DateTime<Utc>, percent: f64, reset: Option<DateTime<Utc>>) -> Sample {
        Sample {
            at,
            used_percent: percent,
            resets_at: reset,
        }
    }

    #[test]
    fn known_reset_times_split_cycles_exactly() {
        // 매주 목요일 10시 리셋
        let first_reset = time(12, 10);
        let second_reset = time(19, 10);
        let samples = vec![
            sample(time(6, 12), 5.0, Some(first_reset)),
            sample(time(11, 9), 80.0, Some(first_reset)),
            // 리셋 뒤 첫 기록 (값이 낮고, 리셋 시각이 일주일 뒤로)
            sample(time(13, 8), 3.0, Some(second_reset)),
            sample(time(16, 8), 40.0, Some(second_reset)),
        ];
        let now = time(17, 10);
        let spans = split_cycles(&samples, None, now);
        assert_eq!(spans.len(), 2);
        assert_eq!(spans[0].start, time(5, 10));
        assert_eq!(spans[0].end, first_reset);
        assert!(
            !spans[0].after_reset,
            "첫 주기는 기록이 중간부터 시작됐을 수 있다"
        );
        assert!(spans[1].start_known && spans[1].after_reset);
        assert_eq!(spans[1].start, first_reset);

        // 둘째 주기 곡선: 리셋 시각에 0% 로 시작해서, 지금까지 이어진다
        let points = cycle_points(&spans[1], now);
        assert_eq!(
            points.first().unwrap(),
            &CyclePoint {
                hours: 0.0,
                percent: 0.0
            }
        );
        assert_eq!(points.last().unwrap().hours, 5.0 * 24.0);
        assert_eq!(points.last().unwrap().percent, 40.0);
        // 첫 주기는 기록이 시작된 곳에서 시작 (0% 점을 지어내지 않는다)
        assert_eq!(cycle_points(&spans[0], now).first().unwrap().percent, 5.0);
    }

    #[test]
    fn old_records_without_reset_times_split_on_big_drops() {
        let samples = vec![
            sample(time(1, 0), 10.0, None),
            sample(time(3, 0), 60.0, None),
            sample(time(6, 0), 90.0, None),
            sample(time(8, 0), 2.0, None), // 큰 폭 하락 → 새 주기
            sample(time(9, 0), 15.0, None),
        ];
        let spans = split_cycles(&samples, None, time(10, 0));
        assert_eq!(spans.len(), 2);
        assert!(!spans[1].start_known);
        assert_eq!(spans[1].start, time(8, 0));
        assert_eq!(spans[0].start, time(1, 0));
        assert_eq!(
            spans[0].end,
            time(8, 0),
            "앞 주기는 다음 주기 시작에서 끝난다"
        );
    }

    #[test]
    fn gaps_are_filled_weekly_instead_of_making_short_cycles() {
        // 10/20 하락(리셋) 뒤 앱이 2주 꺼져 있다가 11/3 에 다시 기록, 그날 또 하락
        let at = |month: u32, day: u32, hour: u32| {
            Utc.with_ymd_and_hms(2026, month, day, hour, 0, 0).unwrap()
        };
        let samples = vec![
            sample(at(10, 18, 0), 80.0, None),
            sample(at(10, 20, 15), 1.0, None), // 리셋
            sample(at(10, 25, 0), 50.0, None),
            sample(at(11, 3, 7), 30.0, None), // 기록 공백 뒤 (리셋이 한 번 지나감)
            sample(at(11, 3, 15), 2.0, None), // 리셋
        ];
        let spans = split_cycles(&samples, None, at(11, 4, 0));
        let starts: Vec<DateTime<Utc>> = spans.iter().map(|span| span.start).collect();
        assert_eq!(
            starts,
            vec![
                at(10, 13, 15),
                at(10, 20, 15),
                at(10, 27, 15),
                at(11, 3, 15)
            ]
        );
        // 공백 뒤 첫 기록(30%)은 리셋(10/27 15시) 뒤의 값: 그 주기 곡선은 0% 에서 시작한다
        let points = cycle_points(&spans[2], at(11, 4, 0));
        assert_eq!(
            points.first().unwrap(),
            &CyclePoint {
                hours: 0.0,
                percent: 0.0
            }
        );
    }

    #[test]
    fn drops_snap_to_the_weekly_grid_of_known_resets() {
        // 정확한 리셋: 11/19 10시. 예전 기록의 하락은 11/12 10시 이후 처음 본 11/12 15시
        let samples = vec![
            sample(time(11, 0), 70.0, None),
            sample(time(12, 15), 3.0, None),
            sample(time(14, 0), 20.0, Some(time(19, 10))),
        ];
        let spans = split_cycles(&samples, None, time(15, 0));
        assert_eq!(spans.len(), 2);
        assert_eq!(
            spans[1].start,
            time(12, 10),
            "하락 구간 안의 7일 간격 리셋 시각으로 맞춤"
        );
        assert!(spans[1].start_known);
    }

    #[test]
    fn weekly_series_drops_to_zero_at_resets_and_is_clipped_to_the_range() {
        let first_reset = time(12, 10);
        let second_reset = time(19, 10);
        let samples = vec![
            sample(time(10, 0), 70.0, Some(first_reset)),
            sample(time(14, 0), 20.0, Some(second_reset)),
        ];
        let now = time(15, 0);
        let spans = split_cycles(&samples, None, now);
        let (points, resets) = weekly_series(&spans, time(11, 0), now);
        let values: Vec<(DateTime<Utc>, f64)> = points
            .iter()
            .map(|point| (point.at, point.percent))
            .collect();
        assert_eq!(
            values,
            vec![
                (time(11, 0), 70.0), // 기간 시작 직전 값
                (first_reset, 0.0),  // 리셋 시각에 0%
                (time(14, 0), 20.0),
                (now, 20.0), // 지금까지 이어짐
            ]
        );
        assert_eq!(resets, vec![first_reset]);
    }
}
