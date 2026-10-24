//! A deterministic client for tests: canned answers by card name, no network, and a log of every request.

use std::collections::{BTreeMap, VecDeque};
use std::sync::Mutex;

use super::model::{ModelClient, ModelError, Request, Response, Usage};
use super::prompt::card_of;

/// Answers each card from a script: the first request about a card gets its first answer, the repair round its second.
#[derive(Debug, Default)]
pub struct ScriptedClient {
    script: Mutex<BTreeMap<String, VecDeque<Result<String, u16>>>>,
    log: Mutex<Vec<Request>>,
}

impl ScriptedClient {
    /// A client with no answers.
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds an answer for the card's next request.
    #[must_use]
    pub fn answer(self, card: &str, text: &str) -> Self {
        self.push(card, Ok(text.to_owned()));
        self
    }

    /// Makes the card's next request fail with an HTTP status.
    #[must_use]
    pub fn failure(self, card: &str, status: u16) -> Self {
        self.push(card, Err(status));
        self
    }

    fn push(&self, card: &str, item: Result<String, u16>) {
        if let Ok(mut script) = self.script.lock() {
            script.entry(card.to_owned()).or_default().push_back(item);
        }
    }

    /// Every request the client got, in order.
    pub fn requests(&self) -> Vec<Request> {
        self.log.lock().map(|log| log.clone()).unwrap_or_default()
    }
}

impl ModelClient for ScriptedClient {
    fn complete(&self, request: &Request) -> Result<Response, ModelError> {
        if let Ok(mut log) = self.log.lock() {
            log.push(request.clone());
        }
        let card = card_of(request).unwrap_or_default().to_owned();
        let next = self
            .script
            .lock()
            .ok()
            .and_then(|mut script| script.get_mut(&card)?.pop_front());
        match next {
            Some(Ok(text)) => Ok(Response {
                usage: Usage {
                    input_tokens: request.estimated_tokens(),
                    cache_read_tokens: 0,
                    output_tokens: u64::try_from(text.len().div_ceil(3)).unwrap_or(u64::MAX),
                },
                text,
            }),
            Some(Err(status)) => Err(ModelError::Status {
                status,
                message: "scripted failure".into(),
                retry_after: None,
            }),
            None => Err(ModelError::Unscripted(card)),
        }
    }
}
