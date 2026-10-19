//! The HTTP client: polite spacing, retries, conditional requests and resumable downloads.

use std::future::Future;
use std::path::Path;
use std::time::Duration;

use reqwest::header::{
    ACCEPT, ACCEPT_ENCODING, CONTENT_RANGE, ETAG, HeaderMap, HeaderValue, IF_MODIFIED_SINCE,
    IF_NONE_MATCH, LAST_MODIFIED, RANGE, RETRY_AFTER,
};
use reqwest::{Client, StatusCode, Url};
use serde::{Deserialize, Serialize};
use tokio::fs::{self, File, OpenOptions};
use tokio::io::AsyncWriteExt;

use crate::error::FetchError;
use crate::pacer::Pacer;
use crate::progress::Progress;

/// The smallest gap between two requests; Scryfall asks for 50 to 100 ms.
pub const MIN_GAP: Duration = Duration::from_millis(100);

/// The `Accept` header Scryfall requires on every request.
const ACCEPT_VALUE: &str = "application/json;q=0.9,*/*;q=0.8";

/// The longest wait a server's `Retry-After` can impose on us.
const MAX_RETRY_WAIT: Duration = Duration::from_secs(60);

/// How the client behaves; the defaults suit the real sources, tests tune them.
#[derive(Debug, Clone)]
pub struct HttpConfig {
    /// Sent as `User-Agent`; Scryfall refuses requests without a descriptive one.
    pub user_agent: String,
    /// Time left between the end of one request and the start of the next.
    pub min_gap: Duration,
    /// How long a connection may take to open; `None` waits for ever.
    pub connect_timeout: Option<Duration>,
    /// How long the server may stay silent mid-answer; `None` waits for ever.
    pub read_timeout: Option<Duration>,
    /// Tries per request, the first included.
    pub attempts: u32,
    /// Wait before the second try; it doubles for each further one.
    pub backoff: Duration,
}

impl Default for HttpConfig {
    fn default() -> Self {
        Self {
            user_agent: format!("mtg-cli/{}", env!("CARGO_PKG_VERSION")),
            min_gap: MIN_GAP,
            connect_timeout: Some(Duration::from_secs(15)),
            read_timeout: Some(Duration::from_secs(60)),
            attempts: 3,
            backoff: Duration::from_secs(2),
        }
    }
}

/// What a cached copy was served with, so the server can tell us it has not changed.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Validators {
    /// The `ETag` of the copy.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub etag: Option<String>,
    /// The `Last-Modified` time of the copy.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_modified: Option<String>,
}

impl Validators {
    fn from_headers(headers: &HeaderMap) -> Self {
        let text = |name| {
            headers
                .get(name)
                .and_then(|v| v.to_str().ok())
                .map(str::to_owned)
        };
        Self {
            etag: text(ETAG),
            last_modified: text(LAST_MODIFIED),
        }
    }

    /// Whether there is anything to ask the server about.
    pub fn is_empty(&self) -> bool {
        self.etag.is_none() && self.last_modified.is_none()
    }
}

/// The answer to a plain request.
#[derive(Debug)]
pub enum Reply {
    /// A fresh copy, with the validators to keep with it.
    Modified {
        /// The decoded body.
        body: Vec<u8>,
        /// What the server says about this copy.
        validators: Validators,
    },
    /// The cached copy is still current.
    NotModified,
}

/// A finished download into a partial file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Downloaded {
    /// Length of the file now.
    pub size: u64,
    /// Bytes that were already there and were kept, 0 for a fresh start.
    pub resumed_from: u64,
}

/// An HTTP client that spaces its requests and retries the ones that can be retried.
#[derive(Debug)]
pub struct Http {
    client: Client,
    pacer: Pacer,
    attempts: u32,
    backoff: Duration,
    progress: Progress,
}

impl Http {
    /// Builds a client from `config`; retries and delays are announced on `progress`.
    pub fn new(config: &HttpConfig, progress: Progress) -> Result<Self, FetchError> {
        let mut headers = HeaderMap::new();
        headers.insert(ACCEPT, HeaderValue::from_static(ACCEPT_VALUE));
        let mut builder = Client::builder()
            .user_agent(config.user_agent.as_str())
            .default_headers(headers)
            // One connection per request: requests are minutes apart or a gap apart, never concurrent.
            .pool_max_idle_per_host(0);
        if let Some(limit) = config.connect_timeout {
            builder = builder.connect_timeout(limit);
        }
        if let Some(limit) = config.read_timeout {
            builder = builder.read_timeout(limit);
        }
        Ok(Self {
            client: builder.build().map_err(FetchError::Client)?,
            pacer: Pacer::new(config.min_gap),
            attempts: config.attempts.max(1),
            backoff: config.backoff,
            progress,
        })
    }

    /// Fetches a small document into memory; `cached` makes the request conditional.
    pub async fn get(&self, url: &Url, cached: Option<&Validators>) -> Result<Reply, FetchError> {
        let cached = cached.filter(|validators| !validators.is_empty());
        self.with_retries(url, move || async move {
            let mut request = self.client.get(url.clone());
            if let Some(validators) = cached {
                request = match (&validators.etag, &validators.last_modified) {
                    (Some(etag), _) => request.header(IF_NONE_MATCH, etag),
                    (None, Some(since)) => request.header(IF_MODIFIED_SINCE, since),
                    (None, None) => request,
                };
            }
            let response = request.send().await.map_err(|e| transport(url, e))?;
            let status = response.status();
            if status == StatusCode::NOT_MODIFIED && cached.is_some() {
                return Ok(Reply::NotModified);
            }
            if !status.is_success() {
                return Err(status_error(url, status, response.headers()));
            }
            let validators = Validators::from_headers(response.headers());
            let body = response.bytes().await.map_err(|e| transport(url, e))?;
            Ok(Reply::Modified {
                body: body.to_vec(),
                validators,
            })
        })
        .await
    }

    /// Downloads `url` into the partial file `part`, resuming what is already there.
    ///
    /// A server that cannot resume sends the whole file again, which replaces the partial one. A
    /// file that ends up with a length other than `expected` is deleted and fetched again.
    pub async fn download(
        &self,
        url: &Url,
        part: &Path,
        expected: Option<u64>,
    ) -> Result<Downloaded, FetchError> {
        self.with_retries(url, move || self.download_once(url, part, expected))
            .await
    }

    async fn download_once(
        &self,
        url: &Url,
        part: &Path,
        expected: Option<u64>,
    ) -> Result<Downloaded, FetchError> {
        let have = length_of(part).await?;
        // The stored representation is what ranges count in, so no content coding on the way.
        let mut request = self
            .client
            .get(url.clone())
            .header(ACCEPT_ENCODING, "identity");
        if have > 0 {
            request = request.header(RANGE, format!("bytes={have}-"));
        }
        let mut response = request.send().await.map_err(|e| transport(url, e))?;
        let status = response.status();

        let kept = match status {
            StatusCode::PARTIAL_CONTENT => match content_range(response.headers()) {
                Some(ContentRange {
                    start: Some(start), ..
                }) if start == have => have,
                _ => {
                    discard(part).await?;
                    return Err(corrupt(url, "the server resumed at a different offset"));
                }
            },
            StatusCode::OK => 0,
            StatusCode::RANGE_NOT_SATISFIABLE => {
                let total = content_range(response.headers()).and_then(|range| range.total);
                if have > 0 && total == Some(have) {
                    return check_length(url, part, have, have, expected).await;
                }
                discard(part).await?;
                return Err(corrupt(
                    url,
                    "the partial file does not fit the remote file",
                ));
            }
            _ => return Err(status_error(url, status, response.headers())),
        };

        let mut file = if kept == 0 {
            File::create(part).await
        } else {
            OpenOptions::new().append(true).open(part).await
        }
        .map_err(|e| FetchError::io(part, e))?;
        let mut size = kept;
        let received = loop {
            match response.chunk().await {
                Ok(Some(chunk)) => {
                    if let Err(error) = file.write_all(&chunk).await {
                        break Err(FetchError::io(part, error));
                    }
                    size += u64::try_from(chunk.len()).unwrap_or(0);
                }
                Ok(None) => break Ok(()),
                Err(error) => break Err(transport(url, error)),
            }
        };
        // What arrived stays on disk even when the body broke off, so the next try can resume it.
        let flushed = match file.flush().await {
            Ok(()) => file.sync_all().await,
            Err(error) => Err(error),
        };
        received?;
        flushed.map_err(|e| FetchError::io(part, e))?;
        check_length(url, part, size, kept, expected).await
    }

    /// Runs one paced attempt after another until one works or the failures stop being retryable.
    async fn with_retries<T, Fut>(
        &self,
        url: &Url,
        mut attempt_once: impl FnMut() -> Fut,
    ) -> Result<T, FetchError>
    where
        Fut: Future<Output = Result<T, FetchError>>,
    {
        let mut attempt = 1;
        loop {
            let result = {
                let _turn = self.pacer.turn().await;
                attempt_once().await
            };
            match result {
                Err(error) if attempt < self.attempts && error.is_retryable() => {
                    let wait = error
                        .retry_after()
                        .unwrap_or_else(|| self.backoff.saturating_mul(1 << (attempt - 1).min(10)))
                        .min(MAX_RETRY_WAIT);
                    self.progress.line(format_args!(
                        "retrying {url} in {:.1} s after attempt {attempt} of {}: {error}",
                        wait.as_secs_f64(),
                        self.attempts
                    ));
                    tokio::time::sleep(wait).await;
                    attempt += 1;
                }
                other => return other,
            }
        }
    }
}

async fn length_of(path: &Path) -> Result<u64, FetchError> {
    match fs::metadata(path).await {
        Ok(metadata) => Ok(metadata.len()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(0),
        Err(error) => Err(FetchError::io(path, error)),
    }
}

async fn discard(path: &Path) -> Result<(), FetchError> {
    match fs::remove_file(path).await {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(FetchError::io(path, error)),
    }
}

/// Accepts a file of the expected length; any other length means a broken download.
async fn check_length(
    url: &Url,
    part: &Path,
    size: u64,
    kept: u64,
    expected: Option<u64>,
) -> Result<Downloaded, FetchError> {
    match expected {
        Some(expected) if expected != size => {
            discard(part).await?;
            Err(corrupt(
                url,
                format!("{size} bytes arrived but {expected} were announced"),
            ))
        }
        _ => Ok(Downloaded {
            size,
            resumed_from: kept,
        }),
    }
}

fn transport(url: &Url, source: reqwest::Error) -> FetchError {
    FetchError::Transport {
        url: url.to_string(),
        source,
    }
}

fn corrupt(url: &Url, reason: impl Into<String>) -> FetchError {
    FetchError::Corrupt {
        url: url.to_string(),
        reason: reason.into(),
    }
}

fn status_error(url: &Url, status: StatusCode, headers: &HeaderMap) -> FetchError {
    if status == StatusCode::NOT_FOUND {
        return FetchError::NotFound {
            url: url.to_string(),
        };
    }
    FetchError::Status {
        url: url.to_string(),
        status: status.as_u16(),
        retry_after: headers
            .get(RETRY_AFTER)
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.trim().parse::<u64>().ok())
            .map(Duration::from_secs),
    }
}

/// A parsed `Content-Range` header: `bytes 100-199/200` or, for a refused range, `bytes */200`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ContentRange {
    /// First byte of the part, absent when the range was refused.
    pub start: Option<u64>,
    /// Length of the whole file, absent when the server does not know it.
    pub total: Option<u64>,
}

/// Reads the `Content-Range` header of a response.
pub fn content_range(headers: &HeaderMap) -> Option<ContentRange> {
    parse_content_range(headers.get(CONTENT_RANGE)?.to_str().ok()?)
}

/// Parses the value of a `Content-Range` header.
pub fn parse_content_range(value: &str) -> Option<ContentRange> {
    let (range, total) = value.trim().strip_prefix("bytes ")?.split_once('/')?;
    let total = match total.trim() {
        "*" => None,
        digits => Some(digits.parse().ok()?),
    };
    let start = match range.trim() {
        "*" => None,
        span => {
            let (first, last) = span.split_once('-')?;
            let (first, last): (u64, u64) = (first.parse().ok()?, last.parse().ok()?);
            (first <= last).then_some(first)?;
            Some(first)
        }
    };
    Some(ContentRange { start, total })
}

#[cfg(test)]
mod tests {
    //! Goal: `Content-Range` values parse into the offset and total a resume needs, anything malformed is rejected, and only failures worth repeating are retried.
    use rstest::rstest;

    use super::*;

    #[rstest]
    #[case("bytes 100-199/200", Some((Some(100), Some(200))))]
    #[case("bytes 0-0/1", Some((Some(0), Some(1))))]
    #[case("bytes 5-9/*", Some((Some(5), None)))]
    #[case("bytes */200", Some((None, Some(200))))]
    #[case(" bytes 7-8/9 ", Some((Some(7), Some(9))))]
    #[case("bytes 9-5/20", None)]
    #[case("bytes a-b/20", None)]
    #[case("bytes 1-2", None)]
    #[case("items 1-2/3", None)]
    #[case("", None)]
    fn content_ranges_parse(
        #[case] value: &str,
        #[case] expected: Option<(Option<u64>, Option<u64>)>,
    ) {
        let parsed = parse_content_range(value).map(|range| (range.start, range.total));
        assert_eq!(parsed, expected);
    }

    fn status(code: u16) -> FetchError {
        FetchError::Status {
            url: "http://x/".into(),
            status: code,
            retry_after: None,
        }
    }

    #[rstest]
    #[case(status(429), true)]
    #[case(status(500), true)]
    #[case(status(503), true)]
    #[case(status(408), true)]
    #[case(status(403), false)]
    #[case(status(400), false)]
    #[case(FetchError::NotFound { url: "x".into() }, false)]
    #[case(FetchError::Corrupt { url: "x".into(), reason: "r".into() }, true)]
    #[case(FetchError::Invalid { url: "x".into(), reason: "r".into() }, false)]
    fn only_failures_worth_repeating_are_retried(#[case] error: FetchError, #[case] retry: bool) {
        assert_eq!(error.is_retryable(), retry);
    }

    #[test]
    fn a_retry_after_header_is_read_in_seconds() {
        let mut headers = HeaderMap::new();
        headers.insert(RETRY_AFTER, HeaderValue::from_static("7"));
        let url = Url::parse("http://x/").unwrap();
        let error = status_error(&url, StatusCode::TOO_MANY_REQUESTS, &headers);
        assert_eq!(error.retry_after(), Some(Duration::from_secs(7)));
        headers.insert(
            RETRY_AFTER,
            HeaderValue::from_static("Wed, 21 Oct 2026 07:28:00 GMT"),
        );
        assert_eq!(
            status_error(&url, StatusCode::TOO_MANY_REQUESTS, &headers).retry_after(),
            None
        );
    }
}
