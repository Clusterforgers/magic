//! The hard budget of a run: requests and input tokens. A request is refused before it is sent when it would pass either.

use thiserror::Error;

use super::model::{Request, Usage};

/// Which budget ended the run.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum Stop {
    /// `--max-requests` requests were made.
    #[error("the budget of {0} requests is spent")]
    Requests(u32),
    /// The next request would pass `--max-input-tokens`.
    #[error("the budget of {0} input tokens would be passed")]
    InputTokens(u64),
}

/// What a run may spend and what it has spent.
#[derive(Debug, Clone)]
pub struct Budget {
    max_requests: u32,
    max_input_tokens: u64,
    requests: u32,
    repairs: u32,
    usage: Usage,
}

impl Budget {
    /// A budget of at most `max_requests` requests and `max_input_tokens` input tokens (cached tokens included).
    pub fn new(max_requests: u32, max_input_tokens: u64) -> Self {
        Self {
            max_requests,
            max_input_tokens,
            requests: 0,
            repairs: 0,
            usage: Usage::default(),
        }
    }

    /// Counts the request when it fits, else says which budget it would pass. The estimate of its tokens is an upper one,
    /// so what is spent never passes the budget.
    pub fn admit(&mut self, request: &Request, repair: bool) -> Result<(), Stop> {
        if self.requests >= self.max_requests {
            return Err(Stop::Requests(self.max_requests));
        }
        if self.usage.input_tokens + request.estimated_tokens() > self.max_input_tokens {
            return Err(Stop::InputTokens(self.max_input_tokens));
        }
        self.requests += 1;
        self.repairs += u32::from(repair);
        Ok(())
    }

    /// Adds what an answered request cost.
    pub fn record(&mut self, usage: Usage) {
        self.usage.input_tokens += usage.input_tokens;
        self.usage.cache_read_tokens += usage.cache_read_tokens;
        self.usage.output_tokens += usage.output_tokens;
    }

    /// The requests made.
    pub fn requests(&self) -> u32 {
        self.requests
    }

    /// The requests that were repair rounds.
    pub fn repairs(&self) -> u32 {
        self.repairs
    }

    /// The tokens spent.
    pub fn usage(&self) -> Usage {
        self.usage
    }

    /// The request budget.
    pub fn max_requests(&self) -> u32 {
        self.max_requests
    }

    /// The input token budget.
    pub fn max_input_tokens(&self) -> u64 {
        self.max_input_tokens
    }
}

#[cfg(test)]
mod tests {
    //! Goal: a request is counted when it fits, refused when it would pass the request budget or the input token budget,
    //! and what answered requests cost adds up.
    use super::*;
    use crate::propose::model::Message;

    fn request(bytes: usize) -> Request {
        Request {
            model: "m".into(),
            system: String::new(),
            messages: vec![Message::user("x".repeat(bytes))],
            max_tokens: 10,
        }
    }

    #[test]
    fn requests_stop_at_the_request_budget() {
        let mut budget = Budget::new(2, u64::MAX);
        assert!(budget.admit(&request(3), false).is_ok());
        assert!(budget.admit(&request(3), true).is_ok());
        assert_eq!(budget.admit(&request(3), false), Err(Stop::Requests(2)));
        assert_eq!((budget.requests(), budget.repairs()), (2, 1));
    }

    #[test]
    fn a_request_that_would_pass_the_token_budget_is_refused_by_its_estimate() {
        // 300 bytes estimate to 100 tokens.
        let mut budget = Budget::new(10, 150);
        assert!(budget.admit(&request(300), false).is_ok());
        budget.record(Usage {
            input_tokens: 80,
            cache_read_tokens: 0,
            output_tokens: 5,
        });
        assert_eq!(
            budget.admit(&request(300), false),
            Err(Stop::InputTokens(150))
        );
        assert_eq!(budget.requests(), 1);
        assert!(budget.admit(&request(150), false).is_ok());
    }

    #[test]
    fn usage_adds_up() {
        let mut budget = Budget::new(10, 1000);
        for _ in 0..3 {
            budget.record(Usage {
                input_tokens: 10,
                cache_read_tokens: 4,
                output_tokens: 2,
            });
        }
        assert_eq!(
            budget.usage(),
            Usage {
                input_tokens: 30,
                cache_read_tokens: 12,
                output_tokens: 6
            }
        );
    }
}
