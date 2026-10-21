//! Turning the id of a deck into the deck a seat plays: a built-in deck, a stored deck of the
//! signed-in caller, or a precon (`precon:<code>`), checked when it is chosen.
//!
//! A stored deck is read as it is at that moment and kept as a snapshot ([`ChosenDeck`]): the
//! oracle id of every card of its main deck, their definitions as assembled now, and the three facts
//! the table is told. The check is the one the deck list shows (`deck_report` in the deck's own
//! format), plus the rules of the game's format: a Normal game has no command zone, and the engine
//! wants 60 cards with at most four copies of a card. A precon is judged by the same rules
//! (`precons.rs`) and needs no account.

use axum::http::{HeaderMap, StatusCode};
use mtg_cards::deck::{DeckReport, DeckState, DeckZone, Issue, IssueCode};
use mtg_cards::support::Support;
use mtg_db::DeckId;
use mtg_engine::format::{DeckError, FormatRules, Normal};
use mtg_protocol::SeatDeck;
use mtg_protocol::rest::{DeckBlockedBody, DeckChoiceErrorCode, PRECON_DECK_PREFIX};
use sqlx::types::uuid::Uuid;

use crate::api::ApiError;
use crate::card_api::failed;
use crate::deck_cards::{Line, Loaded};
use crate::decks::lines_of;
use crate::demo;
use crate::precons::Judge;
use crate::seat_deck::ChosenDeck;
use crate::server::AppState;

/// Why an id did not become a deck.
#[derive(Debug)]
pub(crate) enum Refused {
    /// The id is no built-in deck and no stored deck of the caller's. A deck of another account, a
    /// deck that does not exist and a caller who is not signed in all look alike.
    Unknown,
    /// The deck check refuses the deck.
    Blocked(Vec<Issue>),
    /// The answer to send as it is: the database failed.
    Failed(ApiError),
}

impl From<ApiError> for Refused {
    fn from(error: ApiError) -> Refused {
        Refused::Failed(error)
    }
}

impl Refused {
    /// The answer to a request that names the deck of a seat of a game that exists: 404
    /// `unknown_deck`, or 422 `deck_blocked` with the issues.
    pub(crate) fn into_error(self) -> ApiError {
        self.into_error_with(StatusCode::NOT_FOUND, "no deck has that id")
    }

    /// Like [`Refused::into_error`], with the status and words of an unknown deck chosen by the
    /// caller (a new game answers 400, naming the seat).
    pub(crate) fn into_error_with(self, unknown: StatusCode, message: &str) -> ApiError {
        match self {
            Refused::Unknown => {
                ApiError::new(unknown, DeckChoiceErrorCode::UnknownDeck.as_str(), message)
            }
            Refused::Blocked(issues) => blocked(&issues),
            Refused::Failed(error) => error,
        }
    }
}

/// The 422 of a refused deck: the issues go in the body next to `error` and `message`.
fn blocked(issues: &[Issue]) -> ApiError {
    let body = DeckBlockedBody {
        error: DeckChoiceErrorCode::DeckBlocked,
        message: "that deck cannot be played in this game".to_owned(),
        issues: issues.to_vec(),
    };
    let message = body.message.clone();
    let mut error = ApiError::new(
        StatusCode::UNPROCESSABLE_ENTITY,
        DeckChoiceErrorCode::DeckBlocked.as_str(),
        message,
    );
    if let Ok(extra) = serde_json::to_value(&body.issues) {
        error = error.with_field("issues", extra);
    }
    error
}

/// The code of the precon an id names (`precon:<code>`), if it names one.
pub(crate) fn precon_code(id: &str) -> Option<&str> {
    id.trim().strip_prefix(PRECON_DECK_PREFIX)
}

/// The deck with this id for the caller: a built-in deck, a precon, or one of the signed-in
/// caller's stored decks. Nothing about a deck of anybody else is told.
pub(crate) async fn resolve(
    app: &AppState,
    headers: &HeaderMap,
    id: &str,
) -> Result<ChosenDeck, Refused> {
    if let Some(code) = precon_code(id) {
        return precon(app, code).await;
    }
    if let Some(deck) = demo::find(id) {
        return ChosenDeck::demo(deck).map_err(|error| {
            tracing::error!(%error, "a built-in deck could not be fingerprinted");
            Refused::Failed(ApiError::new(
                StatusCode::INTERNAL_SERVER_ERROR,
                "internal",
                "the deck could not be read",
            ))
        });
    }
    stored(app, headers, id).await
}

/// A built-in deck by its id; a stored deck's id is [`Refused::Unknown`] here. For the seats of a
/// new game other than the creator's, which no stored deck may fill.
pub(crate) fn built_in(id: &str) -> Result<ChosenDeck, Refused> {
    let deck = demo::find(id).ok_or(Refused::Unknown)?;
    ChosenDeck::demo(deck).map_err(|error| {
        tracing::error!(%error, "a built-in deck could not be fingerprinted");
        Refused::Failed(ApiError::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            "internal",
            "the deck could not be read",
        ))
    })
}

/// The precon with this code as a seat's deck: judged as the list shows it, then refused when the
/// deck check blocks it or a game of this server cannot take it. An unknown code, a malformed one
/// and a server without a database are one [`Refused::Unknown`].
async fn precon(app: &AppState, code: &str) -> Result<ChosenDeck, Refused> {
    let Some(source) = app.cards() else {
        return Err(Refused::Unknown);
    };
    let index = source.precon_index().await.map_err(ApiError::from)?;
    let found = index
        .find(&code.trim().to_ascii_lowercase())
        .ok_or(Refused::Unknown)?;
    let deck = source
        .database()
        .precons()
        .get(found.id)
        .await
        .map_err(|error| Refused::Failed(failed(&error)))?
        .ok_or(Refused::Unknown)?;
    let rules = source.rules().await.map_err(ApiError::from)?;
    let judged = Judge::new(&rules).judge(&deck);
    if !judged.playable {
        return Err(Refused::Blocked(judged.issues));
    }
    let note = SeatDeck {
        name: deck.summary.name.clone(),
        format: judged.format.to_owned(),
        manual_cards: manual_cards(&judged.report),
    };
    ChosenDeck::stored(judged.library, judged.cards, note).map_err(|error| {
        tracing::error!(%error, "a precon could not be fingerprinted");
        Refused::Failed(ApiError::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            "internal",
            "the deck could not be read",
        ))
    })
}

async fn stored(app: &AppState, headers: &HeaderMap, id: &str) -> Result<ChosenDeck, Refused> {
    let (Some(source), Ok(id)) = (app.cards(), Uuid::parse_str(id.trim())) else {
        return Err(Refused::Unknown);
    };
    let Some(user) = app.auth().session_user(headers).await? else {
        return Err(Refused::Unknown);
    };
    let deck = source
        .database()
        .decks()
        .get(user.id.uuid(), DeckId(id))
        .await
        .map_err(|error| Refused::Failed(failed(&error)))?
        .ok_or(Refused::Unknown)?;
    let lines = lines_of(&deck);
    let loaded = Loaded::load(source, &lines).await?;
    let report = loaded.report(&deck.format, &lines);

    let mut issues = report.legality.clone();
    if lines.iter().any(|line| line.zone == DeckZone::Commander) {
        issues.push(no_command_zone());
    }
    if !issues.is_empty() || report.summary.state == DeckState::Blocked {
        return Err(Refused::Blocked(issues));
    }
    let ids = library_ids(&lines);
    let cards = source.deck(&ids).await.map_err(ApiError::from)?;
    if let Err(error) = Normal.validate_deck(&cards) {
        return Err(Refused::Blocked(vec![engine_issue(error)]));
    }
    let note = SeatDeck {
        name: deck.name.clone(),
        format: deck.format.clone(),
        manual_cards: manual_cards(&report),
    };
    ChosenDeck::stored(ids, cards, note).map_err(|error| {
        tracing::error!(%error, "a stored deck could not be fingerprinted");
        Refused::Failed(ApiError::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            "internal",
            "the deck could not be read",
        ))
    })
}

/// The oracle id of every card of the main deck, a line's copies one after the other, in line order:
/// the library the game is given.
pub(crate) fn library_ids(lines: &[Line]) -> Vec<String> {
    lines
        .iter()
        .filter(|line| line.zone == DeckZone::Main)
        .flat_map(|line| std::iter::repeat_n(line.oracle_id.clone(), line.count as usize))
        .collect()
}

/// Copies of the main deck's cards the game does not run whole; a card of the sideboard or the
/// companion slot never reaches the game, so the table is told nothing of it.
pub(crate) fn manual_cards(report: &DeckReport) -> u32 {
    report
        .cards
        .iter()
        .filter(|card| card.zone == DeckZone::Main && card.support != Support::Verified)
        .map(|card| card.count)
        .fold(0, u32::saturating_add)
}

pub(crate) fn no_command_zone() -> Issue {
    Issue {
        code: IssueCode::NoCommanderZone,
        message: "a Normal game has no command zone, so the cards in it cannot be played"
            .to_owned(),
        zone: Some(DeckZone::Commander),
        oracle_id: None,
        name: None,
    }
}

/// A rule of the game's format the deck breaks, in the words of the deck check.
pub(crate) fn engine_issue(error: DeckError) -> Issue {
    match error {
        DeckError::TooFewCards { min, got } => Issue {
            code: IssueCode::DeckSize,
            message: format!(
                "a Normal game needs at least {min} cards in the main deck; this has {got}"
            ),
            zone: Some(DeckZone::Main),
            oracle_id: None,
            name: None,
        },
        DeckError::TooManyCopies { name, max, got } => Issue {
            code: IssueCode::Copies,
            message: format!("{name} appears {got} times; a Normal game allows {max}"),
            zone: Some(DeckZone::Main),
            oracle_id: None,
            name: Some(name),
        },
    }
}

#[cfg(test)]
mod tests {
    //! Goal: the library of a stored deck is its main deck's cards in line order with copies side by side, the table is told only the main deck's manual copies, the game's format adds its own issues to the deck check's, and a refusal reaches the wire as a 422 with its issues or a 404 that tells nothing about the deck.
    use mtg_cards::deck::{Counts, ReportCard, Summary};
    use rstest::rstest;

    use super::*;

    fn line(zone: DeckZone, id: &str, count: u32) -> Line {
        Line {
            zone,
            oracle_id: id.to_owned(),
            printing_id: None,
            count,
        }
    }

    fn card(zone: DeckZone, support: Support, count: u32) -> ReportCard {
        ReportCard {
            oracle_id: "id".into(),
            name: "Card".into(),
            zone,
            count,
            support,
            reasons: Vec::new(),
        }
    }

    fn report(cards: Vec<ReportCard>) -> DeckReport {
        DeckReport {
            deck_hash: String::new(),
            coverage_hash: String::new(),
            format: "modern".into(),
            legality: Vec::new(),
            cards,
            summary: Summary {
                format: "modern".into(),
                state: DeckState::ManualPlay,
                counts: Counts::default(),
                zones: Vec::new(),
                issues: 0,
            },
        }
    }

    #[test]
    fn the_library_is_the_main_deck_with_each_lines_copies_side_by_side() {
        let lines = [
            line(DeckZone::Main, "b", 2),
            line(DeckZone::Sideboard, "x", 4),
            line(DeckZone::Main, "a", 3),
            line(DeckZone::Commander, "c", 1),
            line(DeckZone::Companion, "d", 1),
            line(DeckZone::Main, "b", 1),
        ];
        assert_eq!(library_ids(&lines), ["b", "b", "a", "a", "a", "b"]);
        assert_eq!(library_ids(&[]), Vec::<String>::new());
    }

    #[rstest]
    #[case::main_only(vec![card(DeckZone::Main, Support::Manual, 3)], 3)]
    #[case::every_state_but_verified(vec![
        card(DeckZone::Main, Support::Verified, 40),
        card(DeckZone::Main, Support::Partial, 2),
        card(DeckZone::Main, Support::Manual, 3),
        card(DeckZone::Main, Support::Stale, 4),
    ], 9)]
    #[case::the_sideboard_never_reaches_the_game(vec![
        card(DeckZone::Main, Support::Verified, 60),
        card(DeckZone::Sideboard, Support::Manual, 15),
        card(DeckZone::Companion, Support::Partial, 1),
    ], 0)]
    #[case::nothing(Vec::new(), 0)]
    fn the_table_is_told_the_copies_of_the_main_deck_that_are_not_verified(
        #[case] cards: Vec<ReportCard>,
        #[case] expected: u32,
    ) {
        assert_eq!(manual_cards(&report(cards)), expected);
    }

    #[test]
    fn the_rules_of_a_normal_game_become_issues_of_the_deck_check() {
        let few = engine_issue(DeckError::TooFewCards { min: 60, got: 40 });
        assert_eq!(
            (few.code, few.zone),
            (IssueCode::DeckSize, Some(DeckZone::Main))
        );
        assert!(few.message.contains("60") && few.message.contains("40"));
        let many = engine_issue(DeckError::TooManyCopies {
            name: "Lightning Bolt".into(),
            max: 4,
            got: 5,
        });
        assert_eq!(many.code, IssueCode::Copies);
        assert_eq!(many.name.as_deref(), Some("Lightning Bolt"));
        assert_eq!(no_command_zone().code, IssueCode::NoCommanderZone);
        assert_eq!(no_command_zone().zone, Some(DeckZone::Commander));
    }

    #[test]
    fn a_blocked_deck_is_a_422_that_lists_its_issues() {
        let error = Refused::Blocked(vec![no_command_zone()]).into_error();
        assert_eq!(error.status(), StatusCode::UNPROCESSABLE_ENTITY);
        assert_eq!(error.code(), "deck_blocked");
        let issues = error.field("issues").unwrap();
        assert_eq!(issues[0]["code"], "no_commander_zone");
        let empty = Refused::Blocked(Vec::new()).into_error();
        assert_eq!(
            empty.field("issues").unwrap().as_array().map(Vec::len),
            Some(0)
        );
    }

    #[test]
    fn an_unknown_deck_is_a_404_or_the_status_the_caller_chose_and_says_nothing_else() {
        let error = Refused::Unknown.into_error();
        assert_eq!(
            (error.status(), error.code()),
            (StatusCode::NOT_FOUND, "unknown_deck")
        );
        assert!(error.field("issues").is_none());
        let at_creation =
            Refused::Unknown.into_error_with(StatusCode::BAD_REQUEST, "seat 0: no such deck");
        assert_eq!(at_creation.status(), StatusCode::BAD_REQUEST);
        assert_eq!(at_creation.message(), "seat 0: no such deck");
    }

    #[test]
    fn a_built_in_deck_is_chosen_by_its_id_and_a_stored_deck_id_is_not_one() {
        let chosen = built_in("mono-red").unwrap();
        assert_eq!(chosen.note().name, "Mono Red Giants");
        assert_eq!(chosen.cards().len(), 60);
        for id in ["", "MONO-RED", "4457ed35-7c10-48c8-9776-456485fdf070"] {
            assert!(matches!(built_in(id), Err(Refused::Unknown)), "{id:?}");
        }
    }
}
