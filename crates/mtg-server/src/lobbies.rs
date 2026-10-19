//! The game browser's endpoints: the public listing (`GET /api/lobbies`), joining by a game's id
//! or by its join code, giving a seat back, the host removing a player, the host listing or
//! hiding the game (`PATCH /api/games/{id}`) and the host closing it (`DELETE /api/games/{id}`).
//!
//! A visitor who asks for a game by its id learns nothing about invite games: an unknown id, an
//! invite game reached by its id and a wrong code are one and the same `404`. A seat is named by
//! its token (`Authorization: Bearer`, which needs no database) or by the session cookie.

use std::fmt::Write as _;
use std::time::Instant;

use axum::Json;
use axum::extract::rejection::PathRejection;
use axum::extract::{Path, Request, State};
use axum::http::header::{AUTHORIZATION, ETAG, IF_NONE_MATCH, RETRY_AFTER};
use axum::http::{HeaderMap, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use mtg_core::ids::PlayerId;
use mtg_protocol::rest::{
    JoinCodeRequest, JoinRequest, Joined, LobbyErrorCode, LobbyGame, LobbyList, VisibilityBody,
};
use sha2::{Digest, Sha256};

use crate::api::{ApiError, MAX_NAME_CHARS, MAX_SEATS, is_unsafe_in_names, read_json};
use crate::auth::client_address;
use crate::game_id::GameId;
use crate::registry::{ClaimError, Entrance, HOST, ListedGame, SeatClaim, Seated, TableError};
use crate::server::AppState;
use crate::throttle::Blocked;

/// Bytes of the SHA-256 of a listing that its `ETag` keeps: 128 bits.
const TAG_BYTES: usize = 16;

fn failure(status: StatusCode, code: LobbyErrorCode, message: impl Into<String>) -> ApiError {
    ApiError::new(status, code.as_str(), message)
}

/// 429 with the seconds to wait in `Retry-After`.
fn turned_away(blocked: Blocked, code: LobbyErrorCode, what: &str) -> ApiError {
    let seconds = blocked.seconds();
    // Quiet on purpose: a client that keeps knocking would fill the log.
    tracing::debug!(retry_after = seconds, what, "a client was turned away");
    failure(
        StatusCode::TOO_MANY_REQUESTS,
        code,
        format!("too many {what}; try again in {seconds} seconds"),
    )
    .with_header(RETRY_AFTER, HeaderValue::from(seconds))
}

fn invalid_name() -> ApiError {
    failure(
        StatusCode::BAD_REQUEST,
        LobbyErrorCode::InvalidName,
        format!("the name must be 1 to {MAX_NAME_CHARS} characters without control characters"),
    )
}

fn shutting_down() -> ApiError {
    failure(
        StatusCode::SERVICE_UNAVAILABLE,
        LobbyErrorCode::Unavailable,
        "the server is shutting down",
    )
}

impl From<ClaimError> for ApiError {
    fn from(error: ClaimError) -> ApiError {
        match error {
            // An unknown game and an invite game reached by its id are the same answer.
            ClaimError::NotFound => ApiError::not_found(),
            ClaimError::Full => failure(
                StatusCode::CONFLICT,
                LobbyErrorCode::Full,
                "every seat of that game is taken",
            ),
            ClaimError::Started => failure(
                StatusCode::CONFLICT,
                LobbyErrorCode::Started,
                "that game has already started",
            ),
            ClaimError::BadName => invalid_name(),
            ClaimError::Record(_) => {
                tracing::error!(%error, "a seat could not be claimed");
                failure(
                    StatusCode::SERVICE_UNAVAILABLE,
                    LobbyErrorCode::Unavailable,
                    "the seat could not be saved; try again later",
                )
            }
            ClaimError::Random(_) => {
                tracing::error!(%error, "a seat could not be claimed");
                failure(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    LobbyErrorCode::Internal,
                    "the seat could not be claimed",
                )
            }
        }
    }
}

impl From<TableError> for ApiError {
    fn from(error: TableError) -> ApiError {
        match error {
            TableError::NotFound => ApiError::not_found(),
            TableError::NotAllowed => forbidden("only the host can do that"),
            TableError::NotClaimed => failure(
                StatusCode::CONFLICT,
                LobbyErrorCode::NotClaimed,
                "that seat was made with the game, not taken through the game browser or a code",
            ),
            TableError::Started => failure(
                StatusCode::CONFLICT,
                LobbyErrorCode::Started,
                "that game has already started",
            ),
            TableError::Record(_) => {
                tracing::error!(%error, "a change to a table could not be saved");
                failure(
                    StatusCode::SERVICE_UNAVAILABLE,
                    LobbyErrorCode::Unavailable,
                    "the change could not be saved; try again later",
                )
            }
        }
    }
}

fn forbidden(message: &str) -> ApiError {
    failure(StatusCode::FORBIDDEN, LobbyErrorCode::Forbidden, message)
}

/// A game id from a path; a malformed one is a `404`, like an unknown game.
fn game_of(path: Result<Path<String>, PathRejection>) -> Result<GameId, ApiError> {
    let Path(id) = path.map_err(|_| ApiError::not_found())?;
    id.parse().map_err(|_| ApiError::not_found())
}

// --- the listing ---------------------------------------------------------------------------

/// `GET /api/lobbies`: the public games still in their lobby, newest first, at most 50.
///
/// The `ETag` is a digest of what the rows say apart from their age, so a poll that finds
/// nothing new gets a `304` however long the games have been waiting.
pub(crate) async fn list(
    State(app): State<AppState>,
    request: Request,
) -> Result<Response, ApiError> {
    if app.stopping() {
        return Err(failure(
            StatusCode::SERVICE_UNAVAILABLE,
            LobbyErrorCode::LobbiesUnavailable,
            "the server is shutting down",
        ));
    }
    let address = client_address(&request, app.config().auth.trusted_proxies);
    app.registry()
        .admit_listing(address, Instant::now())
        .map_err(|blocked| {
            turned_away(
                blocked,
                LobbyErrorCode::RateLimited,
                "requests for the list",
            )
        })?;
    let listed = app.registry().lobbies().await;
    let tag = listing_tag(&listed);
    let Ok(header) = HeaderValue::from_str(&tag) else {
        return Err(failure(
            StatusCode::INTERNAL_SERVER_ERROR,
            LobbyErrorCode::Internal,
            "the list could not be made",
        ));
    };
    if tag_matches(request.headers(), &tag) {
        return Ok((StatusCode::NOT_MODIFIED, [(ETAG, header)]).into_response());
    }
    let games = listed.into_iter().map(view).collect();
    Ok(([(ETAG, header)], Json(LobbyList { games })).into_response())
}

/// A listed game as the wire shows it.
fn view(game: ListedGame) -> LobbyGame {
    let count = |n: usize| u32::try_from(n).unwrap_or(u32::MAX);
    LobbyGame {
        game: game.game.to_string(),
        host: game.host,
        format: game.format.as_str().to_owned(),
        filled: count(game.filled),
        total: count(game.total),
        created_at: game.created_at,
        age_secs: game.age.as_secs(),
    }
}

/// The strong validator of a listing: a digest of every row but its age, which changes with every
/// second. Fields are length-prefixed so that neighbours cannot run into each other.
fn listing_tag(listed: &[ListedGame]) -> String {
    let mut hash = Sha256::new();
    for game in listed {
        let filled = game.filled.to_string();
        let total = game.total.to_string();
        let id = game.game.to_string();
        let parts = [
            id.as_str(),
            &game.host,
            game.format.as_str(),
            &filled,
            &total,
            &game.created_at,
        ];
        for part in parts {
            hash.update(u64::try_from(part.len()).unwrap_or(u64::MAX).to_be_bytes());
            hash.update(part.as_bytes());
        }
    }
    let digest = hash.finalize();
    let mut tag = String::with_capacity(2 + 2 * TAG_BYTES);
    tag.push('"');
    for byte in &digest[..TAG_BYTES] {
        // Writing to a `String` cannot fail.
        let _ = write!(tag, "{byte:02x}");
    }
    tag.push('"');
    tag
}

/// Whether `If-None-Match` names `tag`. The comparison is the weak one a `GET` uses, `*` matches
/// any listing, and a list of tags matches when any one does.
fn tag_matches(headers: &HeaderMap, tag: &str) -> bool {
    headers
        .get_all(IF_NONE_MATCH)
        .iter()
        .filter_map(|value| value.to_str().ok())
        .flat_map(|line| line.split(','))
        .map(str::trim)
        .any(|candidate| {
            candidate == "*" || candidate.strip_prefix("W/").unwrap_or(candidate) == tag
        })
}

// --- joining -------------------------------------------------------------------------------

/// `POST /api/games/{id}/join`: takes a seat of a public game in its lobby.
pub(crate) async fn join_game(
    State(app): State<AppState>,
    path: Result<Path<String>, PathRejection>,
    request: Request,
) -> Result<Json<Joined>, ApiError> {
    let id = game_of(path)?;
    if app.stopping() {
        return Err(shutting_down());
    }
    let headers = request.headers().clone();
    let body = read_json::<JoinRequest>(request).await?;
    let claim = visitor(&app, &headers, body.name).await?;
    let seated = app.registry().claim(id, Entrance::Listing, claim).await?;
    Ok(joined(seated))
}

/// `POST /api/join`: takes a seat of the game a join code names, public or not.
pub(crate) async fn join_by_code(
    State(app): State<AppState>,
    request: Request,
) -> Result<Json<Joined>, ApiError> {
    if app.stopping() {
        return Err(shutting_down());
    }
    let address = client_address(&request, app.config().auth.trusted_proxies);
    let headers = request.headers().clone();
    let body = read_json::<JoinCodeRequest>(request).await?;
    // The name is judged first: a visitor must not be able to tell a right code from a wrong one
    // by whether the name was asked for.
    let claim = visitor(&app, &headers, body.name).await?;
    let found = app
        .registry()
        .find_by_code(&body.code, address, Instant::now())
        .map_err(|blocked| turned_away(blocked, LobbyErrorCode::TooManyAttempts, "wrong codes"))?;
    let id = found.ok_or_else(ApiError::not_found)?;
    let seated = app.registry().claim(id, Entrance::Code, claim).await?;
    Ok(joined(seated))
}

/// Who is joining and what the table calls them. A signed-in visitor is bound to the account and
/// is named after it unless they say otherwise; a guest must give a name.
async fn visitor(
    app: &AppState,
    headers: &HeaderMap,
    name: Option<String>,
) -> Result<SeatClaim, ApiError> {
    let user = app.auth().session_user(headers).await?;
    let name = match (name, &user) {
        (Some(name), _) => checked_name(&name)?,
        (None, Some(user)) => user.username.clone(),
        (None, None) => return Err(invalid_name()),
    };
    Ok(SeatClaim {
        name,
        user: user.map(|user| user.id),
    })
}

/// A name the table may show: 1 to 32 characters once trimmed, without control characters or
/// invisible marks.
fn checked_name(name: &str) -> Result<String, ApiError> {
    let name = name.trim();
    let length = name.chars().count();
    if length == 0 || length > MAX_NAME_CHARS || name.chars().any(is_unsafe_in_names) {
        return Err(invalid_name());
    }
    Ok(name.to_owned())
}

fn joined(seated: Seated) -> Json<Joined> {
    Json(Joined {
        seat: seated.seat,
        token: seated.token.map(|token| token.expose().to_owned()),
    })
}

// --- a seat's holder and the host ----------------------------------------------------------

/// The seat the request holds in `game`: by the bearer token if there is one, else by the
/// session. A token or session that opens no seat of the game is a `404`, the answer for a game
/// that does not exist, so a stranger learns nothing about it.
async fn holder(app: &AppState, headers: &HeaderMap, game: GameId) -> Result<PlayerId, ApiError> {
    let access = if let Some(token) = bearer(headers) {
        app.registry().authenticate(game, token)
    } else {
        let user = app.auth().require_user(headers).await?;
        app.registry().authenticate_user(game, user.id)
    };
    access
        .map(|access| access.seat)
        .ok_or_else(ApiError::not_found)
}

/// The token of an `Authorization: Bearer` header. Another scheme (a proxy's basic auth) is not
/// a seat token and is left alone.
fn bearer(headers: &HeaderMap) -> Option<&str> {
    let value = headers.get(AUTHORIZATION)?.to_str().ok()?;
    let (scheme, token) = value.split_once(' ')?;
    scheme.eq_ignore_ascii_case("bearer").then(|| token.trim())
}

/// `POST /api/games/{id}/leave`: the seat's holder gives it back before the game starts.
pub(crate) async fn leave(
    State(app): State<AppState>,
    path: Result<Path<String>, PathRejection>,
    headers: HeaderMap,
) -> Result<StatusCode, ApiError> {
    let game = game_of(path)?;
    if app.stopping() {
        return Err(shutting_down());
    }
    let seat = holder(&app, &headers, game).await?;
    app.registry().release(game, seat, seat).await?;
    Ok(StatusCode::NO_CONTENT)
}

/// `POST /api/games/{id}/seats/{n}/release`: the host removes the player in seat `n` before the
/// game starts.
pub(crate) async fn release(
    State(app): State<AppState>,
    path: Result<Path<(String, String)>, PathRejection>,
    headers: HeaderMap,
) -> Result<StatusCode, ApiError> {
    let Path((game, seat)) = path.map_err(|_| ApiError::not_found())?;
    let game: GameId = game.parse().map_err(|_| ApiError::not_found())?;
    let seat = seat_number(&seat).ok_or_else(ApiError::not_found)?;
    if app.stopping() {
        return Err(shutting_down());
    }
    let by = holder(&app, &headers, game).await?;
    if by != HOST {
        return Err(forbidden("only the host can remove a player"));
    }
    app.registry().release(game, seat, by).await?;
    Ok(StatusCode::NO_CONTENT)
}

/// A seat number in a path: one or two digits, with no leading zero, that name a seat any game
/// could have.
fn seat_number(text: &str) -> Option<PlayerId> {
    let plain = (1..=2).contains(&text.len())
        && text.bytes().all(|b| b.is_ascii_digit())
        && (text.len() == 1 || !text.starts_with('0'));
    let number: usize = text.parse().ok().filter(|_| plain)?;
    (number < MAX_SEATS).then(|| PlayerId::from_index(number))
}

/// `PATCH /api/games/{id}`: the host lists the game or hides it again before it starts.
pub(crate) async fn set_visibility(
    State(app): State<AppState>,
    path: Result<Path<String>, PathRejection>,
    request: Request,
) -> Result<Json<VisibilityBody>, ApiError> {
    let game = game_of(path)?;
    if app.stopping() {
        return Err(shutting_down());
    }
    let headers = request.headers().clone();
    let body = read_json::<VisibilityBody>(request).await?;
    let by = holder(&app, &headers, game).await?;
    app.registry()
        .set_visibility(game, by, body.visibility)
        .await?;
    Ok(Json(body))
}

/// `DELETE /api/games/{id}`: the host closes a game that is still in its lobby. Its sockets are
/// closed with 4002 and it leaves the listing at once.
pub(crate) async fn close(
    State(app): State<AppState>,
    path: Result<Path<String>, PathRejection>,
    headers: HeaderMap,
) -> Result<StatusCode, ApiError> {
    let game = game_of(path)?;
    if app.stopping() {
        return Err(shutting_down());
    }
    let by = holder(&app, &headers, game).await?;
    app.registry().close(game, by).await?;
    Ok(StatusCode::NO_CONTENT)
}

#[cfg(test)]
mod tests {
    //! Goal: the small rules that decide what a game browser request means are exact: which text is a seat number, which header is a seat token, which `If-None-Match` names a listing, what the listing's tag does and does not depend on, and which names a table may show.
    use std::time::Duration;

    use axum::http::HeaderValue;
    use rstest::rstest;
    use uuid::Uuid;

    use super::*;
    use crate::recipe::FormatKind;

    #[rstest]
    #[case("0", Some(0))]
    #[case("1", Some(1))]
    #[case("9", Some(9))]
    #[case("10", Some(10))]
    #[case("11", Some(11))]
    #[case("12", None)]
    #[case("99", None)]
    #[case("00", None)]
    #[case("01", None)]
    #[case("001", None)]
    #[case("+1", None)]
    #[case("-1", None)]
    #[case("1.5", None)]
    #[case(" 1", None)]
    #[case("1 ", None)]
    #[case("", None)]
    #[case("one", None)]
    #[case("１", None)]
    fn a_seat_number_is_one_or_two_plain_digits_of_a_seat_a_game_can_have(
        #[case] text: &str,
        #[case] seat: Option<usize>,
    ) {
        assert_eq!(seat_number(text), seat.map(PlayerId::from_index));
    }

    fn with(name: &'static str, value: &'static str) -> HeaderMap {
        let mut headers = HeaderMap::new();
        headers.append(
            axum::http::HeaderName::from_static(name),
            HeaderValue::from_static(value),
        );
        headers
    }

    #[rstest]
    #[case::bearer("Bearer abc", Some("abc"))]
    #[case::any_case("bEARer abc", Some("abc"))]
    #[case::padded("Bearer   abc  ", Some("abc"))]
    #[case::no_token("Bearer", None)]
    #[case::empty_token("Bearer ", Some(""))]
    #[case::basic("Basic dXNlcjpwYXNz", None)]
    #[case::digest("Digest abc", None)]
    #[case::a_bare_token("abc", None)]
    #[case::no_space("Bearerabc", None)]
    #[case::empty("", None)]
    fn only_a_bearer_header_carries_a_seat_token(
        #[case] value: &'static str,
        #[case] token: Option<&str>,
    ) {
        assert_eq!(bearer(&with("authorization", value)), token);
    }

    #[test]
    fn a_request_without_an_authorization_header_has_no_token() {
        assert_eq!(bearer(&HeaderMap::new()), None);
    }

    const TAG: &str = "\"0123456789abcdef0123456789abcdef\"";

    #[rstest]
    #[case::the_tag(TAG, true)]
    #[case::weak("W/\"0123456789abcdef0123456789abcdef\"", true)]
    #[case::any("*", true)]
    #[case::in_a_list("\"x\", \"0123456789abcdef0123456789abcdef\"", true)]
    #[case::in_a_list_without_spaces("\"x\",\"0123456789abcdef0123456789abcdef\",\"y\"", true)]
    #[case::weak_in_a_list("\"x\", W/\"0123456789abcdef0123456789abcdef\"", true)]
    #[case::padded("  \"0123456789abcdef0123456789abcdef\"  ", true)]
    #[case::another("\"fedcba9876543210fedcba9876543210\"", false)]
    #[case::unquoted("0123456789abcdef0123456789abcdef", false)]
    #[case::a_prefix("\"0123456789abcdef\"", false)]
    #[case::lower_case_weak("w/\"0123456789abcdef0123456789abcdef\"", false)]
    #[case::empty("", false)]
    #[case::only_commas(",,", false)]
    fn if_none_match_names_the_listing_as_a_get_reads_it(
        #[case] value: &'static str,
        #[case] matched: bool,
    ) {
        assert_eq!(tag_matches(&with("if-none-match", value), TAG), matched);
    }

    #[test]
    fn several_if_none_match_headers_are_read_together() {
        let mut headers = with("if-none-match", "\"x\"");
        headers.append(IF_NONE_MATCH, HeaderValue::from_static(TAG));
        assert!(tag_matches(&headers, TAG));
        assert!(!tag_matches(&HeaderMap::new(), TAG));
    }

    fn game(host: &str, filled: usize) -> ListedGame {
        ListedGame {
            game: GameId(Uuid::from_u128(7)),
            host: host.to_owned(),
            format: FormatKind::Normal,
            filled,
            total: 4,
            created_at: "2026-10-19T12:30:00Z".to_owned(),
            age: Duration::from_secs(5),
        }
    }

    #[test]
    fn the_tag_is_a_quoted_128_bit_hex_digest() {
        let tag = listing_tag(&[game("Ann", 1)]);
        assert_eq!(tag.len(), 34);
        assert!(tag.starts_with('"') && tag.ends_with('"'));
        assert!(
            tag[1..33]
                .bytes()
                .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
        );
        // The empty list has a tag too.
        assert_ne!(listing_tag(&[]), tag);
        assert_eq!(listing_tag(&[]), listing_tag(&[]));
    }

    #[test]
    fn the_tag_follows_every_field_of_a_row_except_its_age() {
        let base = game("Ann", 1);
        let tag = listing_tag(std::slice::from_ref(&base));
        let older = ListedGame {
            age: Duration::from_mins(50),
            ..base.clone()
        };
        assert_eq!(listing_tag(&[older]), tag, "age must not move the tag");

        let changes = [
            ListedGame {
                game: GameId(Uuid::from_u128(8)),
                ..base.clone()
            },
            ListedGame {
                host: "Bo".to_owned(),
                ..base.clone()
            },
            ListedGame {
                filled: 2,
                ..base.clone()
            },
            ListedGame {
                total: 5,
                ..base.clone()
            },
            ListedGame {
                created_at: "2026-10-19T12:30:01Z".to_owned(),
                ..base.clone()
            },
        ];
        for changed in changes {
            assert_ne!(
                listing_tag(std::slice::from_ref(&changed)),
                tag,
                "{changed:?}"
            );
        }
    }

    #[test]
    fn the_tag_follows_the_order_and_the_number_of_rows() {
        let (ann, bo) = (game("Ann", 1), game("Bo", 2));
        let tags = [
            listing_tag(&[ann.clone(), bo.clone()]),
            listing_tag(&[bo.clone(), ann.clone()]),
            listing_tag(std::slice::from_ref(&ann)),
            listing_tag(&[ann.clone(), ann]),
            listing_tag(&[bo]),
        ];
        let distinct: std::collections::HashSet<&String> = tags.iter().collect();
        assert_eq!(distinct.len(), tags.len());
    }

    #[test]
    fn neighbouring_fields_cannot_run_into_each_other() {
        // "Ann" + "1" and "Ann1" + "" would hash alike without a length in front of each field.
        let one = ListedGame {
            host: "Ann".to_owned(),
            created_at: "1x".to_owned(),
            ..game("", 1)
        };
        let other = ListedGame {
            host: "Ann1".to_owned(),
            created_at: "x".to_owned(),
            ..game("", 1)
        };
        assert_ne!(listing_tag(&[one]), listing_tag(&[other]));
    }

    #[rstest]
    #[case::plain("Bo", Some("Bo"))]
    #[case::trimmed("  Bo\t", Some("Bo"))]
    #[case::one_character("B", Some("B"))]
    #[case::the_longest(
        "nnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnn",
        Some("nnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnn")
    )]
    #[case::counted_in_characters(
        "éééééééééééééééééééééééééééééééé",
        Some("éééééééééééééééééééééééééééééééé")
    )]
    #[case::too_long("nnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnn", None)]
    #[case::empty("", None)]
    #[case::blank(" \t\n", None)]
    #[case::newline_inside("A\nB", None)]
    #[case::nul("A\0B", None)]
    #[case::bidi_override("A\u{202e}B", None)]
    #[case::zero_width("A\u{200b}B", None)]
    fn a_name_is_trimmed_and_judged_by_its_characters(
        #[case] given: &str,
        #[case] shown: Option<&str>,
    ) {
        let checked = checked_name(given).ok();
        assert_eq!(checked.as_deref(), shown);
    }
}
