//! The model behind `propose`: a request, a response and the client trait; the HTTP client and the scripted one implement it.

use std::time::Duration;

use thiserror::Error;

/// The author of a message.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    /// The person asking.
    User,
    /// The model answering.
    Assistant,
}

/// One message of a conversation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Message {
    /// Who wrote it.
    pub role: Role,
    /// What it says.
    pub text: String,
}

impl Message {
    /// A message of the person.
    pub fn user(text: impl Into<String>) -> Self {
        Self {
            role: Role::User,
            text: text.into(),
        }
    }

    /// A message of the model.
    pub fn assistant(text: impl Into<String>) -> Self {
        Self {
            role: Role::Assistant,
            text: text.into(),
        }
    }
}

/// A request for one answer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Request {
    /// The model id.
    pub model: String,
    /// The instructions and the DSL reference; the same for every card, so the API may cache them.
    pub system: String,
    /// The conversation, beginning and ending with the person.
    pub messages: Vec<Message>,
    /// The most tokens the answer may have.
    pub max_tokens: u32,
}

impl Request {
    /// The text of the request in bytes.
    pub fn bytes(&self) -> usize {
        self.system.len() + self.messages.iter().map(|m| m.text.len()).sum::<usize>()
    }

    /// An upper estimate of the input tokens the request costs.
    pub fn estimated_tokens(&self) -> u64 {
        estimate_tokens(self.bytes())
    }
}

/// An upper estimate of the tokens of `bytes` bytes of prompt text (JSON and symbols take about 3 bytes a token).
pub fn estimate_tokens(bytes: usize) -> u64 {
    u64::try_from(bytes.div_ceil(3)).unwrap_or(u64::MAX)
}

/// What a request cost.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Usage {
    /// All input tokens: new, written to the cache and read from it.
    pub input_tokens: u64,
    /// The input tokens that were read from the cache.
    pub cache_read_tokens: u64,
    /// The tokens of the answer.
    pub output_tokens: u64,
}

/// An answer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Response {
    /// The text of the answer.
    pub text: String,
    /// What the request cost.
    pub usage: Usage,
}

/// Why a request got no answer.
#[derive(Debug, Error)]
pub enum ModelError {
    /// The environment has no API key.
    #[error("no API key: set ANTHROPIC_API_KEY")]
    MissingKey,
    /// The HTTP client could not be set up.
    #[error("cannot set up the HTTP client: {0}")]
    Client(String),
    /// The API refused or failed.
    #[error("the API answered {status}: {message}")]
    Status {
        /// The HTTP status.
        status: u16,
        /// The API's own words, or the start of the body.
        message: String,
        /// How long the API asked to wait.
        retry_after: Option<Duration>,
    },
    /// The connection or the body failed.
    #[error("cannot reach the API: {0}")]
    Transport(String),
    /// The server cannot be reached at all: nothing listens at the address, or the name does not resolve.
    #[error("cannot reach the model server at {endpoint}: {detail}; is it running?")]
    Unreachable {
        /// The address asked.
        endpoint: String,
        /// The operating system's complaint.
        detail: String,
    },
    /// The server read far fewer prompt tokens than were sent: it cut the prompt to fit its context window.
    #[error(
        "the server read {read} prompt tokens of an estimated {sent}, so it cut the prompt to its context window; \
         raise the context to at least 32768 tokens (`OLLAMA_CONTEXT_LENGTH=32768 ollama serve`, `llama-server -c 32768`)"
    )]
    Truncated {
        /// The prompt tokens the server reports.
        read: u64,
        /// The upper estimate of the tokens sent.
        sent: u64,
    },
    /// The answer is not what the API documents.
    #[error("unexpected answer from the API: {0}")]
    Answer(String),
    /// A scripted client has no answer for the card.
    #[error("no scripted answer: {0}")]
    Unscripted(String),
}

impl ModelError {
    /// Whether asking again can help: overload, rate limits, server errors and dropped connections.
    pub fn is_retryable(&self) -> bool {
        match self {
            ModelError::Status { status, .. } => {
                matches!(status, 408 | 409 | 425 | 429 | 500..=599)
            }
            ModelError::Transport(_) | ModelError::Unreachable { .. } => true,
            _ => false,
        }
    }

    /// Whether every further request will fail the same way: a key or model the API does not accept, a server that cannot
    /// be reached after the retries, or one that cuts the prompt.
    pub fn is_fatal(&self) -> bool {
        match self {
            ModelError::Status { status, .. } => matches!(status, 400 | 401 | 403 | 404),
            ModelError::MissingKey
            | ModelError::Client(_)
            | ModelError::Unreachable { .. }
            | ModelError::Truncated { .. } => true,
            _ => false,
        }
    }

    /// The wait the API asked for.
    pub fn retry_after(&self) -> Option<Duration> {
        match self {
            ModelError::Status { retry_after, .. } => *retry_after,
            _ => None,
        }
    }
}

/// Something that answers requests. `propose` asks for one answer at a time.
pub trait ModelClient {
    /// Sends the request and returns the model's answer.
    fn complete(&self, request: &Request) -> Result<Response, ModelError>;
}
