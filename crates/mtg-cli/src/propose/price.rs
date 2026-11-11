//! What the Anthropic models cost: a table of prices per million tokens and the cost of a request in micro-dollars,
//! rounded up so that a sum of costs never falls short of the bill.

use std::fmt::{self, Write as _};

use super::model::{Request, Usage};

/// An amount of money in micro-dollars (millionths of a US dollar).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord)]
pub struct Usd(pub u64);

impl Usd {
    /// The amount as `12.345678`, without a currency sign (the ledger's column).
    pub fn plain(self) -> String {
        format!("{}.{:06}", self.0 / 1_000_000, self.0 % 1_000_000)
    }

    /// The sum, saturating.
    #[must_use]
    pub fn plus(self, other: Usd) -> Usd {
        Usd(self.0.saturating_add(other.0))
    }

    /// The difference, zero when `other` is larger.
    #[must_use]
    pub fn minus(self, other: Usd) -> Usd {
        Usd(self.0.saturating_sub(other.0))
    }
}

impl fmt::Display for Usd {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "${}", self.plain())
    }
}

/// Reads dollars as `12`, `0.5` or `200.000001` (at most six decimals); zero and anything else are refused.
pub fn parse_usd(text: &str) -> Result<Usd, String> {
    let bad = || format!("`{text}` is not an amount of dollars such as 20 or 0.50");
    let (whole, fraction) = text.trim().split_once('.').unwrap_or((text.trim(), ""));
    let digits = |part: &str| part.chars().all(|c| c.is_ascii_digit());
    if whole.is_empty() || !digits(whole) || !digits(fraction) || fraction.len() > 6 {
        return Err(bad());
    }
    let whole: u64 = whole.parse().map_err(|_| bad())?;
    let fraction: u64 = format!("{fraction:0<6}").parse().map_err(|_| bad())?;
    let micros = whole
        .checked_mul(1_000_000)
        .and_then(|whole| whole.checked_add(fraction))
        .ok_or_else(bad)?;
    if micros == 0 {
        return Err("the amount must be more than zero".into());
    }
    Ok(Usd(micros))
}

/// The prices of a model in micro-dollars per million tokens.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rates {
    /// A new input token.
    pub input: u64,
    /// An output token.
    pub output: u64,
}

/// What the table knows about a model.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Price {
    /// The model id without a date.
    pub id: &'static str,
    /// The prices up to `long_over` input tokens.
    pub rates: Rates,
    /// The prices of a request with more input tokens than the first of the pair, when the model has a dearer tier.
    pub long: Option<(u64, Rates)>,
}

const fn rates(input_dollars_milli: u64, output_dollars_milli: u64) -> Rates {
    Rates {
        input: input_dollars_milli * 1_000,
        output: output_dollars_milli * 1_000,
    }
}

/// The models `propose` can price, from the Anthropic price list (dollars per million tokens, here in thousandths).
const TABLE: &[Price] = &[
    Price {
        id: "claude-opus-5-5",
        rates: rates(4_000, 20_000),
        long: None,
    },
    Price {
        id: "claude-sonnet-5-5",
        rates: rates(2_000, 10_000),
        long: None,
    },
    Price {
        id: "claude-haiku-5-5",
        rates: rates(100, 500),
        long: Some((100_000, rates(500, 2_500))),
    },
    Price {
        id: "claude-opus-5",
        rates: rates(5_000, 25_000),
        long: None,
    },
    Price {
        id: "claude-opus-4-8",
        rates: rates(5_000, 25_000),
        long: None,
    },
    Price {
        id: "claude-sonnet-5",
        rates: rates(2_000, 10_000),
        long: None,
    },
    Price {
        id: "claude-sonnet-4-6",
        rates: rates(3_000, 15_000),
        long: None,
    },
    Price {
        id: "claude-haiku-4-5",
        rates: rates(1_000, 5_000),
        long: None,
    },
];

/// The model ids with a price, for a message that refuses another.
pub fn known_models() -> Vec<&'static str> {
    TABLE.iter().map(|price| price.id).collect()
}

/// The full id a short name stands for: `opus`, `sonnet` and `haiku` are the current models of those families.
pub fn model_id(name: &str) -> &str {
    match name {
        "opus" => "claude-opus-5-5",
        "sonnet" => "claude-sonnet-5-5",
        "haiku" => "claude-haiku-5-5",
        other => other,
    }
}

/// The price of a model by its id, with or without a trailing `-YYYYMMDD` date.
pub fn price_of(model: &str) -> Option<Price> {
    let bare = match model.rsplit_once('-') {
        Some((head, tail)) if tail.len() == 8 && tail.bytes().all(|b| b.is_ascii_digit()) => head,
        _ => model,
    };
    TABLE.iter().find(|price| price.id == bare).copied()
}

/// How a request is billed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Billing {
    /// One request, one answer: the full price.
    Standard,
    /// The Message Batches API: half the price.
    Batch,
}

/// `tokens` at `rate` micro-dollars per million tokens times `numerator / denominator`, rounded up.
fn charge(tokens: u64, rate: u64, numerator: u128, denominator: u128) -> u64 {
    let scaled = u128::from(tokens) * u128::from(rate) * numerator;
    let per = denominator * 1_000_000;
    u64::try_from(scaled.div_ceil(per)).unwrap_or(u64::MAX)
}

impl Price {
    /// The rates a request with `input_tokens` input tokens is billed at.
    fn tier(&self, input_tokens: u64) -> Rates {
        match self.long {
            Some((over, dearer)) if input_tokens > over => dearer,
            _ => self.rates,
        }
    }

    /// What a request cost: new input at the input rate, cache writes at 1.25 times it, cache reads at 0.1 times it and the
    /// answer at the output rate; a batch is billed at half. Each kind of token rounds up on its own.
    pub fn cost(&self, usage: &Usage, billing: Billing) -> Usd {
        let rates = self.tier(usage.input_tokens);
        let half = match billing {
            Billing::Standard => 1,
            Billing::Batch => 2,
        };
        let total = charge(usage.uncached_tokens(), rates.input, 1, half)
            .saturating_add(charge(usage.cache_write_tokens, rates.input, 5, 4 * half))
            .saturating_add(charge(usage.cache_read_tokens, rates.input, 1, 10 * half))
            .saturating_add(charge(usage.output_tokens, rates.output, 1, half));
        Usd(total)
    }

    /// The most a request can cost: every input token as a cache write (the dearest kind, by the upper estimate of the
    /// tokens) and the answer as long as `max_tokens` allows.
    pub fn worst_case(&self, request: &Request, billing: Billing) -> Usd {
        let input = request.estimated_tokens();
        self.cost(
            &Usage {
                input_tokens: input,
                cache_write_tokens: input,
                cache_read_tokens: 0,
                output_tokens: u64::from(request.max_tokens),
            },
            billing,
        )
    }
}

/// The answer tokens a card is assumed to take: a rules file of a few abilities.
pub const TYPICAL_ANSWER_TOKENS: u64 = 1_500;

/// The tokens a repair message adds to the conversation, apart from the model's own answer.
const REPAIR_NOTE_TOKENS: u64 = 400;

/// What a run is expected to cost a card, from the size of its prompts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Estimate {
    /// A card whose rules are read from the cache.
    pub steady: Usd,
    /// The same card with a repair round.
    pub with_repair: Usd,
    /// The first request of a run, which writes the rules to the cache.
    pub cache_write: Usd,
    /// A card in a batch whose rules are not cached.
    pub batch_cold: Usd,
    /// A card in a batch whose rules are cached.
    pub batch_warm: Usd,
    /// The worst case of one request, at the standard price.
    pub worst: Usd,
}

impl Price {
    /// The expected cost of a card with `system` shared input tokens (the rules) and `card` tokens of its own, with
    /// [`TYPICAL_ANSWER_TOKENS`] of answer; `worst` is a request with the answer allowance of `request`.
    pub fn estimate(&self, system: u64, card: u64, request: &Request) -> Estimate {
        let asked = |write: u64, read: u64| Usage {
            input_tokens: system + card,
            cache_write_tokens: write,
            cache_read_tokens: read,
            output_tokens: TYPICAL_ANSWER_TOKENS,
        };
        let steady = self.cost(&asked(0, system), Billing::Standard);
        let repair = Usage {
            input_tokens: system + card + TYPICAL_ANSWER_TOKENS + REPAIR_NOTE_TOKENS,
            cache_write_tokens: 0,
            cache_read_tokens: system,
            output_tokens: TYPICAL_ANSWER_TOKENS,
        };
        Estimate {
            steady,
            with_repair: steady.plus(self.cost(&repair, Billing::Standard)),
            cache_write: self.cost(&asked(system, 0), Billing::Standard),
            batch_cold: self.cost(&asked(0, 0), Billing::Batch),
            batch_warm: self.cost(&asked(0, system), Billing::Batch),
            worst: self.worst_case(request, Billing::Standard),
        }
    }
}

impl Estimate {
    /// The estimate as the lines of a dry run; `cap` adds how far `--max-spend` goes.
    pub fn lines(&self, model: &str, cap: Option<Usd>) -> String {
        let mut text = format!(
            "estimated cost ({model}, the rules read from the cache after the first request, {TYPICAL_ANSWER_TOKENS} answer tokens a card):\n  \
             a card: {}; with a repair round: {}\n  \
             the first request writes the rules to the cache: {}\n  \
             --batch: {} a card when its rules are not cached (a batch caches on a best effort), {} when they are\n  \
             worst case of one request: {}",
            self.steady,
            self.with_repair,
            self.cache_write,
            self.batch_cold,
            self.batch_warm,
            self.worst
        );
        if let Some(cap) = cap {
            let cards = cap.0.checked_div(self.with_repair.0.max(1)).unwrap_or(0);
            let requests = cap.0.checked_div(self.worst.0.max(1)).unwrap_or(0);
            let _ = write!(
                text,
                "\n  --max-spend {cap}: about {cards} cards at the expected price with a repair round each, and the run stops at the first request that does not fit its worst case (at least {requests} requests)"
            );
        }
        text
    }
}

#[cfg(test)]
mod tests {
    //! Goal: amounts parse and print as dollars, the table prices each model the way the price list does (cache writes
    //! at 1.25 times, reads at 0.1 times, a batch at half), costs round up and a request's worst case is every input token
    //! as a cache write plus the whole answer allowance.
    use rstest::rstest;

    use super::*;
    use crate::propose::model::Message;

    fn usage(input: u64, write: u64, read: u64, output: u64) -> Usage {
        Usage {
            input_tokens: input,
            cache_write_tokens: write,
            cache_read_tokens: read,
            output_tokens: output,
        }
    }

    #[rstest]
    #[case("200", Some(200_000_000))]
    #[case("0.5", Some(500_000))]
    #[case("20.000001", Some(20_000_001))]
    #[case(" 3 ", Some(3_000_000))]
    #[case("0", None)]
    #[case("0.0", None)]
    #[case("-5", None)]
    #[case("1.2345678", None)]
    #[case("abc", None)]
    #[case("", None)]
    #[case(".5", None)]
    #[case("$5", None)]
    fn dollars_parse_to_micro_dollars(#[case] text: &str, #[case] want: Option<u64>) {
        assert_eq!(parse_usd(text).ok().map(|usd| usd.0), want);
    }

    #[test]
    fn an_amount_prints_with_six_decimals() {
        assert_eq!(Usd(1_500_000).plain(), "1.500000");
        assert_eq!(Usd(7).to_string(), "$0.000007");
        assert_eq!(Usd(200_000_000).to_string(), "$200.000000");
    }

    #[rstest]
    #[case::opus("claude-opus-5-5", 4_000_000, 20_000_000)]
    #[case::sonnet("claude-sonnet-5-5", 2_000_000, 10_000_000)]
    #[case::haiku("claude-haiku-5-5", 100_000, 500_000)]
    #[case::a_dated_id("claude-sonnet-5-5-20261001", 2_000_000, 10_000_000)]
    #[case::the_older_opus("claude-opus-5", 5_000_000, 25_000_000)]
    fn the_table_prices_each_model(#[case] id: &str, #[case] input: u64, #[case] output: u64) {
        let price = price_of(id).unwrap();
        assert_eq!((price.rates.input, price.rates.output), (input, output));
    }

    #[test]
    fn an_unknown_model_has_no_price_and_a_short_name_is_the_current_model() {
        assert!(price_of("gpt-oss:20b").is_none());
        assert!(price_of("claude-sonnet").is_none());
        assert_eq!(model_id("sonnet"), "claude-sonnet-5-5");
        assert_eq!(model_id("claude-opus-5-5"), "claude-opus-5-5");
        assert!(known_models().contains(&"claude-haiku-5-5"));
    }

    #[rstest]
    // 1M of each kind on Sonnet: $2 new, $2.50 written, $0.20 read, $10 out.
    #[case::new_input(usage(1_000_000, 0, 0, 0), Billing::Standard, 2_000_000)]
    #[case::cache_write(usage(1_000_000, 1_000_000, 0, 0), Billing::Standard, 2_500_000)]
    #[case::cache_read(usage(1_000_000, 0, 1_000_000, 0), Billing::Standard, 200_000)]
    #[case::output(usage(0, 0, 0, 1_000_000), Billing::Standard, 10_000_000)]
    #[case::a_batch_is_half(usage(1_000_000, 1_000_000, 0, 1_000_000), Billing::Batch, 6_250_000)]
    #[case::mixed(usage(10_000, 1_000, 8_000, 500), Billing::Standard, 2_000 + 2_500 + 1_600 + 5_000)]
    fn a_cost_is_each_kind_of_token_at_its_rate(
        #[case] used: Usage,
        #[case] billing: Billing,
        #[case] micros: u64,
    ) {
        let sonnet = price_of("claude-sonnet-5-5").unwrap();
        assert_eq!(sonnet.cost(&used, billing), Usd(micros));
    }

    #[test]
    fn a_cost_rounds_up_to_the_next_micro_dollar() {
        // One output token of Haiku is 0.5 micro-dollars; a cache read of one token is 0.01.
        let haiku = price_of("claude-haiku-5-5").unwrap();
        assert_eq!(haiku.cost(&usage(0, 0, 0, 1), Billing::Standard), Usd(1));
        assert_eq!(haiku.cost(&usage(1, 0, 1, 0), Billing::Standard), Usd(1));
        assert_eq!(haiku.cost(&usage(0, 0, 0, 0), Billing::Standard), Usd(0));
    }

    #[test]
    fn haiku_has_a_dearer_tier_above_a_hundred_thousand_input_tokens() {
        let haiku = price_of("claude-haiku-5-5").unwrap();
        let cheap = haiku.cost(&usage(100_000, 0, 0, 0), Billing::Standard);
        let dear = haiku.cost(&usage(100_001, 0, 0, 0), Billing::Standard);
        assert_eq!(cheap, Usd(10_000));
        assert_eq!(dear, Usd(50_001));
    }

    #[test]
    fn the_worst_case_writes_every_input_token_to_the_cache_and_uses_every_answer_token() {
        let sonnet = price_of("claude-sonnet-5-5").unwrap();
        let request = Request {
            model: "claude-sonnet-5-5".into(),
            system: "x".repeat(299_997),
            messages: vec![Message::user("y".repeat(3))],
            max_tokens: 8_000,
        };
        // 300 000 bytes are at most 100 000 tokens: $0.25 as a cache write, 8000 answer tokens $0.08.
        assert_eq!(request.estimated_tokens(), 100_000);
        assert_eq!(
            sonnet.worst_case(&request, Billing::Standard),
            Usd(250_000 + 80_000)
        );
        assert_eq!(sonnet.worst_case(&request, Billing::Batch), Usd(165_000));
    }
}
