//! 모델별 API 가격표. 구독으로 쓴 토큰을 "API 로 냈다면 얼마였을지"로 환산할 때 쓴다 (추정).
//!
//! 가격은 100만 토큰당 달러. 출처 (2026-11-21 확인):
//! - Claude: https://platform.claude.com/docs/en/about-claude/pricing
//! - OpenAI: https://developers.openai.com/api/docs/models/gpt-6-astra
//!
//! 어림인 이유
//! - 표준 요금만 쓴다. 빠른 모드(2배), 배치(절반), 아주 긴 프롬프트 할증은 반영하지 않는다.
//! - 가격은 바뀔 수 있다. 새 모델이 나오면 아래 표에 한 줄 추가한다.
//!   표에 없는 모델은 금액에서 빼고, 뺀 토큰 수를 화면에 따로 알려 준다.

/// 100만 토큰당 가격 (달러)
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Price {
    pub input: f64,
    pub output: f64,
    /// 5분 캐시 쓰기
    pub cache_write_5m: f64,
    /// 1시간 캐시 쓰기 (Claude Code 는 거의 항상 이쪽을 쓴다)
    pub cache_write_1h: f64,
    /// 캐시 읽기
    pub cache_read: f64,
}

/// 가격을 매길 토큰 수
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct TokenCounts {
    /// 캐시를 뺀 입력
    pub input: u64,
    pub output: u64,
    pub cache_write_5m: u64,
    pub cache_write_1h: u64,
    pub cache_read: u64,
}

const fn price(input: f64, output: f64, write_5m: f64, write_1h: f64, read: f64) -> Price {
    Price {
        input,
        output,
        cache_write_5m: write_5m,
        cache_write_1h: write_1h,
        cache_read: read,
    }
}

/// (모델 이름, 가격). 이름은 로그에 찍히는 모델 ID 다.
const PRICES: &[(&str, Price)] = &[
    // Claude                           입력   출력   쓰기5분 쓰기1시간 읽기
    ("claude-fable-5-2", price(8.0, 40.0, 10.0, 16.0, 0.20)),
    ("claude-mythos-5-2", price(8.0, 40.0, 10.0, 16.0, 0.20)),
    ("claude-fable-5-1", price(10.0, 50.0, 12.5, 20.0, 0.25)),
    ("claude-mythos-5-1", price(10.0, 50.0, 12.5, 20.0, 0.25)),
    ("claude-fable-5", price(10.0, 50.0, 12.5, 20.0, 1.0)),
    ("claude-mythos-5", price(10.0, 50.0, 12.5, 20.0, 1.0)),
    ("claude-opus-5-5", price(4.0, 20.0, 5.0, 8.0, 0.20)),
    ("claude-opus-5", price(5.0, 25.0, 6.25, 10.0, 0.50)),
    ("claude-opus-4-8", price(5.0, 25.0, 6.25, 10.0, 0.50)),
    ("claude-opus-4-7", price(5.0, 25.0, 6.25, 10.0, 0.50)),
    ("claude-opus-4-6", price(5.0, 25.0, 6.25, 10.0, 0.50)),
    ("claude-opus-4-5", price(5.0, 25.0, 6.25, 10.0, 0.50)),
    ("claude-sonnet-5-5", price(2.0, 10.0, 2.5, 4.0, 0.10)),
    ("claude-sonnet-5", price(2.0, 10.0, 2.5, 4.0, 0.20)),
    ("claude-sonnet-4-6", price(3.0, 15.0, 3.75, 6.0, 0.30)),
    ("claude-sonnet-4-5", price(3.0, 15.0, 3.75, 6.0, 0.30)),
    // Haiku 5.5 는 프롬프트 10만 토큰 이하 요금 (그보다 길면 5배)
    ("claude-haiku-5-5", price(0.10, 0.50, 0.125, 0.20, 0.01)),
    ("claude-haiku-4-5", price(1.0, 5.0, 1.25, 2.0, 0.10)),
    // OpenAI (Codex). 캐시 쓰기 요금은 하나뿐이라 두 칸에 같은 값을 넣는다.
    ("gpt-6-astra", price(10.0, 50.0, 12.5, 12.5, 1.0)),
];

/// 모델 이름으로 가격을 찾는다. 모르는 모델이면 `None`.
/// 이름 뒤에 날짜("-20251101")나 "[1m]" 같은 꼬리가 붙어도 같은 모델로 본다.
/// ("claude-opus-5" 가 "claude-opus-5-5" 에 걸리지 않게, 꼬리 모양을 정해 둔다)
pub fn price_for(model: &str) -> Option<&'static Price> {
    PRICES.iter().find_map(|(name, price)| {
        let rest = model.strip_prefix(name)?;
        let same_model = rest.is_empty()
            || rest.starts_with("-20")
            || rest.starts_with('[')
            || rest.starts_with('@');
        same_model.then_some(price)
    })
}

/// 토큰 수에 가격을 곱한 금액 (달러)
pub fn cost_usd(price: &Price, tokens: &TokenCounts) -> f64 {
    let dollars = tokens.input as f64 * price.input
        + tokens.output as f64 * price.output
        + tokens.cache_write_5m as f64 * price.cache_write_5m
        + tokens.cache_write_1h as f64 * price.cache_write_1h
        + tokens.cache_read as f64 * price.cache_read;
    dollars / 1_000_000.0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn similar_model_names_do_not_get_mixed_up() {
        assert_eq!(price_for("claude-opus-5-5").unwrap().output, 20.0);
        assert_eq!(price_for("claude-opus-5").unwrap().output, 25.0);
        assert_eq!(price_for("claude-fable-5").unwrap().cache_read, 1.0);
        assert_eq!(price_for("claude-fable-5-2").unwrap().cache_read, 0.20);
        // 날짜나 [1m] 꼬리는 같은 모델
        assert_eq!(price_for("claude-opus-4-5-20251101").unwrap().input, 5.0);
        assert_eq!(price_for("claude-opus-5-5[1m]").unwrap().input, 4.0);
        // 모르는 모델과, 이름이 비슷하지만 다른 모델
        assert!(price_for("gpt-6-astra-pro").is_none());
        assert!(price_for("unknown").is_none());
        assert!(price_for("<synthetic>").is_none());
    }

    #[test]
    fn cost_adds_each_token_kind_at_its_own_price() {
        let million = TokenCounts {
            input: 1_000_000,
            output: 1_000_000,
            cache_write_5m: 1_000_000,
            cache_write_1h: 1_000_000,
            cache_read: 1_000_000,
        };
        // Fable 5.2: 8 + 40 + 10 + 16 + 0.20
        let cost = cost_usd(price_for("claude-fable-5-2").unwrap(), &million);
        assert!((cost - 74.2).abs() < 1e-9);
        // Codex: 캐시 입력은 일반 입력의 1/10
        let codex = TokenCounts {
            input: 2_000_000,
            cache_read: 10_000_000,
            output: 100_000,
            ..TokenCounts::default()
        };
        let cost = cost_usd(price_for("gpt-6-astra").unwrap(), &codex);
        assert!((cost - (20.0 + 10.0 + 5.0)).abs() < 1e-9);
    }
}
