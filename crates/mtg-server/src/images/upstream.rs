//! Requests to the Scryfall CDN: polite spacing, retries and a check of what comes back.

use std::sync::OnceLock;
use std::time::Duration;

use reqwest::header::{ACCEPT, RETRY_AFTER};
use reqwest::{Client, StatusCode, Url, redirect};
use thiserror::Error;
use tokio::sync::{Mutex, MutexGuard};
use tokio::time::Instant;

use super::config::ImageConfig;
use super::link::ImageLink;

/// Largest picture accepted, in bytes: Scryfall's biggest, the PNG of a card, is about 1 MiB.
const MAX_PICTURE_BYTES: usize = 8 * 1024 * 1024;
/// The longest wait a `Retry-After` can impose, because a player is waiting for the picture.
const MAX_RETRY_WAIT: Duration = Duration::from_secs(20);
/// What a picture request accepts.
const ACCEPT_VALUE: &str = "image/jpeg,image/png;q=0.9,*/*;q=0.5";

/// Why a picture was not had from the CDN.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum FetchError {
    /// The HTTP client could not be built.
    #[error("the HTTP client could not be built: {0}")]
    Client(String),
    /// The CDN answered with a status other than 200.
    #[error("the CDN answered {0}")]
    Status(u16),
    /// The request failed on the way or timed out.
    #[error("the request failed: {0}")]
    Transport(String),
    /// The answer is not a picture of the format the link names.
    #[error("the answer is not a picture")]
    NotAPicture,
    /// The answer is larger than any picture.
    #[error("the answer is larger than a picture can be")]
    TooBig,
}

impl FetchError {
    /// Whether another try can help.
    fn retryable(&self) -> bool {
        match self {
            FetchError::Status(code) => matches!(*code, 408 | 425 | 429 | 500..=599),
            FetchError::Transport(_) => true,
            _ => false,
        }
    }
}

/// Hands out turns that start at least `gap` after the previous turn ended.
#[derive(Debug)]
pub(crate) struct Pacer {
    gap: Duration,
    last_end: Mutex<Option<Instant>>,
}

/// The right to make one request; the next turn is timed from the moment this one is dropped.
pub(crate) struct Turn<'a> {
    last_end: MutexGuard<'a, Option<Instant>>,
}

impl Pacer {
    pub(crate) fn new(gap: Duration) -> Pacer {
        Pacer {
            gap,
            last_end: Mutex::new(None),
        }
    }

    /// Waits for the gap to pass, then holds every other turn back until the result is dropped.
    pub(crate) async fn turn(&self) -> Turn<'_> {
        let last_end = self.last_end.lock().await;
        if let Some(end) = *last_end {
            tokio::time::sleep_until(end + self.gap).await;
        }
        Turn { last_end }
    }
}

impl Drop for Turn<'_> {
    fn drop(&mut self) {
        *self.last_end = Some(Instant::now());
    }
}

/// The CDN client: one per server, so the gap between requests is global.
#[derive(Debug)]
pub(crate) struct Upstream {
    client: OnceLock<Result<Client, FetchError>>,
    pacer: Pacer,
    base: Option<Url>,
    attempts: u32,
    backoff: Duration,
    timeout: Duration,
}

impl Upstream {
    pub(crate) fn new(config: &ImageConfig) -> Upstream {
        let base = config
            .upstream_base
            .as_deref()
            .and_then(|text| Url::parse(text).ok());
        Upstream {
            client: OnceLock::new(),
            pacer: Pacer::new(config.min_gap),
            base,
            attempts: config.attempts.max(1),
            backoff: config.backoff,
            timeout: config.timeout,
        }
    }

    /// The client, built on first use: Scryfall refuses requests without a descriptive agent.
    fn client(&self) -> Result<&Client, FetchError> {
        self.client
            .get_or_init(|| {
                Client::builder()
                    .user_agent(format!("mtg-cli/{}", env!("CARGO_PKG_VERSION")))
                    .timeout(self.timeout)
                    .connect_timeout(self.timeout)
                    // A stored link never redirects, and a redirect would leave the validated host.
                    .redirect(redirect::Policy::none())
                    .build()
                    .map_err(|error| FetchError::Client(error.to_string()))
            })
            .as_ref()
            .map_err(Clone::clone)
    }

    /// Where the request for `link` goes: the link itself, or its path on the injected base.
    fn target(&self, link: &ImageLink) -> Url {
        let Some(base) = &self.base else {
            return link.url().clone();
        };
        let mut url = base.clone();
        url.set_path(link.url().path());
        url.set_query(link.url().query());
        url
    }

    /// The picture behind `link`, tried again after 429, 408, 425, 5xx and failed connections.
    pub(crate) async fn fetch(&self, link: &ImageLink) -> Result<Vec<u8>, FetchError> {
        let mut wait = self.backoff;
        let mut attempt = 1;
        loop {
            let (outcome, retry_after) = self.once(link).await;
            match outcome {
                Err(error) if error.retryable() && attempt < self.attempts => {
                    let pause = retry_after.map_or(wait, |asked| asked.min(MAX_RETRY_WAIT));
                    tracing::debug!(%error, attempt, ?pause, key = link.key(), "picture request will be retried");
                    tokio::time::sleep(pause).await;
                    wait = wait.saturating_mul(2);
                    attempt += 1;
                }
                other => return other,
            }
        }
    }

    /// One request, and the wait the CDN asked for when it refused.
    async fn once(&self, link: &ImageLink) -> (Result<Vec<u8>, FetchError>, Option<Duration>) {
        let client = match self.client() {
            Ok(client) => client,
            Err(error) => return (Err(error), None),
        };
        let _turn = self.pacer.turn().await;
        let sent = client
            .get(self.target(link))
            .header(ACCEPT, ACCEPT_VALUE)
            .send()
            .await;
        let mut response = match sent {
            Ok(response) => response,
            Err(error) => return (Err(FetchError::Transport(error.to_string())), None),
        };
        let status = response.status();
        if status != StatusCode::OK {
            let asked = response
                .headers()
                .get(RETRY_AFTER)
                .and_then(|value| value.to_str().ok())
                .and_then(|text| text.trim().parse::<u64>().ok())
                .map(Duration::from_secs);
            return (Err(FetchError::Status(status.as_u16())), asked);
        }
        let mut body = Vec::new();
        loop {
            match response.chunk().await {
                Ok(Some(chunk)) => {
                    body.extend_from_slice(&chunk);
                    if body.len() > MAX_PICTURE_BYTES {
                        return (Err(FetchError::TooBig), None);
                    }
                }
                Ok(None) => break,
                Err(error) => return (Err(FetchError::Transport(error.to_string())), None),
            }
        }
        if !link.looks_like_picture(&body) {
            return (Err(FetchError::NotAPicture), None);
        }
        (Ok(body), None)
    }
}

#[cfg(test)]
mod tests {
    //! Goal: the pacer leaves the gap between turns however long they last and however many tasks ask, and only busy or broken answers are tried again.
    use rstest::rstest;

    use super::*;

    const GAP: Duration = Duration::from_millis(100);

    #[tokio::test(start_paused = true)]
    async fn the_first_turn_starts_at_once() {
        let started = Instant::now();
        let pacer = Pacer::new(GAP);
        let _turn = pacer.turn().await;
        assert_eq!(started.elapsed(), Duration::ZERO);
    }

    #[rstest]
    #[case::instant_requests(Duration::ZERO)]
    #[case::short_requests(Duration::from_millis(30))]
    #[case::requests_longer_than_the_gap(Duration::from_millis(450))]
    #[tokio::test(start_paused = true)]
    async fn consecutive_turns_are_a_gap_apart(#[case] busy: Duration) {
        let pacer = Pacer::new(GAP);
        let mut starts = Vec::new();
        let mut ends = Vec::new();
        for _ in 0..4 {
            let turn = pacer.turn().await;
            starts.push(Instant::now());
            tokio::time::sleep(busy).await;
            ends.push(Instant::now());
            drop(turn);
        }
        for pair in 1..starts.len() {
            assert!(
                starts[pair] - ends[pair - 1] >= GAP,
                "turn {pair} started too soon"
            );
        }
    }

    #[tokio::test(start_paused = true)]
    async fn tasks_asking_together_are_served_one_gap_apart() {
        let pacer = std::sync::Arc::new(Pacer::new(GAP));
        let tasks: Vec<_> = (0..5)
            .map(|_| {
                let pacer = pacer.clone();
                tokio::spawn(async move {
                    let _turn = pacer.turn().await;
                    Instant::now()
                })
            })
            .collect();
        let mut starts = Vec::new();
        for task in tasks {
            starts.push(task.await.unwrap());
        }
        starts.sort();
        for pair in starts.windows(2) {
            assert!(pair[1] - pair[0] >= GAP);
        }
    }

    #[rstest]
    #[case::too_many_requests(429, true)]
    #[case::server_error(500, true)]
    #[case::bad_gateway(502, true)]
    #[case::timeout(408, true)]
    #[case::too_early(425, true)]
    #[case::not_found(404, false)]
    #[case::forbidden(403, false)]
    #[case::redirect(301, false)]
    fn only_busy_and_broken_answers_are_tried_again(#[case] code: u16, #[case] retry: bool) {
        assert_eq!(FetchError::Status(code).retryable(), retry);
        assert!(FetchError::Transport("reset".into()).retryable());
        assert!(!FetchError::NotAPicture.retryable() && !FetchError::TooBig.retryable());
    }
}
