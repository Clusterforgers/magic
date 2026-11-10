//! The spend cap of a run: a request is refused before it is sent when what the ledger holds, plus the worst cases of the
//! requests that are out, plus its own worst case would pass `--max-spend`; what a request really cost goes into the
//! ledger as it is answered.

use super::budget::Stop;
use super::ledger::{Entry, Ledger};
use super::model::{Request, Usage};
use super::price::{Billing, Price, Usd, known_models, price_of};
use crate::error::CliError;

/// Why a request may not be sent.
#[derive(Debug)]
pub enum Refused {
    /// The cap would be passed.
    Cap(Stop),
    /// The ledger cannot be read or written, so the spend cannot be kept.
    Ledger(CliError),
}

/// The money side of a run.
#[derive(Debug, Clone)]
pub struct Spend {
    ledger: Ledger,
    cap: Usd,
    price: Price,
    model: String,
    billing: Billing,
    /// The worst cases of the requests that are out and not yet billed.
    in_flight: Usd,
    /// What this run has been billed.
    run: Usd,
}

impl Spend {
    /// A cap of `cap` over the ledger for the model; a model without a price is refused, because its worst case cannot be
    /// known.
    pub fn new(ledger: Ledger, cap: Usd, model: &str, billing: Billing) -> Result<Self, CliError> {
        let price = price_of(model).ok_or_else(|| {
            CliError::Config(format!(
                "no price is known for the model `{model}`, so --max-spend cannot be kept; priced models: {}",
                known_models().join(", ")
            ))
        })?;
        Ok(Self {
            ledger,
            cap,
            price,
            model: model.to_owned(),
            billing,
            in_flight: Usd(0),
            run: Usd(0),
        })
    }

    /// The cap.
    pub fn cap(&self) -> Usd {
        self.cap
    }

    /// The model's price.
    pub fn price(&self) -> Price {
        self.price
    }

    /// How requests are billed.
    pub fn billing(&self) -> Billing {
        self.billing
    }

    /// What this run has been billed.
    pub fn run(&self) -> Usd {
        self.run
    }

    /// What the ledger holds: the billed requests of every run and the batches still out.
    pub fn ledger_total(&self) -> Result<Usd, CliError> {
        Ok(Usd(u64::try_from(self.ledger.total()?).unwrap_or(0)))
    }

    /// The ledger.
    pub fn ledger(&self) -> &Ledger {
        &self.ledger
    }

    /// Checks that `extra` more would stay within the cap; the stop says what is committed already.
    pub fn check(&self, extra: Usd) -> Result<(), Refused> {
        let committed = self
            .ledger_total()
            .map_err(Refused::Ledger)?
            .plus(self.in_flight);
        if committed.plus(extra) > self.cap {
            return Err(Refused::Cap(Stop::Spend {
                cap: self.cap,
                spent: committed,
                next: extra,
            }));
        }
        Ok(())
    }

    /// The most the request can cost at this model's price.
    pub fn worst_case(&self, request: &Request) -> Usd {
        self.price.worst_case(request, self.billing)
    }

    /// Holds the worst case of a request that is about to be sent, or refuses it.
    pub fn reserve(&mut self, request: &Request) -> Result<Usd, Refused> {
        let worst = self.worst_case(request);
        self.check(worst)?;
        self.in_flight = self.in_flight.plus(worst);
        Ok(worst)
    }

    /// Gives back what a request held when it was not sent after all.
    pub fn release(&mut self, reserved: Usd) {
        self.in_flight = self.in_flight.minus(reserved);
    }

    /// Writes what a request cost to the ledger and returns it; the request's hold ends.
    pub fn bill(&mut self, reserved: Usd, card: &str, usage: Usage) -> Result<Usd, CliError> {
        self.release(reserved);
        let cost = self.price.cost(&usage, self.billing);
        self.ledger.append(&[Entry {
            card: card.to_owned(),
            model: self.model.clone(),
            usage,
            micros: signed(cost),
        }])?;
        self.run = self.run.plus(cost);
        Ok(cost)
    }

    /// Writes a request that got no answer at its worst case, because the API may have billed it; returns the amount.
    pub fn forfeit(&mut self, reserved: Usd, card: &str) -> Result<Usd, CliError> {
        self.release(reserved);
        self.ledger.append(&[Entry {
            card: format!("{card} (no answer, may be billed)"),
            model: self.model.clone(),
            usage: Usage::default(),
            micros: signed(reserved),
        }])?;
        self.run = self.run.plus(reserved);
        Ok(reserved)
    }

    /// Adds money billed outside [`Spend::bill`] (a batch's results) to this run's total.
    pub fn add_run(&mut self, cost: Usd) {
        self.run = self.run.plus(cost);
    }
}

/// The amount as a ledger number.
pub fn signed(amount: Usd) -> i64 {
    i64::try_from(amount.0).unwrap_or(i64::MAX)
}

#[cfg(test)]
mod tests {
    //! Goal: a request is refused when the ledger, the requests out and its own worst case would pass the cap, what a
    //! request cost is written to the ledger and counts for the next run, a model without a price is refused, and a
    //! request that got no answer is counted at its worst case.
    use super::*;
    use crate::propose::model::Message;

    fn request(system_bytes: usize, max_tokens: u32) -> Request {
        Request {
            model: "claude-sonnet-5-5".into(),
            system: "x".repeat(system_bytes),
            messages: vec![Message::user("Card: Opt")],
            max_tokens,
        }
    }

    fn spend(dir: &std::path::Path, cap: u64) -> Spend {
        Spend::new(
            Ledger::at(dir.join("l.tsv")),
            Usd(cap),
            "claude-sonnet-5-5",
            Billing::Standard,
        )
        .unwrap()
    }

    #[test]
    fn requests_out_and_the_ledger_count_against_the_cap() {
        let dir = tempfile::tempdir().unwrap();
        // 300 bytes are at most 100 tokens: a cache write of 100 tokens is $0.000250, 1000 answer tokens $0.01.
        let req = request(291, 1_000);
        let worst = spend(dir.path(), u64::MAX).worst_case(&req);
        assert_eq!(worst, Usd(250 + 10_000));
        let mut run = spend(dir.path(), worst.0 * 2 + 5);
        let first = run.reserve(&req).unwrap();
        let second = run.reserve(&req).unwrap();
        assert_eq!((first, second), (worst, worst));
        let Err(Refused::Cap(Stop::Spend { cap, spent, next })) = run.reserve(&req) else {
            panic!("the third request passes the cap");
        };
        assert_eq!(
            (cap.0, spent, next),
            (worst.0 * 2 + 5, Usd(worst.0 * 2), worst)
        );
        // A billed request frees its hold and leaves its real cost in the ledger.
        let cost = run
            .bill(
                first,
                "Opt",
                Usage {
                    input_tokens: 100,
                    cache_write_tokens: 0,
                    cache_read_tokens: 0,
                    output_tokens: 100,
                },
            )
            .unwrap();
        assert_eq!(cost, Usd(200 + 1_000));
        assert_eq!(run.ledger_total().unwrap(), Usd(1_200));
        // 1 200 billed and one request out leave room for 9 055 more.
        assert!(run.check(Usd(9_055)).is_ok());
        assert!(run.check(Usd(9_056)).is_err());
    }

    #[test]
    fn the_ledger_of_an_earlier_run_counts_toward_the_cap() {
        let dir = tempfile::tempdir().unwrap();
        let req = request(291, 1_000);
        let mut before = spend(dir.path(), 1_000_000);
        let held = before.reserve(&req).unwrap();
        before
            .bill(
                held,
                "Opt",
                Usage {
                    input_tokens: 100,
                    output_tokens: 99_000,
                    ..Usage::default()
                },
            )
            .unwrap();
        // 100 input tokens $0.0002 and 99 000 answer tokens $0.99.
        assert_eq!(before.ledger_total().unwrap(), Usd(990_200));
        let mut after = spend(dir.path(), 1_000_000);
        assert!(matches!(after.reserve(&req), Err(Refused::Cap(_))));
        let roomy = spend(dir.path(), 1_000_450);
        assert!(roomy.check(Usd(10_250)).is_ok());
    }

    #[test]
    fn a_request_without_an_answer_is_counted_at_its_worst_case() {
        let dir = tempfile::tempdir().unwrap();
        let req = request(291, 1_000);
        let mut run = spend(dir.path(), 1_000_000);
        let held = run.reserve(&req).unwrap();
        run.forfeit(held, "Opt").unwrap();
        assert_eq!(run.ledger_total().unwrap(), held);
        assert_eq!(run.run(), held);
        let text = std::fs::read_to_string(run.ledger().path()).unwrap();
        assert!(text.contains("Opt (no answer, may be billed)"), "{text}");
    }

    #[test]
    fn a_model_without_a_price_is_refused_and_a_batch_halves_the_worst_case() {
        let dir = tempfile::tempdir().unwrap();
        let error = Spend::new(
            Ledger::at(dir.path().join("l.tsv")),
            Usd(5),
            "gpt-oss:20b",
            Billing::Standard,
        )
        .unwrap_err()
        .to_string();
        assert!(
            error.contains("no price is known") && error.contains("claude-haiku-5-5"),
            "{error}"
        );
        let req = request(291, 1_000);
        let standard = spend(dir.path(), u64::MAX).worst_case(&req);
        let batch = Spend::new(
            Ledger::at(dir.path().join("b.tsv")),
            Usd(u64::MAX),
            "claude-sonnet-5-5",
            Billing::Batch,
        )
        .unwrap()
        .worst_case(&req);
        assert_eq!(batch, Usd(standard.0 / 2));
    }
}
