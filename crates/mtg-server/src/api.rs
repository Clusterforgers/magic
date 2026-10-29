//! The REST endpoints: create a game, look one up, list the demo decks, health.

use axum::Json;
use axum::extract::rejection::PathRejection;
use axum::extract::{Path, Request, State};
use axum::http::header::CONTENT_TYPE;
use axum::http::{HeaderMap, HeaderName, HeaderValue, Method, StatusCode};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use mtg_core::house::{HouseRuleError, HouseRules};
use mtg_core::ids::PlayerId;
use mtg_protocol::rest::{GameInfo, GameSeat};
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::deck_choice::{self, Refused};
use crate::demo::{self, DemoDeck};
use crate::game_id::GameId;
use crate::origin::origin_allowed;
use crate::recipe::FormatKind;
use crate::records::Visibility;
use crate::registry::{ChosenDeck, CreateError, GameOptions};
use crate::server::AppState;

/// Largest request body, in bytes.
pub const BODY_LIMIT: usize = 16 * 1024;
/// Fewest seats in a game.
pub const MIN_SEATS: usize = 2;
/// Largest body of `POST /api/games`, in bytes. It is the one ceiling on a game's seats: a seat is a
/// few dozen bytes of the request, and nothing else bounds their number (a seat is a `u64` index).
pub const GAME_BODY_LIMIT: usize = 256 * 1024;
/// Longest seat name, in characters.
pub const MAX_NAME_CHARS: usize = 32;
/// Longest part of a parser message echoed to the client, in characters.
const MAX_DETAIL_CHARS: usize = 120;

/// A failed request: a status and a short machine-readable code plus message, and any headers the
/// answer needs (`Retry-After`, a cookie to clear).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ApiError {
    status: StatusCode,
    code: &'static str,
    message: String,
    /// Kept as a list, not a map, so the error stays small enough to return by value.
    headers: Vec<(HeaderName, HeaderValue)>,
    /// Members of the body besides `error` and `message`.
    fields: Vec<(&'static str, serde_json::Value)>,
}

impl ApiError {
    pub(crate) fn new(
        status: StatusCode,
        code: &'static str,
        message: impl Into<String>,
    ) -> ApiError {
        ApiError {
            status,
            code,
            message: message.into(),
            headers: Vec::new(),
            fields: Vec::new(),
        }
    }

    /// The same error with one more header on its answer.
    pub(crate) fn with_header(mut self, name: HeaderName, value: HeaderValue) -> ApiError {
        self.headers.push((name, value));
        self
    }

    /// The same error with one more member in the body of its answer.
    pub(crate) fn with_field(mut self, name: &'static str, value: serde_json::Value) -> ApiError {
        self.fields.push((name, value));
        self
    }

    #[cfg(test)]
    pub(crate) fn field(&self, name: &str) -> Option<&serde_json::Value> {
        self.fields
            .iter()
            .find(|(field, _)| *field == name)
            .map(|(_, value)| value)
    }

    #[cfg(test)]
    pub(crate) fn status(&self) -> StatusCode {
        self.status
    }

    #[cfg(test)]
    pub(crate) fn code(&self) -> &'static str {
        self.code
    }

    #[cfg(test)]
    pub(crate) fn message(&self) -> &str {
        &self.message
    }

    pub(crate) fn invalid(message: impl Into<String>) -> ApiError {
        ApiError::new(StatusCode::BAD_REQUEST, "invalid_request", message)
    }

    pub(crate) fn not_found() -> ApiError {
        ApiError::new(StatusCode::NOT_FOUND, "not_found", "no such resource")
    }

    pub(crate) fn unavailable() -> ApiError {
        ApiError::new(
            StatusCode::SERVICE_UNAVAILABLE,
            "unavailable",
            "the server is shutting down",
        )
    }

    pub(crate) fn forbidden_origin() -> ApiError {
        ApiError::new(
            StatusCode::FORBIDDEN,
            "forbidden_origin",
            "origin not allowed",
        )
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let mut body = json!({ "error": self.code, "message": self.message });
        if let Some(members) = body.as_object_mut() {
            members.extend(
                self.fields
                    .into_iter()
                    .map(|(name, value)| (name.to_owned(), value)),
            );
        }
        let mut response = (self.status, Json(body)).into_response();
        for (name, value) in self.headers {
            response.headers_mut().append(name, value);
        }
        response
    }
}

/// Refuses a request that changes something (any method but `GET`, `HEAD` and `OPTIONS`) when it
/// comes from another site. Browsers always say where a page they ran came from, so this keeps a
/// stranger's page from acting with a visitor's cookie; a request with no `Origin` is not from a
/// browser and passes. Same rule, same list as the game socket's.
pub(crate) async fn same_site_only(
    State(app): State<AppState>,
    request: Request,
    next: Next,
) -> Response {
    let safe = matches!(
        *request.method(),
        Method::GET | Method::HEAD | Method::OPTIONS
    );
    if !safe && !origin_allowed(request.headers(), &app.config().allowed_origins) {
        return ApiError::forbidden_origin().into_response();
    }
    next.run(request).await
}

impl From<HouseRuleError> for ApiError {
    /// A 400 whose `message` starts with `house_rules.<rule>` and whose `field` is the rule's name.
    fn from(error: HouseRuleError) -> ApiError {
        ApiError::invalid(format!("house_rules.{error}")).with_field("field", json!(error.field()))
    }
}

impl From<CreateError> for ApiError {
    fn from(error: CreateError) -> ApiError {
        match error {
            CreateError::Full => ApiError::new(
                StatusCode::SERVICE_UNAVAILABLE,
                "server_full",
                "the server is running its maximum number of games; try again later",
            ),
            CreateError::Record(_) => {
                tracing::error!(%error, "game creation failed");
                ApiError::new(
                    StatusCode::SERVICE_UNAVAILABLE,
                    "unavailable",
                    "the game could not be saved; try again later",
                )
            }
            // Details stay in the log: they describe the server, not the request.
            CreateError::Random(_)
            | CreateError::Start(_)
            | CreateError::Recipe(_)
            | CreateError::Cards(_) => {
                tracing::error!(%error, "game creation failed");
                ApiError::new(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "internal",
                    "the game could not be created",
                )
            }
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct NewGame {
    format: String,
    seats: Vec<NewSeat>,
    /// Who may find and join the game; invite-only when absent.
    #[serde(default)]
    visibility: Visibility,
    /// How many seats, from seat 0, are handed out with a token; every seat when absent. The
    /// rest are open for a join to take.
    #[serde(default)]
    held: Option<usize>,
    /// What the creator sets in place of the format's values; the format's own when absent.
    #[serde(default)]
    house_rules: HouseRules,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct NewSeat {
    /// Judged for held seats only: an open seat is named by whoever takes it.
    #[serde(default)]
    name: String,
    /// The deck the seat starts with: a built-in deck's id, `precon:<code>`, or, for the creator's
    /// seat, the id of one of their stored decks. The seat's holder may change it before the game starts; a seat
    /// made without one has none until its holder chooses.
    #[serde(default)]
    deck: Option<String>,
}

#[derive(Debug, Serialize)]
pub(crate) struct CreatedBody {
    game: String,
    /// The join code, shown here and never again.
    code: String,
    /// The seats that are held, each with its token; open seats have none.
    seats: Vec<CreatedSeat>,
}

#[derive(Debug, Serialize)]
struct CreatedSeat {
    seat: PlayerId,
    token: String,
}

#[derive(Debug, Serialize)]
pub(crate) struct DeckBody {
    id: &'static str,
    name: &'static str,
    color: mtg_core::color::Color,
    size: usize,
    cards: Vec<DeckLine>,
}

#[derive(Debug, Serialize)]
struct DeckLine {
    name: String,
    count: usize,
}

/// `GET /healthz`: the process is up.
pub(crate) async fn healthz() -> &'static str {
    "ok"
}

/// Fallback for paths that match no route.
pub(crate) async fn not_found() -> ApiError {
    ApiError::not_found()
}

/// Fallback for known paths used with the wrong method.
pub(crate) async fn method_not_allowed() -> ApiError {
    ApiError::new(
        StatusCode::METHOD_NOT_ALLOWED,
        "method_not_allowed",
        "method not allowed",
    )
}

/// `GET /api/demo-decks`: the built-in decks.
pub(crate) async fn decks() -> Json<Vec<DeckBody>> {
    Json(demo::all().iter().map(deck_body).collect())
}

fn deck_body(deck: &DemoDeck) -> DeckBody {
    DeckBody {
        id: deck.id,
        name: deck.name,
        color: deck.color,
        size: deck.size(),
        cards: deck
            .list()
            .into_iter()
            .map(|(name, count)| DeckLine { name, count })
            .collect(),
    }
}

/// `GET /api/games/{id}`: public facts, never secrets. A bad id and an unknown game look alike.
pub(crate) async fn game(
    State(app): State<AppState>,
    path: Result<Path<String>, PathRejection>,
) -> Result<Json<GameInfo>, ApiError> {
    let Path(id) = path.map_err(|_| ApiError::not_found())?;
    let id: GameId = id.parse().map_err(|_| ApiError::not_found())?;
    let game = app
        .registry()
        .describe(id)
        .await
        .ok_or_else(ApiError::not_found)?;
    Ok(Json(GameInfo {
        format: game.format.as_str().to_owned(),
        started: game.started,
        seats: game
            .seats
            .into_iter()
            .map(|s| GameSeat {
                seat: s.seat,
                name: s.name,
            })
            .collect(),
        connected: game.connected,
        ready: game.ready,
        result: game.result,
        visibility: game.visibility,
        manual_play: game.manual_play,
        house_rules: Some(game.house_rules).filter(|rules| !rules.is_empty()),
    }))
}

/// `POST /api/games`: creates a Normal or Commander game and returns each seat's token (the only time it is shown).
///
/// A signed-in creator owns seat 0 from the start (the web opens "my seat" there); the rest of the
/// seats are guests' until somebody signed in opens their link.
pub(crate) async fn create_game(
    State(app): State<AppState>,
    request: Request,
) -> Result<(StatusCode, Json<CreatedBody>), ApiError> {
    if app.stopping() {
        return Err(ApiError::unavailable());
    }
    let headers = request.headers().clone();
    let body = read_json_limited::<NewGame>(request, GAME_BODY_LIMIT).await?;
    let (names, options) = validate(&body)?;
    let host = app.auth().session_user(&headers).await?.map(|user| user.id);
    let mut decks = Vec::with_capacity(names.len());
    for (index, seat) in body.seats.iter().enumerate() {
        decks.push(match seat.deck.as_deref() {
            Some(id) => Some(starting_deck(&app, &headers, index, id, &options).await?),
            None => None,
        });
    }
    let (seats, visibility) = (names.len(), options.visibility);
    let created = app
        .registry()
        .create_decked(names, decks, options, host)
        .await?;
    tracing::info!(
        game = %created.id,
        seats,
        held = created.seats.len(),
        visibility = visibility.as_str(),
        bound = host.is_some(),
        "game created"
    );
    let seats = created
        .seats
        .iter()
        .map(|(seat, token)| CreatedSeat {
            seat: *seat,
            token: token.expose().to_owned(),
        })
        .collect();
    let body = CreatedBody {
        game: created.id.to_string(),
        code: created.code.expose().to_owned(),
        seats,
    };
    Ok((StatusCode::CREATED, Json(body)))
}

/// Reads a bounded JSON body; only `application/json` is accepted, which also forces a CORS preflight.
pub(crate) async fn read_json<T: serde::de::DeserializeOwned>(
    request: Request,
) -> Result<T, ApiError> {
    read_json_limited(request, BODY_LIMIT).await
}

/// [`read_json`] for an endpoint whose bodies may be larger than `BODY_LIMIT` bytes.
pub(crate) async fn read_json_limited<T: serde::de::DeserializeOwned>(
    request: Request,
    limit: usize,
) -> Result<T, ApiError> {
    if !is_json(request.headers()) {
        return Err(ApiError::new(
            StatusCode::UNSUPPORTED_MEDIA_TYPE,
            "unsupported_media_type",
            "send the body as application/json",
        ));
    }
    let too_large = || {
        ApiError::new(
            StatusCode::PAYLOAD_TOO_LARGE,
            "payload_too_large",
            format!("the request body is limited to {limit} bytes"),
        )
    };
    let declared = request
        .headers()
        .get(axum::http::header::CONTENT_LENGTH)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.parse::<usize>().ok());
    if declared.is_some_and(|n| n > limit) {
        return Err(too_large());
    }
    let bytes = axum::body::to_bytes(request.into_body(), limit)
        .await
        .map_err(|_| too_large())?;
    serde_json::from_slice(&bytes).map_err(|error| {
        let detail: String = error.to_string().chars().take(MAX_DETAIL_CHARS).collect();
        ApiError::invalid(format!("the body is not a valid request: {detail}"))
    })
}

fn is_json(headers: &HeaderMap) -> bool {
    headers
        .get(CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.split(';').next())
        .is_some_and(|mime| mime.trim().eq_ignore_ascii_case("application/json"))
}

/// The deck seat `index` of a new game starts with. Only the creator's seat may name a stored deck,
/// and only the creator's own; every other seat starts with a built-in deck, a precon (public
/// data, so any seat may take one) or none.
async fn starting_deck(
    app: &AppState,
    headers: &HeaderMap,
    index: usize,
    id: &str,
    options: &GameOptions,
) -> Result<ChosenDeck, ApiError> {
    let found = if index == 0 || deck_choice::precon_code(id).is_some() {
        deck_choice::resolve(app, headers, id).await
    } else {
        deck_choice::built_in(id)
    };
    // The game's own rules judge the deck too: a demo deck has no commander, a Commander deck no
    // place in a Normal game.
    let found = found.and_then(
        |deck| match deck.issues_in(options.format, &options.house_rules) {
            issues if issues.is_empty() => Ok(deck),
            issues => Err(Refused::Blocked(issues)),
        },
    );
    found.map_err(|refused| {
        refused.into_error_with(
            StatusCode::BAD_REQUEST,
            &format!("seat {index}: no such deck; see GET /api/demo-decks"),
        )
    })
}

/// Checks a creation request; returns the names of the seats and what the game is made with
/// besides them.
fn validate(body: &NewGame) -> Result<(Vec<String>, GameOptions), ApiError> {
    let Some(format) = FormatKind::parse(&body.format) else {
        return Err(ApiError::new(
            StatusCode::BAD_REQUEST,
            "unsupported_format",
            "the supported formats are \"normal\" and \"commander\"",
        ));
    };
    let count = body.seats.len();
    if count < MIN_SEATS {
        return Err(ApiError::invalid(format!(
            "a game needs at least {MIN_SEATS} seats, got {count}"
        )));
    }
    let held = body.held.unwrap_or(count);
    if !(1..=count).contains(&held) {
        return Err(ApiError::invalid(format!(
            "held must be 1 to {count}: the creator's seat is always held"
        )));
    }
    let seats = body
        .seats
        .iter()
        .enumerate()
        .map(|(index, seat)| {
            // An open seat takes the name of whoever joins it; what was sent is never used.
            let name = if index < held {
                let name = seat.name.trim();
                let length = name.chars().count();
                if length == 0 || length > MAX_NAME_CHARS || name.chars().any(is_unsafe_in_names) {
                    return Err(ApiError::invalid(format!(
                        "seat {index}: the name must be 1 to {MAX_NAME_CHARS} characters without control characters"
                    )));
                }
                name.to_owned()
            } else {
                String::new()
            };
            Ok(name)
        })
        .collect::<Result<Vec<_>, ApiError>>()?;
    // The handicaps name seats of this table, so the count is checked here, before anything is made.
    body.house_rules.validate_for(count)?;
    let options = GameOptions {
        visibility: body.visibility,
        held: Some(held),
        house_rules: body.house_rules.clone(),
        format,
    };
    Ok((seats, options))
}

/// Control characters and invisible or direction-changing marks, which could make a name
/// pose as other text in the game log.
pub(crate) fn is_unsafe_in_names(c: char) -> bool {
    c.is_control()
        || matches!(
            c,
            '\u{200b}'..='\u{200f}'
                | '\u{202a}'..='\u{202e}'
                | '\u{2060}'..='\u{2064}'
                | '\u{2066}'..='\u{2069}'
                | '\u{feff}'
        )
}
