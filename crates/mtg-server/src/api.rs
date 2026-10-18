//! The REST endpoints: create a game, look one up, list the demo decks, health.

use axum::Json;
use axum::extract::rejection::PathRejection;
use axum::extract::{Path, Request, State};
use axum::http::header::CONTENT_TYPE;
use axum::http::{HeaderMap, HeaderName, HeaderValue, Method, StatusCode};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use mtg_core::ids::PlayerId;
use mtg_engine::state::GameResult;
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::demo::{self, DemoDeck};
use crate::game_id::GameId;
use crate::origin::origin_allowed;
use crate::registry::{CreateError, SeatRequest};
use crate::server::AppState;

/// Largest request body, in bytes.
pub const BODY_LIMIT: usize = 16 * 1024;
/// Fewest seats in a game.
pub const MIN_SEATS: usize = 2;
/// Most seats in a game.
pub const MAX_SEATS: usize = 12;
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
        }
    }

    /// The same error with one more header on its answer.
    pub(crate) fn with_header(mut self, name: HeaderName, value: HeaderValue) -> ApiError {
        self.headers.push((name, value));
        self
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
        let body = json!({ "error": self.code, "message": self.message });
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
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct NewSeat {
    name: String,
    deck: String,
}

#[derive(Debug, Serialize)]
pub(crate) struct CreatedBody {
    game: String,
    seats: Vec<CreatedSeat>,
}

#[derive(Debug, Serialize)]
struct CreatedSeat {
    seat: PlayerId,
    token: String,
}

#[derive(Debug, Serialize)]
struct SeatBody {
    seat: PlayerId,
    name: String,
}

#[derive(Debug, Serialize)]
pub(crate) struct GameBody {
    started: bool,
    seats: Vec<SeatBody>,
    connected: Vec<PlayerId>,
    ready: Vec<PlayerId>,
    result: Option<GameResult>,
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

/// `GET /api/decks`: the built-in decks.
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
) -> Result<Json<GameBody>, ApiError> {
    let Path(id) = path.map_err(|_| ApiError::not_found())?;
    let id: GameId = id.parse().map_err(|_| ApiError::not_found())?;
    let game = app
        .registry()
        .describe(id)
        .await
        .ok_or_else(ApiError::not_found)?;
    Ok(Json(GameBody {
        started: game.started,
        seats: game
            .seats
            .into_iter()
            .map(|s| SeatBody {
                seat: s.seat,
                name: s.name,
            })
            .collect(),
        connected: game.connected,
        ready: game.ready,
        result: game.result,
    }))
}

/// `POST /api/games`: creates a Normal game and returns each seat's token (the only time it is shown).
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
    let body = read_json::<NewGame>(request).await?;
    let seats = validate(&body)?;
    let host = app.auth().session_user(&headers).await?.map(|user| user.id);
    let created = app.registry().create_for(&seats, host).await?;
    tracing::info!(game = %created.id, seats = seats.len(), bound = host.is_some(), "game created");
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
        seats,
    };
    Ok((StatusCode::CREATED, Json(body)))
}

/// Reads a bounded JSON body; only `application/json` is accepted, which also forces a CORS preflight.
pub(crate) async fn read_json<T: serde::de::DeserializeOwned>(
    request: Request,
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
            format!("the request body is limited to {BODY_LIMIT} bytes"),
        )
    };
    let declared = request
        .headers()
        .get(axum::http::header::CONTENT_LENGTH)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.parse::<usize>().ok());
    if declared.is_some_and(|n| n > BODY_LIMIT) {
        return Err(too_large());
    }
    let bytes = axum::body::to_bytes(request.into_body(), BODY_LIMIT)
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

/// Checks a creation request and resolves its decks.
fn validate(body: &NewGame) -> Result<Vec<SeatRequest>, ApiError> {
    if body.format != "normal" {
        return Err(ApiError::new(
            StatusCode::BAD_REQUEST,
            "unsupported_format",
            "the only supported format is \"normal\"",
        ));
    }
    if !(MIN_SEATS..=MAX_SEATS).contains(&body.seats.len()) {
        return Err(ApiError::invalid(format!(
            "a game needs {MIN_SEATS} to {MAX_SEATS} seats, got {}",
            body.seats.len()
        )));
    }
    body.seats
        .iter()
        .enumerate()
        .map(|(index, seat)| {
            let name = seat.name.trim();
            let length = name.chars().count();
            if length == 0 || length > MAX_NAME_CHARS || name.chars().any(is_unsafe_in_names) {
                return Err(ApiError::invalid(format!(
                    "seat {index}: the name must be 1 to {MAX_NAME_CHARS} characters without control characters"
                )));
            }
            let deck = demo::find(&seat.deck).ok_or_else(|| {
                ApiError::new(
                    StatusCode::BAD_REQUEST,
                    "unknown_deck",
                    format!("seat {index}: no such deck; see GET /api/decks"),
                )
            })?;
            Ok(SeatRequest {
                name: name.to_owned(),
                deck,
            })
        })
        .collect()
}

/// Control characters and invisible or direction-changing marks, which could make a name
/// pose as other text in the game log.
fn is_unsafe_in_names(c: char) -> bool {
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
