//! The art endpoints: the printings of a card (`GET /api/cards/{oracle_id}/printings`, public), the printing a
//! stored deck shows for a card's lines (`PUT /api/decks/{id}/printing`, owner only) and the art an account plays a
//! card in when a deck line chooses none (`/api/me/printings`). A printing must be one of the card; a server without
//! a database answers 503.

use std::collections::HashMap;

use axum::Json;
use axum::extract::rejection::PathRejection;
use axum::extract::{Path, Request, State};
use axum::http::{HeaderMap, StatusCode};
use mtg_db::codec::rarity_keyword;
use mtg_db::{DeckId, Printing, PrintingChoice};
use mtg_import::ids::{OracleId, PrintingId};
use mtg_protocol::rest::{
    DeckErrorCode, LinePrintingBody, PreferredPrintingBody, PreferredPrintings, PrintingList,
    PrintingView,
};
use sqlx::types::uuid::Uuid;

use crate::api::{ApiError, read_json_limited};
use crate::card_api::{card_images, failed};
use crate::cards::CardSource;
use crate::deck_cards::{Line, invalid_deck, stored_zone};
use crate::server::AppState;

/// Largest body of an art choice, in bytes.
const BODY_LIMIT: usize = 4 * 1024;

/// A printing as the wire shows it.
pub(crate) fn printing_view(printing: &Printing) -> PrintingView {
    PrintingView {
        printing_id: printing.printing_id.to_string(),
        set: printing.set.clone(),
        collector_number: printing.collector_number.clone(),
        rarity: printing.rarity.map(|r| rarity_keyword(r).to_owned()),
        released_at: printing.released_at.map(|date| date.to_string()),
        lang: printing.lang.clone(),
        digital: printing.digital,
        images: card_images(&printing.images),
        set_name: printing.set_name.clone(),
        frame: printing.frame.clone(),
        finishes: printing.finishes.clone(),
        full_art: printing.full_art,
    }
}

/// The lines with the account's preferred art on every line that chooses none.
pub(crate) fn with_preferred(
    lines: Vec<Line>,
    preferred: &HashMap<OracleId, PrintingId>,
) -> Vec<Line> {
    lines
        .into_iter()
        .map(|mut line| {
            if line.printing_id.is_none() {
                line.printing_id = preferred
                    .get(&OracleId(line.oracle_id.clone()))
                    .map(ToString::to_string);
            }
            line
        })
        .collect()
}

/// The preferred art of `owner`; an account whose choices do not read plays the default art.
pub(crate) async fn preferred_of(cards: &CardSource, owner: Uuid) -> HashMap<OracleId, PrintingId> {
    match cards.database().preferred_printings().all(owner).await {
        Ok(found) => found,
        Err(error) => {
            tracing::warn!(%error, "the preferred art did not read; the default art stands");
            HashMap::new()
        }
    }
}

fn no_database() -> ApiError {
    ApiError::new(
        StatusCode::SERVICE_UNAVAILABLE,
        DeckErrorCode::CardsUnavailable.as_str(),
        "this server runs without a database, so it has no printings",
    )
}

/// An id in the path in its canonical spelling; a malformed one and an unknown one look alike.
fn path_id(path: Result<Path<String>, PathRejection>) -> Result<Uuid, ApiError> {
    let Path(id) = path.map_err(|_| ApiError::not_found())?;
    Uuid::parse_str(id.trim()).map_err(|_| ApiError::not_found())
}

/// A printing id of a body in its canonical spelling.
fn printing_of_body(id: &str) -> Result<PrintingId, ApiError> {
    Uuid::parse_str(id.trim())
        .map(|uuid| PrintingId(uuid.to_string()))
        .map_err(|_| ApiError::invalid("printing_id is not a printing id"))
}

fn not_of_card() -> ApiError {
    invalid_deck("the printing does not exist or is not a printing of the card")
}

/// `GET /api/cards/{oracle_id}/printings`: the card's printings, newest first; an unknown card is 404.
pub(crate) async fn of_card(
    State(app): State<AppState>,
    path: Result<Path<String>, PathRejection>,
) -> Result<Json<PrintingList>, ApiError> {
    let cards = app.cards().ok_or_else(no_database)?;
    let id = OracleId(path_id(path)?.to_string());
    let repo = cards.database().cards();
    let printings = repo.printings_of(&id).await.map_err(|e| failed(&e))?;
    if printings.is_empty() && repo.get(&id).await.map_err(|e| failed(&e))?.is_none() {
        return Err(ApiError::not_found());
    }
    Ok(Json(PrintingList {
        printings: printings.iter().map(printing_view).collect(),
    }))
}

/// `PUT /api/decks/{id}/printing`: the art of a card's lines in one zone of the account's deck, or the default with
/// a null printing; 204. Another account's deck, an unknown deck and a card the zone does not hold are 404; a
/// printing of another card is 400 `invalid_deck`.
pub(crate) async fn set_line(
    State(app): State<AppState>,
    path: Result<Path<String>, PathRejection>,
    request: Request,
) -> Result<StatusCode, ApiError> {
    if app.stopping() {
        return Err(ApiError::unavailable());
    }
    let cards = app.cards().ok_or_else(|| {
        ApiError::new(
            StatusCode::SERVICE_UNAVAILABLE,
            DeckErrorCode::DecksUnavailable.as_str(),
            "this server runs without a database, so it has no decks",
        )
    })?;
    let user = app.auth().require_user(request.headers()).await?;
    let deck = DeckId(path_id(path)?);
    let body: LinePrintingBody = read_json_limited(request, BODY_LIMIT).await?;
    let oracle = Uuid::parse_str(body.oracle_id.trim())
        .map(|uuid| OracleId(uuid.to_string()))
        .map_err(|_| ApiError::invalid("oracle_id is not an oracle id"))?;
    let printing = body
        .printing_id
        .as_deref()
        .map(printing_of_body)
        .transpose()?;
    let choice = cards
        .database()
        .decks()
        .set_printing(
            user.id.uuid(),
            deck,
            stored_zone(body.zone),
            &oracle,
            printing.as_ref(),
        )
        .await
        .map_err(|e| failed(&e))?;
    match choice {
        PrintingChoice::Set => Ok(StatusCode::NO_CONTENT),
        PrintingChoice::NoDeck | PrintingChoice::NoLine => Err(ApiError::not_found()),
        PrintingChoice::NotOfCard => Err(not_of_card()),
    }
}

/// The card source and the signed-in account of a preferred-art request.
async fn account<'a>(
    app: &'a AppState,
    headers: &HeaderMap,
) -> Result<(&'a CardSource, Uuid), ApiError> {
    let cards = app.cards().ok_or_else(no_database)?;
    let user = app.auth().require_user(headers).await?;
    Ok((cards, user.id.uuid()))
}

/// `GET /api/me/printings`: the art the account plays each card in when a deck line chooses none.
pub(crate) async fn preferred(
    State(app): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<PreferredPrintings>, ApiError> {
    let (cards, owner) = account(&app, &headers).await?;
    let found = cards
        .database()
        .preferred_printings()
        .all(owner)
        .await
        .map_err(|e| failed(&e))?;
    Ok(Json(PreferredPrintings {
        printings: found
            .into_iter()
            .map(|(card, printing)| (card.to_string(), printing.to_string()))
            .collect(),
    }))
}

/// `PUT /api/me/printings/{oracle_id}`: plays the card in this printing where a deck line chooses none; 204. A
/// printing of another card is 400 `invalid_deck`.
pub(crate) async fn prefer(
    State(app): State<AppState>,
    path: Result<Path<String>, PathRejection>,
    request: Request,
) -> Result<StatusCode, ApiError> {
    if app.stopping() {
        return Err(ApiError::unavailable());
    }
    let (cards, owner) = account(&app, request.headers()).await?;
    let oracle = OracleId(path_id(path)?.to_string());
    let body: PreferredPrintingBody = read_json_limited(request, BODY_LIMIT).await?;
    let printing = printing_of_body(&body.printing_id)?;
    let stored = cards
        .database()
        .preferred_printings()
        .set(owner, &oracle, &printing)
        .await
        .map_err(|e| failed(&e))?;
    if stored {
        Ok(StatusCode::NO_CONTENT)
    } else {
        Err(not_of_card())
    }
}

/// `DELETE /api/me/printings/{oracle_id}`: the card's default art again; 204, also when none was chosen.
pub(crate) async fn forget(
    State(app): State<AppState>,
    path: Result<Path<String>, PathRejection>,
    headers: HeaderMap,
) -> Result<StatusCode, ApiError> {
    if app.stopping() {
        return Err(ApiError::unavailable());
    }
    let (cards, owner) = account(&app, &headers).await?;
    let oracle = OracleId(path_id(path)?.to_string());
    cards
        .database()
        .preferred_printings()
        .clear(owner, &oracle)
        .await
        .map_err(|e| failed(&e))?;
    Ok(StatusCode::NO_CONTENT)
}

#[cfg(test)]
mod tests {
    //! Goal: a line without art takes the account's preferred printing, a line with art keeps its own, and ids in a body or path are read in any spelling while anything else is refused.
    use rstest::rstest;

    use super::*;
    use mtg_cards::deck::DeckZone;

    const CARD: &str = "4457ed35-7c10-48c8-9776-456485fdf070";
    const OTHER: &str = "e3285e6b-3e79-4d7c-bf96-d920f973b122";
    const OWN: &str = "00000000-0000-4000-8000-000000000001";
    const LIKED: &str = "00000000-0000-4000-8000-000000000002";

    fn line(oracle: &str, printing: Option<&str>) -> Line {
        Line {
            zone: DeckZone::Main,
            oracle_id: oracle.to_owned(),
            printing_id: printing.map(str::to_owned),
            count: 1,
            by_hand: false,
        }
    }

    #[rstest]
    #[case::none_takes_the_preferred(line(CARD, None), Some(LIKED))]
    #[case::own_choice_stays(line(CARD, Some(OWN)), Some(OWN))]
    #[case::no_preference_stays_default(line(OTHER, None), None)]
    fn preferred_art_fills_only_lines_without_art(
        #[case] given: Line,
        #[case] printing: Option<&str>,
    ) {
        let preferred = HashMap::from([(OracleId(CARD.to_owned()), PrintingId(LIKED.to_owned()))]);
        let out = with_preferred(vec![given], &preferred);
        assert_eq!(out[0].printing_id.as_deref(), printing);
    }

    #[rstest]
    #[case(
        " 00000000-0000-4000-8000-00000000000A ",
        Some("00000000-0000-4000-8000-00000000000a")
    )]
    #[case("not-an-id", None)]
    #[case("", None)]
    fn body_printing_ids_are_read_in_any_spelling(#[case] text: &str, #[case] read: Option<&str>) {
        let got = printing_of_body(text);
        match read {
            Some(id) => assert_eq!(got.unwrap().to_string(), id),
            None => assert_eq!(got.unwrap_err().code(), "invalid_request"),
        }
    }

    #[rstest]
    #[case("nope")]
    #[case("")]
    fn path_ids_that_are_no_ids_are_404(#[case] text: &str) {
        let error = path_id(Ok(Path(text.to_owned()))).unwrap_err();
        assert_eq!(error.status(), StatusCode::NOT_FOUND);
    }
}
