//! The local server for line mode: the chat completions API with the JSON schema of the ability's kind as
//! `response_format`, one client for each kind.

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex, PoisonError};

use reqwest::header::HeaderMap;
use serde_json::{Value, json};
use tokio::runtime::Handle;

use super::LineModels;
use super::part::{self, Kind};
use crate::error::CliError;
use crate::progress::Progress;
use crate::propose::c11i_http::{ApiKey, Dialect, HttpClient, HttpConfig};
use crate::propose::c11i_openai::{
    OpenAi, c11i_key_from_env, c11i_parse_response, c11i_request_body,
};
use crate::propose::model::{ModelClient, ModelError, Request, Response};

/// The chat completions API asked for one ability of a schema.
#[derive(Debug)]
pub struct LineDialect {
    base: OpenAi,
    schema: Value,
}

impl LineDialect {
    /// The dialect for the schema of `kind`.
    pub fn new(base: OpenAi, kind: Kind) -> Self {
        Self {
            base,
            schema: part::schema(kind),
        }
    }
}

impl Dialect for LineDialect {
    fn path(&self) -> &'static str {
        self.base.path()
    }

    fn headers(&self, key: Option<&ApiKey>) -> Result<HeaderMap, ModelError> {
        self.base.headers(key)
    }

    fn body(&self, request: &Request) -> Value {
        let mut body = c11i_request_body(request, false, self.base.max_answer_tokens);
        body["response_format"] = json!({
            "type": "json_schema",
            "json_schema": {"name": "ability", "strict": true, "schema": self.schema},
        });
        if let Some(effort) = self.base.reasoning {
            body["reasoning_effort"] = json!(effort.as_str());
        }
        body
    }

    fn parse(&self, request: &Request, body: &str) -> Result<Response, ModelError> {
        c11i_parse_response(
            body,
            request.estimated_tokens(),
            self.base.max_answer_tokens,
        )
    }
}

/// The clients of a run against one server, made when a kind is first asked.
pub struct HttpModels {
    base: OpenAi,
    config: HttpConfig,
    handle: Handle,
    progress: Progress,
    made: Mutex<BTreeMap<Kind, Arc<dyn ModelClient>>>,
}

impl HttpModels {
    /// Clients that speak `base` to the server in `config`, on the runtime of `handle`.
    pub fn new(base: OpenAi, config: HttpConfig, handle: Handle, progress: Progress) -> Self {
        Self {
            base,
            config,
            handle,
            progress,
            made: Mutex::default(),
        }
    }
}

impl LineModels for HttpModels {
    fn client(&self, kind: Kind) -> Result<Arc<dyn ModelClient>, CliError> {
        let mut made = self.made.lock().unwrap_or_else(PoisonError::into_inner);
        if let Some(client) = made.get(&kind) {
            return Ok(Arc::clone(client));
        }
        let key = c11i_key_from_env(|name| std::env::var(name).ok());
        let dialect = LineDialect::new(self.base, kind);
        let client = HttpClient::c11i_build(
            dialect,
            key,
            &self.config,
            self.handle.clone(),
            self.progress.clone(),
        )?;
        let client: Arc<dyn ModelClient> = Arc::new(client);
        made.insert(kind, Arc::clone(&client));
        Ok(client)
    }
}
