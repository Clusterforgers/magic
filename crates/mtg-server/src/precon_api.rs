//! The precon browser's endpoints: `GET /api/precons` lists every stored precon with its deck check
//! and `GET /api/precons/{code}` shows one with its cards by board. Both are read-only and need no
//! account, since a precon is card data; a server without a database answers 503 `cards_unavailable`.
//! The deck check of a precon (`POST /api/decks/check` with `precon:<code>`) is answered here too.
//!
//! Every request counts against the address's budget (`AttemptLimits::precon_requests_per_ip`), the
//! unknown codes too, so the endpoints cannot be used to probe or to make the server rebuild its
//! index over and over.

use std::collections::HashMap;
use std::net::IpAddr;
use std::time::Instant;

use axum::Json;
use axum::extract::rejection::PathRejection;
use axum::extract::{Path, Request, State};
use axum::http::header::RETRY_AFTER;
use axum::http::{HeaderValue, StatusCode};
use mtg_cards::card::Span;
use mtg_cards::deck::{DeckReport, DeckZone, ReportCard};
use mtg_cards::support::{Reason, Support};
use mtg_db::PreconDeckDetail;
use mtg_protocol::rest::{
    CheckRequest, PreconBoard, PreconCardView, PreconErrorCode, PreconList, PreconView,
};

use crate::api::ApiError;
use crate::auth::client_address;
use crate::card_api::{failed, source};
use crate::cards::CardSource;
use crate::deck_cards::{Line, Loaded};
use crate::deck_choice::judge_cards_at_table;
use crate::decks::table_of;
use crate::precons::{Judge, Judged, PreconError, zone_of};
use crate::server::AppState;

impl From<PreconError> for ApiError {
    fn from(error: PreconError) -> ApiError {
        failed(&error)
    }
}

/// The reason a card has when the database does not hold it.
const UNKNOWN_CARD: &str = "the card is not in the card database";

/// The card source, once the caller is within its budget.
fn admitted<'a>(app: &'a AppState, request: &Request) -> Result<&'a CardSource, ApiError> {
    admitted_address(
        app,
        client_address(request, app.config().auth.trusted_proxies),
    )
}

/// The card source, once the address is within the budget of the precon endpoints.
fn admitted_address(app: &AppState, address: IpAddr) -> Result<&CardSource, ApiError> {
    let cards = source(app)?;
    app.registry()
        .admit_precons(address, Instant::now())
        .map_err(|blocked| {
            let seconds = blocked.seconds();
            tracing::debug!(
                retry_after = seconds,
                "a client was turned away from the precons"
            );
            ApiError::new(
                StatusCode::TOO_MANY_REQUESTS,
                PreconErrorCode::RateLimited.as_str(),
                format!("too many requests for precons; try again in {seconds} seconds"),
            )
            .with_header(RETRY_AFTER, HeaderValue::from(seconds))
        })?;
    Ok(cards)
}

/// The precon with this code, judged as the list shows it, with the deck it was read from; `None`
/// for a code nobody has.
pub(crate) async fn judged_precon(
    cards: &CardSource,
    code: &str,
) -> Result<Option<(PreconDeckDetail, Judged)>, ApiError> {
    let index = cards.precon_index().await?;
    let Some(found) = index.find(&code.trim().to_ascii_lowercase()) else {
        return Ok(None);
    };
    let Some(deck) = cards
        .database()
        .precons()
        .get(found.id)
        .await
        .map_err(|error| failed(&error))?
    else {
        return Ok(None);
    };
    let rules = cards.rules().await?;
    let judged = Judge::new(&rules).judge(&deck);
    Ok(Some((deck, judged)))
}

/// `POST /api/decks/check` for `precon:<code>` as the deck: the precon's report, judged by the
/// table the request names (`game` or `table`) when it names one. Public like the rest of the
/// precon endpoints and counted against the same budget.
pub(crate) async fn check(
    app: &AppState,
    address: IpAddr,
    body: CheckRequest,
    code: &str,
) -> Result<Json<DeckReport>, ApiError> {
    let cards = admitted_address(app, address)?;
    if body.format.is_some() || body.cards.is_some() {
        return Err(ApiError::invalid(
            "send either the id of a deck, or a format with the cards of a list",
        ));
    }
    let table = table_of(app, body.game.as_deref(), body.table.as_ref())?;
    let (_, judged) = judged_precon(cards, code)
        .await?
        .ok_or_else(ApiError::not_found)?;
    let mut report = judged.report;
    if let Some((format, house)) = table {
        judge_cards_at_table(&mut report, format, &house, &judged.cards, &judged.leaders);
    }
    Ok(Json(report))
}

/// `GET /api/precons`: every stored precon, newest release first, then by name.
pub(crate) async fn list(
    State(app): State<AppState>,
    request: Request,
) -> Result<Json<PreconList>, ApiError> {
    let cards = admitted(&app, &request)?;
    let index = cards.precon_index().await?;
    Ok(Json(PreconList {
        precons: index.items().cloned().collect(),
    }))
}

/// `GET /api/precons/{code}`: one precon with its cards by board and each card's support.
pub(crate) async fn get_one(
    State(app): State<AppState>,
    path: Result<Path<String>, PathRejection>,
    request: Request,
) -> Result<Json<PreconView>, ApiError> {
    let cards = admitted(&app, &request)?;
    // A path that is no code is as unknown as a code nobody has.
    let Path(code) = path.map_err(|_| ApiError::not_found())?;
    let index = cards.precon_index().await?;
    let indexed = index
        .find(&code.to_ascii_lowercase())
        .ok_or_else(ApiError::not_found)?;
    let deck = cards
        .database()
        .precons()
        .get(indexed.id)
        .await
        .map_err(|error| failed(&error))?
        .ok_or_else(ApiError::not_found)?;
    let rules = cards.rules().await?;
    let judged = Judge::new(&rules).judge(&deck);
    let loaded = Loaded::load_under(cards, rules.clone(), &judged.lines).await?;

    let reported: HashMap<(DeckZone, &str), &ReportCard> = judged
        .report
        .cards
        .iter()
        .map(|card| ((card.zone, card.oracle_id.as_str()), card))
        .collect();
    let mut boards: Vec<PreconBoard> = Vec::new();
    for zone in [DeckZone::Commander, DeckZone::Main, DeckZone::Sideboard] {
        let mut board = PreconBoard {
            board: zone,
            copies: 0,
            cards: Vec::new(),
        };
        for entry in deck.entries.iter().filter(|e| zone_of(e.board) == zone) {
            board.copies = board.copies.saturating_add(entry.count);
            let printing_id = entry.printing_id.as_ref().map(ToString::to_string);
            let line = entry.card.as_ref().map(|card| Line {
                zone,
                oracle_id: card.oracle_id.to_string(),
                printing_id: printing_id.clone(),
                count: entry.count,
            });
            let hit = match &line {
                Some(line) => loaded.view(line).map(|view| view.card),
                None => None,
            };
            let record = line
                .as_ref()
                .and_then(|line| reported.get(&(zone, line.oracle_id.as_str())));
            let (support, reasons) = match record {
                Some(record) => (record.support, record.reasons.clone()),
                None => (
                    Support::Unplayable,
                    vec![Reason {
                        face: 0,
                        span: Span { start: 0, end: 0 },
                        text: UNKNOWN_CARD.to_owned(),
                    }],
                ),
            };
            board.cards.push(PreconCardView {
                name: entry
                    .card
                    .as_ref()
                    .map_or_else(|| entry.card_name.clone(), |card| card.name.clone()),
                count: entry.count,
                printing_id,
                card: hit,
                support,
                reasons,
            });
        }
        if !board.cards.is_empty() {
            boards.push(board);
        }
    }
    Ok(Json(PreconView {
        precon: judged.item(&deck, &indexed.item.code),
        issues: judged.issues,
        boards,
    }))
}
