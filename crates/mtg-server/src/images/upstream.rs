//! Requests to the Scryfall CDN: polite spacing, a cap on requests in flight, retries and a check of what comes back.

use std::sync::{Arc, OnceLock};
use std::time::Duration;

use reqwest::header::{ACCEPT, RETRY_AFTER};
use reqwest::{Client, StatusCode, Url, redirect};
use thiserror::Error;
use tokio::sync::{Mutex, OwnedSemaphorePermit, Semaphore};
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

/// Hands out turns: at most `slots` at once, each starting at least `gap` after the previous one started.
#[derive(Debug)]
pub(crate) struct Pacer {
    gap: Duration,
    slots: Arc<Semaphore>,
    last_start: Mutex<Option<Instant>>,
}

/// The right to make one request; its slot is free again once this is dropped.
pub(crate) struct Turn {
    _slot: Option<OwnedSemaphorePermit>,
}

impl Pacer {
    pub(crate) fn new(gap: Duration, slots: usize) -> Pacer {
        Pacer {
            gap,
            slots: Arc::new(Semaphore::new(slots.max(1))),
            last_start: Mutex::new(None),
        }
    }

    /// Waits for a free slot and for the gap since the last start, then starts a turn.
    pub(crate) async fn turn(&self) -> Turn {
        // The semaphore is never closed, so the error case cannot happen.
        let slot = self.slots.clone().acquire_owned().await.ok();
        let mut last_start = self.last_start.lock().await;
        if let Some(start) = *last_start {
            tokio::time::sleep_until(start + self.gap).await;
        }
        *last_start = Some(Instant::now());
        Turn { _slot: slot }
    }
}

/// The CDN client: one per server, so the spacing and the cap on requests in flight are global.
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
            pacer: Pacer::new(config.min_gap, config.in_flight),
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
    //! Goal: the pacer spaces the starts of turns by the gap and keeps at most its slots in flight however many tasks ask and however long each lasts, and only busy or broken answers are tried again.
    use rstest::rstest;

    use super::*;

    const GAP: Duration = Duration::from_millis(25);

    #[tokio::test(start_paused = true)]
    async fn the_first_turn_starts_at_once() {
        let started = Instant::now();
        let pacer = Pacer::new(GAP, 8);
        let _turn = pacer.turn().await;
        assert_eq!(started.elapsed(), Duration::ZERO);
    }

    /// Runs `tasks` turns that each last `busy` and returns each turn's start and end.
    async fn run(slots: usize, tasks: usize, busy: Duration) -> Vec<(Instant, Instant)> {
        let pacer = Arc::new(Pacer::new(GAP, slots));
        let spawned: Vec<_> = (0..tasks)
            .map(|_| {
                let pacer = pacer.clone();
                tokio::spawn(async move {
                    let _turn = pacer.turn().await;
                    let start = Instant::now();
                    tokio::time::sleep(busy).await;
                    (start, Instant::now())
                })
            })
            .collect();
        let mut spans = Vec::new();
        for task in spawned {
            spans.push(task.await.unwrap());
        }
        spans.sort();
        spans
    }

    #[rstest]
    #[case::one_slot_instant(1, Duration::ZERO)]
    #[case::one_slot_slow(1, Duration::from_millis(450))]
    #[case::eight_slots_instant(8, Duration::ZERO)]
    #[case::eight_slots_short(8, Duration::from_millis(30))]
    #[case::eight_slots_slow(8, Duration::from_millis(450))]
    #[case::three_slots_slow(3, Duration::from_millis(450))]
    #[tokio::test(start_paused = true)]
    async fn starts_are_a_gap_apart_and_never_more_than_the_slots_run(
        #[case] slots: usize,
        #[case] busy: Duration,
    ) {
        let spans = run(slots, 20, busy).await;
        for pair in spans.windows(2) {
            assert!(pair[1].0 - pair[0].0 >= GAP, "a turn started too soon");
        }
        for &(start, _) in &spans {
            let running = spans
                .iter()
                .filter(|(s, e)| *s <= start && start < *e)
                .count();
            assert!(running <= slots, "{running} turns ran at once");
        }
    }

    #[tokio::test(start_paused = true)]
    async fn slow_requests_overlap_so_many_pictures_take_a_fraction_of_the_serial_time() {
        let busy = Duration::from_millis(200);
        let first = Instant::now();
        let spans = run(8, 40, busy).await;
        let elapsed = spans.iter().map(|(_, end)| *end).max().unwrap() - first;
        // One at a time they would take 40 × 200 ms; eight at once take five rounds and the spacing.
        assert!(
            elapsed < Duration::from_millis(40 * 200 / 6),
            "took {elapsed:?}"
        );
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
