//! The dry run of line mode: which names are eligible and why the others are not, the size of the requests, and what a
//! card is expected to cost on each priced model, read from the sizes of its own prompts.

use std::collections::BTreeMap;
use std::fmt::Write as _;

use super::LineCtx;
use crate::error::CliError;
use crate::propose::model::{Request, Usage, estimate_tokens};
use crate::propose::price::{Billing, Price, Usd, price_of};

/// The answer tokens a line is assumed to take, thinking included (billed as answer tokens).
pub const LINE_TYPICAL_ANSWER_TOKENS: u64 = 800;

/// The tokens a repair message adds to a conversation, apart from the model's own answer.
const REPAIR_NOTE_TOKENS: u64 = 400;

/// The models the dry run prices.
pub const PRICED: [&str; 3] = ["claude-opus-5-5", "claude-sonnet-5-5", "claude-haiku-5-5"];

/// The token sizes of one line's first request: the system prompt (cached per kind) and the rest.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LineSize {
    /// The system prompt's tokens.
    pub system: u64,
    /// The message's tokens.
    pub message: u64,
}

impl LineSize {
    /// The sizes of a request, by the upper estimate of 3 bytes a token.
    pub fn of(request: &Request) -> LineSize {
        let system = estimate_tokens(request.system.len());
        LineSize {
            system,
            message: request.estimated_tokens().saturating_sub(system),
        }
    }
}

/// What a card is expected to cost on one model.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct CardCost {
    /// The first answers, the system prompt read from the cache.
    pub first: Usd,
    /// The first answers and one repair round for every line.
    pub with_repair: Usd,
    /// The first answers through a batch, the system prompt cached.
    pub batch_warm: Usd,
    /// The first answers through a batch, nothing cached.
    pub batch_cold: Usd,
}

/// The expected cost of a card whose lines have `sizes`, each asked `samples` times.
pub fn card_cost(price: &Price, sizes: &[LineSize], samples: u64) -> CardCost {
    let mut cost = CardCost::default();
    let answer = LINE_TYPICAL_ANSWER_TOKENS;
    for size in sizes {
        let input = size.system + size.message;
        let asked = |read: u64| Usage {
            input_tokens: input,
            cache_write_tokens: 0,
            cache_read_tokens: read,
            output_tokens: answer,
        };
        let times = |usd: Usd| Usd(usd.0.saturating_mul(samples));
        let first = times(price.cost(&asked(size.system), Billing::Standard));
        let repair = price.cost(
            &Usage {
                input_tokens: input + answer + REPAIR_NOTE_TOKENS,
                cache_write_tokens: 0,
                cache_read_tokens: size.system,
                output_tokens: answer,
            },
            Billing::Standard,
        );
        cost.first = cost.first.plus(first);
        cost.with_repair = cost.with_repair.plus(first).plus(repair);
        cost.batch_warm = cost
            .batch_warm
            .plus(times(price.cost(&asked(size.system), Billing::Batch)));
        cost.batch_cold = cost
            .batch_cold
            .plus(times(price.cost(&asked(0), Billing::Batch)));
    }
    cost
}

/// The reason of a skipped name without its details (a path, a count), for counting.
fn reason_key(why: &str) -> String {
    if why.starts_with("no translation at") {
        return "no translation".into();
    }
    if why.ends_with("gapped lines") {
        return "more than two gapped lines".into();
    }
    why.split(':').next().unwrap_or(why).to_owned()
}

impl LineCtx<'_> {
    /// What a run over `names` would send, without sending anything: the eligible cards, the size of the requests, the
    /// expected cost per card on each priced model and the first prompt; `cap` adds how far `--max-spend` goes.
    pub fn dry_run(
        &self,
        names: &[String],
        samples: usize,
        cap: Option<Usd>,
    ) -> Result<String, CliError> {
        let mut skipped: BTreeMap<String, usize> = BTreeMap::new();
        let mut cards: Vec<Vec<LineSize>> = Vec::new();
        let mut first: Option<(String, Vec<Request>)> = None;
        for name in names {
            match self.prepare(name) {
                Ok(prepared) => {
                    cards.push(prepared.requests.iter().map(LineSize::of).collect());
                    first.get_or_insert_with(|| (prepared.card.name.clone(), prepared.requests));
                }
                Err(why) => *skipped.entry(reason_key(&why)).or_default() += 1,
            }
        }
        let Some((first_name, first_requests)) = first else {
            return Err(CliError::Config("no name can be asked in line mode".into()));
        };
        let lines: usize = cards.iter().map(Vec::len).sum();
        let two = cards.iter().filter(|sizes| sizes.len() == 2).count();
        let all: Vec<u64> = cards
            .iter()
            .flatten()
            .map(|size| size.system + size.message)
            .collect();
        let total: u64 = all.iter().sum();
        let mean = total / u64::try_from(all.len().max(1)).unwrap_or(1);
        let largest = all.iter().copied().max().unwrap_or(0);
        let mut text = format!(
            "propose --lines --dry-run: nothing is sent\nmodel: {}\nnames: {}; eligible: {} cards ({} with two gapped lines), {lines} lines; samples per line: {samples}\n",
            self.model,
            names.len(),
            cards.len(),
            two
        );
        for (why, count) in &skipped {
            let _ = writeln!(text, "  skipped, {why}: {count}");
        }
        let _ = writeln!(
            text,
            "input tokens of a line request: {mean} on average, {largest} at most (an upper estimate, 3 bytes a token); the system prompt of a kind is cached"
        );
        let count = u64::try_from(cards.len()).unwrap_or(u64::MAX);
        let samples_n = u64::try_from(samples.max(1)).unwrap_or(1);
        let _ = writeln!(
            text,
            "expected cost per card ({LINE_TYPICAL_ANSWER_TOKENS} answer tokens a line, thinking included; first answers / with a repair round on every line / --batch cached / --batch uncached), and for all {count} cards:"
        );
        let mut chosen = None;
        for model in PRICED {
            let Some(price) = price_of(model) else {
                continue;
            };
            let sum = cards.iter().fold(CardCost::default(), |sum, sizes| {
                let one = card_cost(&price, sizes, samples_n);
                CardCost {
                    first: sum.first.plus(one.first),
                    with_repair: sum.with_repair.plus(one.with_repair),
                    batch_warm: sum.batch_warm.plus(one.batch_warm),
                    batch_cold: sum.batch_cold.plus(one.batch_cold),
                }
            });
            let each = |usd: Usd| Usd(usd.0 / count.max(1));
            let worst = first_requests.first().map_or(Usd(0), |request| {
                price.worst_case(request, Billing::Standard)
            });
            let _ = writeln!(
                text,
                "  {model}: {} / {} / {} / {} a card; all cards {} / {} / {} / {}; worst case of one request {worst}",
                each(sum.first),
                each(sum.with_repair),
                each(sum.batch_warm),
                each(sum.batch_cold),
                sum.first,
                sum.with_repair,
                sum.batch_warm,
                sum.batch_cold,
            );
            if price_of(self.model).is_some_and(|own| own.id == price.id) {
                chosen = Some(each(sum.with_repair));
            }
        }
        if let (Some(cap), Some(per_card)) = (cap, chosen) {
            let _ = writeln!(
                text,
                "--max-spend {cap}: about {} cards on {} with a repair round on every line",
                cap.0 / per_card.0.max(1),
                self.model
            );
        }
        let _ = write!(text, "\n--- first card: {first_name} ---");
        for (line, request) in first_requests.iter().enumerate() {
            let _ = write!(
                text,
                "\n\n--- line {}: system ({} tokens, cached per kind) ---\n{}\n\n--- line {}: message ---\n{}",
                line + 1,
                estimate_tokens(request.system.len()),
                request.system,
                line + 1,
                request.messages.first().map_or("", |m| m.text.as_str())
            );
        }
        Ok(text)
    }
}

#[cfg(test)]
mod tests {
    //! Goal: a card's expected cost counts each line and sample at the model's price with the system prompt read from the
    //! cache, a repair round on top, a batch at half, and the skip reasons count without their details.
    use rstest::rstest;

    use super::*;

    #[rstest]
    #[case::opus("claude-opus-5-5")]
    #[case::sonnet("claude-sonnet-5-5")]
    #[case::haiku("claude-haiku-5-5")]
    fn a_line_costs_its_cached_system_its_message_and_its_answer(#[case] model: &str) {
        let price = price_of(model).unwrap();
        let size = LineSize {
            system: 1_000,
            message: 2_000,
        };
        let one = card_cost(&price, &[size], 1);
        let rate = price.rates;
        // 2000 new tokens, 1000 read at a tenth, 800 answered.
        let expected =
            (2_000 * rate.input + 100 * rate.input + 800 * rate.output).div_ceil(1_000_000);
        assert!(one.first.0.abs_diff(expected) <= 3, "{one:?} vs {expected}");
        assert!(one.with_repair > one.first);
        assert!(one.batch_warm.0.abs_diff(one.first.0 / 2) <= 2);
        assert!(one.batch_cold > one.batch_warm);
        let two = card_cost(&price, &[size, size], 3);
        assert!(two.first.0.abs_diff(one.first.0 * 6) <= 6);
    }

    #[rstest]
    #[case("no translation at /x/y.json", "no translation")]
    #[case("3 gapped lines", "more than two gapped lines")]
    #[case("not Commander-legal", "not Commander-legal")]
    fn skip_reasons_count_without_their_details(#[case] why: &str, #[case] key: &str) {
        assert_eq!(reason_key(why), key);
    }
}
