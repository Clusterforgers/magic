//! Turning the id of a deck into the deck a seat plays: a built-in deck, a stored deck of the
//! signed-in caller, or a precon (`precon:<code>`), checked when it is chosen.
//!
//! A stored deck is read as it is at that moment and kept as a snapshot ([`ChosenDeck`]): the
//! oracle id of every card of its main deck and of its command zone, their definitions as assembled
//! now, and the three facts the table is told. The check is the one the deck list shows
//! (`deck_report` in the deck's own format); the rules of the game's format come after it, once the
//! game is known ([`format_issues`]): a Normal game has no command zone and wants 60 cards with at
//! most four copies of a card, a Commander game the commanders, 100 cards, one copy of each and the
//! colour identity. A precon is judged by the same rules (`precons.rs`) and needs no account.

use axum::http::{HeaderMap, StatusCode};
use mtg_cards::CardDefinition;
use mtg_cards::deck::{DeckReport, DeckZone, Issue, IssueCode};
use mtg_cards::support::Support;
use mtg_core::house::HouseRules;
use mtg_db::DeckId;
use mtg_engine::format::{Commander, DeckError, FormatRules, Normal, RuleSet};
use mtg_protocol::SeatDeck;
use mtg_protocol::rest::{DeckBlockedBody, DeckChoiceErrorCode, PRECON_DECK_PREFIX};
use sqlx::types::uuid::Uuid;
use std::sync::Arc;

use crate::api::ApiError;
use crate::card_api::failed;
use crate::cards::CardSource;
use crate::deck_cards::{Line, Loaded};
use crate::decks::lines_of;
use crate::demo;
use crate::precons::Judge;
use crate::recipe::FormatKind;
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
pub(crate) fn blocked(issues: &[Issue]) -> ApiError {
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
    // What the deck check refuses and no table can lift ends here; the rest, and the engine's own
    // check, wait for the table the deck is chosen for.
    if is_held_back(&judged.report) {
        return Err(Refused::Blocked(judged.issues));
    }
    let own_issues = judged.report.legality.clone();
    let note = SeatDeck {
        name: deck.summary.name.clone(),
        format: judged.format.to_owned(),
        manual_cards: manual_cards(&judged.report),
    };
    ChosenDeck::led(
        judged.library,
        judged.cards,
        judged.commanders,
        judged.leaders,
        note,
    )
    .map(|chosen| chosen.with_own_issues(own_issues))
    .map_err(|error| {
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

    if is_held_back(&report) {
        return Err(Refused::Blocked(report.legality.clone()));
    }
    let ids = library_ids(&lines);
    let commander_ids = commander_ids(&lines);
    let both: Vec<String> = ids.iter().chain(&commander_ids).cloned().collect();
    let mut cards = source.deck(&both).await.map_err(ApiError::from)?;
    let commanders = cards.split_off(ids.len());
    let note = SeatDeck {
        name: deck.name.clone(),
        format: deck.format.clone(),
        manual_cards: manual_cards(&report),
    };
    ChosenDeck::led(ids, cards, commander_ids, commanders, note)
        .map(|chosen| chosen.with_own_issues(report.legality.clone()))
        .map_err(|error| {
            tracing::error!(%error, "a stored deck could not be fingerprinted");
            Refused::Failed(ApiError::new(
                StatusCode::INTERNAL_SERVER_ERROR,
                "internal",
                "the deck could not be read",
            ))
        })
}

/// The report of a deck as the table with this `format` and these `house` rules reads it: the issues
/// the table's deck rules lift are gone, and a deck the check has nothing left against is judged by
/// the engine under the table's rules (`lines` are the deck's cards). The issues are those a seat
/// choosing the deck would be refused with ([`ChosenDeck::issues_in`]).
pub(crate) async fn judge_at_table(
    source: &CardSource,
    lines: &[Line],
    report: &mut DeckReport,
    format: FormatKind,
    house: &HouseRules,
) -> Result<(), ApiError> {
    let none = HouseRules::default();
    let house = if format == FormatKind::Commander {
        house
    } else {
        &none
    };
    let mut legality: Vec<Issue> = report
        .legality
        .iter()
        .filter(|issue| !issue.code.lifted_by(house))
        .cloned()
        .collect();
    if legality.is_empty() && report.summary.counts.unplayable == 0 {
        let ids = library_ids(lines);
        let leaders = commander_ids(lines);
        let both: Vec<String> = ids.iter().chain(&leaders).cloned().collect();
        let mut cards = source.deck(&both).await?;
        let commanders = cards.split_off(ids.len());
        legality = format_issues(format, &cards, &commanders, house);
    }
    report.set_legality(legality);
    Ok(())
}

/// Whether the deck check refuses a deck at every table: a card the game cannot play, or an issue
/// that no deck rule of a table lifts. The other issues are judged at the table the deck is for
/// ([`ChosenDeck::issues_in`]).
pub(crate) fn is_held_back(report: &DeckReport) -> bool {
    report.summary.counts.unplayable > 0
        || report.legality.iter().any(|issue| !issue.code.liftable())
}

/// The oracle id of every card of a zone, a line's copies one after the other, in line order.
fn zone_ids(lines: &[Line], zone: DeckZone) -> Vec<String> {
    lines
        .iter()
        .filter(|line| line.zone == zone)
        .flat_map(|line| std::iter::repeat_n(line.oracle_id.clone(), line.count as usize))
        .collect()
}

/// The oracle id of every card of the main deck, a line's copies one after the other, in line order:
/// the library the game is given.
pub(crate) fn library_ids(lines: &[Line]) -> Vec<String> {
    zone_ids(lines, DeckZone::Main)
}

/// The oracle id of every card of the command zone: the commanders the seat starts with.
pub(crate) fn commander_ids(lines: &[Line]) -> Vec<String> {
    zone_ids(lines, DeckZone::Commander)
}

/// Copies of the cards a game plays (the main deck and the commanders) that it does not run whole;
/// a card of the sideboard or the companion slot never reaches the game, so the table is told
/// nothing of it.
pub(crate) fn manual_cards(report: &DeckReport) -> u32 {
    report
        .cards
        .iter()
        .filter(|card| {
            matches!(card.zone, DeckZone::Main | DeckZone::Commander)
                && card.support != Support::Verified
        })
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

/// The rules of `format` that a seat's library and commanders break, in the words of the deck
/// check: a Normal game has no command zone and wants 60 cards with at most four copies of a card,
/// a Commander game the engine's `validate_seat` (the commander, the size, the singleton rule and
/// the colour identity) under the deck relaxations of the game's `house` rules.
pub(crate) fn format_issues(
    format: FormatKind,
    library: &[Arc<CardDefinition>],
    commanders: &[Arc<CardDefinition>],
    house: &HouseRules,
) -> Vec<Issue> {
    match format {
        FormatKind::Normal => {
            let mut issues = Vec::new();
            if !commanders.is_empty() {
                issues.push(no_command_zone());
            }
            if let Err(error) = Normal.validate_deck(library) {
                issues.push(engine_issue(error, format));
            }
            issues
        }
        FormatKind::Commander => Commander
            .validate_seat(library, commanders, &RuleSet::new(&Commander, house))
            .err()
            .map(|error| engine_issue(error, format))
            .into_iter()
            .collect(),
    }
}

/// A rule of the game's format the deck breaks, in the words of the deck check.
pub(crate) fn engine_issue(error: DeckError, format: FormatKind) -> Issue {
    let game = match format {
        FormatKind::Normal => "Normal",
        FormatKind::Commander => "Commander",
    };
    match error {
        DeckError::TooFewCards { min, got } => Issue {
            code: IssueCode::DeckSize,
            message: format!(
                "a {game} game needs at least {min} cards in the main deck; this has {got}"
            ),
            zone: Some(DeckZone::Main),
            oracle_id: None,
            name: None,
        },
        DeckError::TooManyCopies { name, max, got } => Issue {
            code: IssueCode::Copies,
            message: format!("{name} appears {got} times; a {game} game allows {max}"),
            zone: Some(DeckZone::Main),
            oracle_id: None,
            name: Some(name),
        },
        // The Commander format's other rules, in the words of the engine's error.
        other => {
            let (code, zone, name) = match &other {
                DeckError::WrongSize { .. } => (IssueCode::DeckSize, DeckZone::Main, None),
                DeckError::CommandersNotAllowed => {
                    (IssueCode::NoCommanderZone, DeckZone::Commander, None)
                }
                DeckError::NoCommander | DeckError::TooManyCommanders { .. } => {
                    (IssueCode::CommanderCount, DeckZone::Commander, None)
                }
                DeckError::NotACommander { name } => (
                    IssueCode::CommanderEligibility,
                    DeckZone::Commander,
                    Some(name.clone()),
                ),
                DeckError::NotPartners { .. } => {
                    (IssueCode::CommanderPair, DeckZone::Commander, None)
                }
                DeckError::OutsideColorIdentity { name, .. } => {
                    (IssueCode::ColorIdentity, DeckZone::Main, Some(name.clone()))
                }
                DeckError::TooFewCards { .. } | DeckError::TooManyCopies { .. } => {
                    (IssueCode::DeckSize, DeckZone::Main, None)
                }
            };
            // The engine reads a commander's rules from the card's definition, which a card without
            // its rules lacks; the deck check, which reads the printed text, may accept the same deck.
            let note = match other {
                DeckError::NotACommander { .. } | DeckError::NotPartners { .. } => {
                    "; the game does not run this card's commander rule yet"
                }
                _ => "",
            };
            Issue {
                code,
                message: format!("{other}{note}"),
                zone: Some(zone),
                oracle_id: None,
                name,
            }
        }
    }
}

#[cfg(test)]
mod tests {
    //! Goal: the library of a stored deck is its main deck's cards in line order with copies side by side, the table is told only the main deck's manual copies, the game's format adds its own issues to the deck check's, and a refusal reaches the wire as a 422 with its issues or a 404 that tells nothing about the deck.
    use mtg_cards::deck::{Counts, DeckState, ReportCard, Summary};
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
        let few = engine_issue(
            DeckError::TooFewCards { min: 60, got: 40 },
            FormatKind::Normal,
        );
        assert_eq!(
            (few.code, few.zone),
            (IssueCode::DeckSize, Some(DeckZone::Main))
        );
        assert!(few.message.contains("60") && few.message.contains("40"));
        let many = engine_issue(
            DeckError::TooManyCopies {
                name: "Lightning Bolt".into(),
                max: 4,
                got: 5,
            },
            FormatKind::Normal,
        );
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
