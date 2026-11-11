//! Line mode on the Anthropic API: a client for each kind of ability (with `--schema`, the kind's schema as structured
//! output when its types are not recursive), and the meter that holds each request's worst case against the spend cap
//! and writes what it cost to the ledger.

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex, PoisonError};

use reqwest::header::HeaderMap;
use serde_json::{Value, json};
use tokio::runtime::Handle;

use super::LineModels;
use super::part::{self, Kind};
use crate::error::CliError;
use crate::progress::Progress;
use crate::propose::anthropic::{Anthropic, AnthropicConfig, parse_response, request_body_with};
use crate::propose::budget::Stop;
use crate::propose::c11i_http::{ApiKey, Dialect, HttpClient};
use crate::propose::model::{ModelClient, ModelError, Request, Response};
use crate::propose::price::Usd;
use crate::propose::spend::{Refused, Spend};

/// The body of a line request: the Messages API body, with the answer's schema as `output_config.format` when given.
pub fn line_body(request: &Request, effort: Option<&str>, format: Option<&Value>) -> Value {
    let mut body = request_body_with(request, effort);
    if let Some(schema) = format {
        body["output_config"]["format"] = json!({"type": "json_schema", "schema": schema});
    }
    body
}

/// The schema a request about `kind` constrains its answer to: only with `--schema`, and only when the API takes it.
pub fn format_of(kind: Kind, schema: bool) -> Option<Value> {
    if schema { part::structured(kind) } else { None }
}

/// The Messages API asked for one ability of a kind.
#[derive(Debug)]
pub struct AnthropicLine {
    base: Anthropic,
    format: Option<Value>,
}

impl Dialect for AnthropicLine {
    fn path(&self) -> &'static str {
        self.base.path()
    }

    fn headers(&self, key: Option<&ApiKey>) -> Result<HeaderMap, ModelError> {
        self.base.headers(key)
    }

    fn body(&self, request: &Request) -> Value {
        line_body(request, self.base.effort, self.format.as_ref())
    }

    fn parse(&self, _request: &Request, body: &str) -> Result<Response, ModelError> {
        parse_response(body)
    }
}

/// The clients of a paid run, made when a kind is first asked.
pub struct PaidModels {
    key: ApiKey,
    config: AnthropicConfig,
    handle: Handle,
    progress: Progress,
    base: Anthropic,
    schema: bool,
    made: Mutex<BTreeMap<Kind, Arc<dyn ModelClient>>>,
}

impl PaidModels {
    /// Clients with `key` against `config`; `base` holds the effort and the workspace, `schema` asks for structured
    /// outputs.
    pub fn new(
        key: ApiKey,
        config: AnthropicConfig,
        handle: Handle,
        progress: Progress,
        base: Anthropic,
        schema: bool,
    ) -> Self {
        Self {
            key,
            config,
            handle,
            progress,
            base,
            schema,
            made: Mutex::default(),
        }
    }
}

impl LineModels for PaidModels {
    fn client(&self, kind: Kind) -> Result<Arc<dyn ModelClient>, CliError> {
        let mut made = self.made.lock().unwrap_or_else(PoisonError::into_inner);
        if let Some(client) = made.get(&kind) {
            return Ok(Arc::clone(client));
        }
        let dialect = AnthropicLine {
            base: self.base.clone(),
            format: format_of(kind, self.schema),
        };
        let client = HttpClient::c11i_build(
            dialect,
            Some(self.key.clone()),
            &self.config,
            self.handle.clone(),
            self.progress.clone(),
        )?;
        let client: Arc<dyn ModelClient> = Arc::new(client);
        made.insert(kind, Arc::clone(&client));
        Ok(client)
    }
}

/// What became of one request.
#[derive(Debug)]
pub enum Sent {
    /// The answer's text and what it cost.
    Answer(String, Usd),
    /// No usable answer: why, and what it may have cost.
    Unanswered(String, Usd),
    /// The spend cap refused the request; nothing was sent.
    Stop(Stop),
}

/// The money side of a line run: none for a local server.
#[derive(Debug, Default)]
pub struct Meter {
    /// The cap and the ledger of a paid run.
    pub spend: Option<Spend>,
}

impl Meter {
    /// Sends `request` when the cap allows its worst case, and writes what it cost to the ledger under `card`; an error
    /// no card can recover from (a refused key, an unknown model) ends the run.
    pub fn send(
        &mut self,
        client: &dyn ModelClient,
        request: &Request,
        card: &str,
    ) -> Result<Sent, CliError> {
        let held = match self.spend.as_mut().map(|spend| spend.reserve(request)) {
            None => None,
            Some(Ok(held)) => Some(held),
            Some(Err(Refused::Cap(stop))) => return Ok(Sent::Stop(stop)),
            Some(Err(Refused::Ledger(error))) => return Err(error),
        };
        let result = client.complete(request);
        let usage = match &result {
            Ok(response) => Some(response.usage),
            Err(error) => error.billed(),
        };
        let mut cost = Usd(0);
        if let (Some(spend), Some(held)) = (self.spend.as_mut(), held) {
            cost = match (&result, usage) {
                (_, Some(usage)) => spend.bill(held, card, usage)?,
                (Err(error), None) if error.may_be_billed() => spend.forfeit(held, card)?,
                _ => {
                    spend.release(held);
                    Usd(0)
                }
            };
        }
        match result {
            Ok(response) => Ok(Sent::Answer(response.text, cost)),
            Err(error) if error.is_fatal() => Err(CliError::Model(error)),
            Err(error) => Ok(Sent::Unanswered(error.to_string(), cost)),
        }
    }
}
