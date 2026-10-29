//! The picture endpoints: `GET /api/images/oracle/{oracle_id}/{face}/{size}`,
//! `GET /api/images/printing/{printing_id}/{face}/{size}` and
//! `GET /api/images/token/{face}/{size}?name=...`. Public and read-only like the card endpoints.

use std::collections::HashMap;

use axum::body::Body;
use axum::extract::rejection::{PathRejection, QueryRejection};
use axum::extract::{Path, Query, State};
use axum::http::header::{CACHE_CONTROL, CONTENT_TYPE, ETAG};
use axum::http::{HeaderMap, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use mtg_import::ids::{OracleId, PrintingId};
use sqlx::types::uuid::Uuid;

use super::resolve::{Identity, Size, TokenQuery, link_for};
use super::{FetchError, ImageError, ImageLink, Images};
use crate::api::ApiError;
use crate::card_api::{colors_of, failed, source};
use crate::lobbies::tag_matches;
use crate::server::AppState;

/// How long a browser keeps a picture without asking: a week. The `ETag` settles the rest.
const CACHE_CONTROL_VALUE: &str = "public, max-age=604800";
/// Longest token name, type line, power or toughness, in characters.
const MAX_FACT_CHARS: usize = 200;
/// The parameters `GET /api/images/token/...` knows.
const TOKEN_PARAMETERS: [&str; 5] = ["name", "type", "colors", "power", "toughness"];

/// The typed 404 that tells the client to draw the card's text face: the identity is unknown, or
/// it has no picture of that face and size.
fn no_image() -> ApiError {
    ApiError::new(
        StatusCode::NOT_FOUND,
        "no_image",
        "there is no picture for this card, face and size",
    )
}

fn unavailable() -> ApiError {
    ApiError::new(
        StatusCode::SERVICE_UNAVAILABLE,
        "images_unavailable",
        "this server has no picture cache",
    )
}

/// A failure to get a picture the database names: the CDN lacking it is a missing picture, any
/// other failure is a bad gateway, so the client draws the text face and may ask again later.
fn upstream_error(error: &ImageError) -> ApiError {
    if matches!(error, ImageError::Fetch(FetchError::Status(404 | 410))) {
        return no_image();
    }
    tracing::warn!(%error, "a picture could not be fetched");
    ApiError::new(
        StatusCode::BAD_GATEWAY,
        "image_unavailable",
        "the picture could not be fetched right now; try again later",
    )
}

fn parse_face(text: &str) -> Result<usize, ApiError> {
    text.parse::<usize>()
        .map_err(|_| ApiError::invalid("face is the number of the face, 0 for the front"))
}

fn parse_size(text: &str) -> Result<Size, ApiError> {
    Size::parse(text).ok_or_else(|| ApiError::invalid(format!("size is {}", Size::NAMES)))
}

/// `GET /api/images/oracle/{oracle_id}/{face}/{size}`: the picture of a card, from its newest
/// paper printing that has one. A malformed and an unknown id look alike.
pub(crate) async fn by_oracle(
    State(app): State<AppState>,
    headers: HeaderMap,
    path: Result<Path<(String, String, String)>, PathRejection>,
) -> Result<Response, ApiError> {
    let Path((id, face, size)) = path.map_err(|_| no_image())?;
    let (face, size) = (parse_face(&face)?, parse_size(&size)?);
    let id = Uuid::parse_str(&id).map_err(|_| no_image())?;
    let identity = Identity::Oracle(OracleId(id.to_string()));
    serve(&app, &headers, &identity, face, size).await
}

/// `GET /api/images/printing/{printing_id}/{face}/{size}`: the picture of one printing.
pub(crate) async fn by_printing(
    State(app): State<AppState>,
    headers: HeaderMap,
    path: Result<Path<(String, String, String)>, PathRejection>,
) -> Result<Response, ApiError> {
    let Path((id, face, size)) = path.map_err(|_| no_image())?;
    let (face, size) = (parse_face(&face)?, parse_size(&size)?);
    let id = Uuid::parse_str(&id).map_err(|_| no_image())?;
    let identity = Identity::Printing(PrintingId(id.to_string()));
    serve(&app, &headers, &identity, face, size).await
}

/// `GET /api/images/token/{face}/{size}?name=&type=&colors=&power=&toughness=`: the picture of a
/// token; `name` is required and every other fact given must match the stored token.
pub(crate) async fn by_token(
    State(app): State<AppState>,
    headers: HeaderMap,
    path: Result<Path<(String, String)>, PathRejection>,
    query: Result<Query<HashMap<String, String>>, QueryRejection>,
) -> Result<Response, ApiError> {
    let Path((face, size)) = path.map_err(|_| no_image())?;
    let (face, size) = (parse_face(&face)?, parse_size(&size)?);
    let Query(params) = query.map_err(|_| ApiError::invalid("the query string is not valid"))?;
    let identity = Identity::Token(token_query(&params)?);
    serve(&app, &headers, &identity, face, size).await
}

/// Reads the facts of a token; a blank value is absent and every problem is a 400 naming it.
fn token_query(params: &HashMap<String, String>) -> Result<TokenQuery, ApiError> {
    if let Some(unknown) = params
        .keys()
        .filter(|key| !TOKEN_PARAMETERS.contains(&key.as_str()))
        .min()
    {
        return Err(ApiError::invalid(format!("unknown parameter {unknown:?}")));
    }
    let get = |key: &str| params.get(key).map(|v| v.trim()).filter(|v| !v.is_empty());
    if let Some(long) = TOKEN_PARAMETERS
        .iter()
        .find(|key| get(key).is_some_and(|v| v.chars().count() > MAX_FACT_CHARS))
    {
        return Err(ApiError::invalid(format!(
            "{long} is at most {MAX_FACT_CHARS} characters"
        )));
    }
    let name = get("name").ok_or_else(|| ApiError::invalid("name is the token's name"))?;
    Ok(TokenQuery {
        name: name.to_owned(),
        type_line: get("type").map(str::to_owned),
        colors: get("colors").map(|v| colors_of("colors", v)).transpose()?,
        power: get("power").map(str::to_owned),
        toughness: get("toughness").map(str::to_owned),
    })
}

/// Resolves the identity to its stored link and answers the picture, or 304 when the client holds it.
async fn serve(
    app: &AppState,
    headers: &HeaderMap,
    identity: &Identity,
    face: usize,
    size: Size,
) -> Result<Response, ApiError> {
    let cards = source(app)?;
    let images: &Images = cards.images().ok_or_else(unavailable)?;
    let repo = cards.database().cards();
    let stored = link_for(&repo, identity, face, size)
        .await
        .map_err(|error| failed(&error))?
        .ok_or_else(no_image)?;
    let link = ImageLink::parse(&stored).map_err(|error| {
        tracing::warn!(%error, link = stored, "a stored picture link is refused");
        no_image()
    })?;
    let tag = link.etag();
    if tag_matches(headers, &tag) {
        return Ok(with_validators(StatusCode::NOT_MODIFIED, &tag, Body::empty()).into_response());
    }
    let bytes = images
        .get(&link)
        .await
        .map_err(|error| upstream_error(&error))?;
    let mut response = with_validators(StatusCode::OK, &tag, Body::from(bytes));
    response
        .headers_mut()
        .insert(CONTENT_TYPE, HeaderValue::from_static(link.content_type()));
    Ok(response)
}

/// An answer that carries the `ETag` and the caching rule, which a 304 repeats.
fn with_validators(status: StatusCode, tag: &str, body: Body) -> Response {
    let mut response = Response::new(body);
    *response.status_mut() = status;
    let headers = response.headers_mut();
    headers.insert(CACHE_CONTROL, HeaderValue::from_static(CACHE_CONTROL_VALUE));
    if let Ok(value) = HeaderValue::from_str(tag) {
        headers.insert(ETAG, value);
    }
    response
}

#[cfg(test)]
mod tests {
    //! Goal: the token facts are read exactly: a name is required, blank facts are absent, colors use the card letters, and anything else is a 400 that names the problem.
    use rstest::rstest;

    use super::*;

    fn params(pairs: &[(&str, &str)]) -> HashMap<String, String> {
        pairs
            .iter()
            .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
            .collect()
    }

    #[test]
    fn every_fact_is_read() {
        let query = token_query(&params(&[
            ("name", " Goblin "),
            ("type", "Creature \u{2014} Goblin"),
            ("colors", "r"),
            ("power", "1"),
            ("toughness", "*"),
        ]))
        .unwrap();
        assert_eq!(query.name, "Goblin");
        assert_eq!(query.type_line.as_deref(), Some("Creature \u{2014} Goblin"));
        assert_eq!(query.colors, colors_of("colors", "R").ok());
        assert_eq!(
            (query.power.as_deref(), query.toughness.as_deref()),
            (Some("1"), Some("*"))
        );
    }

    #[test]
    fn blank_facts_are_absent() {
        let query = token_query(&params(&[
            ("name", "Treasure"),
            ("type", " "),
            ("colors", ""),
        ]))
        .unwrap();
        assert_eq!(
            query,
            TokenQuery {
                name: "Treasure".into(),
                ..TokenQuery::default()
            }
        );
    }

    #[rstest]
    #[case::no_name(&[])]
    #[case::blank_name(&[("name", "  ")])]
    #[case::unknown(&[("name", "Goblin"), ("size", "normal")])]
    #[case::blank_unknown(&[("name", "Goblin"), ("extra", "")])]
    #[case::bad_colors(&[("name", "Goblin"), ("colors", "RX")])]
    fn refused(#[case] pairs: &[(&str, &str)]) {
        let error = token_query(&params(pairs)).unwrap_err();
        assert_eq!(error.status(), StatusCode::BAD_REQUEST);
        assert_eq!(error.code(), "invalid_request");
    }

    #[test]
    fn a_long_fact_is_refused_by_name() {
        let long = "x".repeat(MAX_FACT_CHARS + 1);
        let error = token_query(&params(&[("name", "Goblin"), ("type", &long)])).unwrap_err();
        assert!(error.message().contains("type"), "{}", error.message());
    }

    #[rstest]
    #[case::missing_picture(
        ImageError::Fetch(FetchError::Status(404)),
        StatusCode::NOT_FOUND,
        "no_image"
    )]
    #[case::gone(
        ImageError::Fetch(FetchError::Status(410)),
        StatusCode::NOT_FOUND,
        "no_image"
    )]
    #[case::busy(
        ImageError::Fetch(FetchError::Status(429)),
        StatusCode::BAD_GATEWAY,
        "image_unavailable"
    )]
    #[case::broken(
        ImageError::Fetch(FetchError::Status(500)),
        StatusCode::BAD_GATEWAY,
        "image_unavailable"
    )]
    #[case::unreachable(ImageError::Fetch(FetchError::Transport("down".into())), StatusCode::BAD_GATEWAY, "image_unavailable")]
    #[case::not_a_picture(
        ImageError::Fetch(FetchError::NotAPicture),
        StatusCode::BAD_GATEWAY,
        "image_unavailable"
    )]
    fn the_cdn_failing_is_a_missing_picture_or_a_bad_gateway(
        #[case] error: ImageError,
        #[case] status: StatusCode,
        #[case] code: &str,
    ) {
        let answer = upstream_error(&error);
        assert_eq!((answer.status(), answer.code()), (status, code));
    }
}
