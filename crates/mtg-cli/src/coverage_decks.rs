//! `coverage-decks`: every card in any account's saved deck that the game does not play whole, the most used first,
//! with the support state the deck check gives it (`docs/deck-check.md`). Read-only; it names no account.

use std::collections::BTreeMap;
use std::fmt::Write as _;

use mtg_cards::load::{GeneratedRules, Library};
use mtg_cards::support::Support;
use mtg_cards::{DraftRules, ReadState as CardsReadState, builtin};
use mtg_db::{CardDemand, Database, DbError, ReadState};
use mtg_import::ids::OracleId;
use serde_json::json;

use crate::args::CoverageDecksArgs;
use crate::coverage::printed_card;
use crate::error::CliError;
use crate::fix_marks::support_word;
use crate::status::missing_table;

/// The longest card name of the table; a longer one is cut and ends in `...`.
const NAME_WIDTH: usize = 40;

/// One card of the saved decks as the deck check sees it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Row {
    /// The card and how the decks use it.
    pub demand: CardDemand,
    /// The support state the deck check gives the card.
    pub support: Support,
}

/// How the distinct cards of the decks divide by what the game does for them.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Totals {
    /// Distinct cards in any saved deck.
    pub cards: usize,
    /// Played whole by the game: `Verified`, `Read` or `Forge`.
    pub automatic: usize,
    /// `Partial`: some clauses by hand.
    pub partly: usize,
    /// `Manual`, `Stale` or `Unplayable`: the whole text by hand.
    pub by_hand: usize,
}

/// Whether the deck check counts a card of this state as run whole.
pub const fn is_automatic(support: Support) -> bool {
    matches!(support, Support::Verified | Support::Read | Support::Forge)
}

/// The totals over `rows`.
pub fn totals(rows: &[Row]) -> Totals {
    let mut totals = Totals {
        cards: rows.len(),
        ..Totals::default()
    };
    for row in rows {
        match row.support {
            support if is_automatic(support) => totals.automatic += 1,
            Support::Partial => totals.partly += 1,
            _ => totals.by_hand += 1,
        }
    }
    totals
}

/// The rows that are not fully automatic, in the order given (most used first).
pub fn open(rows: &[Row]) -> Vec<&Row> {
    rows.iter()
        .filter(|row| !is_automatic(row.support))
        .collect()
}

/// `text` cut to `width` characters, ending in `...` when it was longer.
fn cut(text: &str, width: usize) -> String {
    if text.chars().count() <= width {
        return text.to_owned();
    }
    let kept: String = text.chars().take(width.saturating_sub(3)).collect();
    format!("{kept}...")
}

fn total_line(totals: Totals) -> String {
    format!(
        "{} distinct cards in saved decks: {} fully automatic, {} partly by hand, {} by hand",
        totals.cards, totals.automatic, totals.partly, totals.by_hand
    )
}

/// The plain-text table of the cards still to do, then the total line.
pub fn table(rows: &[Row]) -> String {
    let open = open(rows);
    let mut text = String::new();
    if open.is_empty() {
        text.push_str("every card in the saved decks is fully automatic\n");
    } else {
        let name_width = open
            .iter()
            .map(|row| row.demand.name.chars().count().min(NAME_WIDTH))
            .max()
            .unwrap_or(0)
            .max("card".len());
        let _ = writeln!(
            text,
            "{:<36}  {:<10}  {:>5}  {:>6}  {:<name_width$}",
            "oracle id", "support", "decks", "owners", "card"
        );
        for row in open {
            let line = format!(
                "{:<36}  {:<10}  {:>5}  {:>6}  {}",
                row.demand.oracle_id.as_str(),
                support_word(row.support),
                row.demand.decks,
                row.demand.owners,
                cut(&row.demand.name, NAME_WIDTH),
            );
            let _ = writeln!(text, "{line}");
        }
    }
    text.push_str(&total_line(totals(rows)));
    text
}

/// The same as JSON: `{cards: [...], totals: {...}}`, the cards still to do only.
pub fn json_text(rows: &[Row]) -> String {
    let totals = totals(rows);
    let cards: Vec<_> = open(rows)
        .into_iter()
        .map(|row| {
            json!({
                "oracle_id": row.demand.oracle_id.as_str(),
                "name": row.demand.name,
                "support": support_word(row.support),
                "decks": row.demand.decks,
                "owners": row.demand.owners,
            })
        })
        .collect();
    json!({
        "cards": cards,
        "totals": {
            "cards": totals.cards,
            "fully_automatic": totals.automatic,
            "partly": totals.partly,
            "by_hand": totals.by_hand,
        },
    })
    .to_string()
}

fn schema(error: DbError) -> CliError {
    if missing_table(&error) {
        CliError::Config("the schema is not applied yet; `mtg-cli ingest` applies it".into())
    } else {
        CliError::Db(error)
    }
}

/// The library the server judges decks with: the embedded rules, the compiled rows and the current drafts with the
/// state of their reads (the Forge flag included), as `mtg-server`'s card source builds it.
async fn deck_check_library(db: &Database) -> Result<Library, CliError> {
    let hand_written = builtin::library().map_err(|errors| {
        CliError::Config(format!("the embedded rules files do not load:\n{errors}"))
    })?;
    let compiled: Vec<GeneratedRules> = db
        .card_rules()
        .all()
        .await
        .map_err(schema)?
        .into_iter()
        .map(|row| GeneratedRules {
            oracle_id: row.oracle_id.as_str().to_owned(),
            rules: row.rules,
        })
        .collect();
    let (base, _) = hand_written.with_generated(&compiled);
    let drafts = db.drafts().current().await.map_err(schema)?;
    let reads: BTreeMap<OracleId, mtg_db::DraftRead> = db
        .reads()
        .state()
        .await
        .map_err(schema)?
        .into_iter()
        .map(|read| (read.oracle_id.clone(), read))
        .collect();
    let rows: Vec<DraftRules> = drafts
        .into_iter()
        .map(|draft| {
            let read = reads
                .get(&draft.oracle_id)
                .filter(|read| read.read_hash == draft.read_hash);
            let state = match read.map_or(ReadState::Pending, |read| read.state) {
                ReadState::Pending => CardsReadState::Pending,
                ReadState::Approved => CardsReadState::Approved,
                ReadState::Wrong => CardsReadState::Wrong,
            };
            DraftRules {
                forge: read.is_some_and(|read| read.forge),
                proof: draft
                    .proof
                    .and_then(|proof| serde_json::from_value(proof).ok()),
                oracle_id: draft.oracle_id.as_str().to_owned(),
                read_hash: draft.read_hash,
                rules: draft.rules,
                read: state,
            }
        })
        .collect();
    Ok(base.with_drafts(&rows).0)
}

/// Every card of the saved decks with its support state, the most used first.
pub async fn rows(db: &Database) -> Result<Vec<Row>, CliError> {
    let demand = db.decks().card_demand().await.map_err(schema)?;
    let ids: Vec<OracleId> = demand.iter().map(|d| d.oracle_id.clone()).collect();
    let cards = db.cards().get_many(&ids).await?;
    let library = deck_check_library(db).await?;
    let caps = mtg_engine::caps();
    let supports: BTreeMap<String, Support> = cards
        .iter()
        .map(|card| {
            let support = library.support(&printed_card(card), &caps).support;
            (card.oracle_id.as_str().to_owned(), support)
        })
        .collect();
    Ok(demand
        .into_iter()
        .map(|demand| {
            // A card the database lacks is `unplayable` in a deck check too.
            let support = supports
                .get(demand.oracle_id.as_str())
                .copied()
                .unwrap_or(Support::Unplayable);
            Row { demand, support }
        })
        .collect())
}

/// The text of `coverage-decks`.
pub async fn coverage_decks(db: &Database, args: &CoverageDecksArgs) -> Result<String, CliError> {
    let rows = rows(db).await?;
    Ok(if args.json {
        json_text(&rows)
    } else {
        table(&rows)
    })
}

#[cfg(test)]
mod tests {
    //! Goal: the list leaves out fully automatic cards, keeps the order given, counts the totals by what the game does for the cards, and writes the same rows as JSON.
    use super::*;

    fn row(name: &str, decks: u32, support: Support) -> Row {
        Row {
            demand: CardDemand {
                oracle_id: OracleId(format!("id-{name}")),
                name: name.to_owned(),
                decks,
                owners: 1,
            },
            support,
        }
    }

    #[test]
    fn automatic_cards_are_left_out_and_every_state_is_counted_once() {
        let rows = [
            row("A", 5, Support::Verified),
            row("B", 4, Support::Read),
            row("C", 3, Support::Forge),
            row("D", 2, Support::Partial),
            row("E", 1, Support::Manual),
            row("F", 1, Support::Stale),
            row("G", 1, Support::Unplayable),
        ];
        assert_eq!(
            totals(&rows),
            Totals {
                cards: 7,
                automatic: 3,
                partly: 1,
                by_hand: 3
            }
        );
        let text = table(&rows);
        let names: Vec<&str> = text
            .lines()
            .skip(1)
            .take(4)
            .filter_map(|l| l.split_whitespace().last())
            .collect();
        assert_eq!(names, ["D", "E", "F", "G"]);
        assert!(text.ends_with(
            "7 distinct cards in saved decks: 3 fully automatic, 1 partly by hand, 3 by hand"
        ));
        let value: serde_json::Value = serde_json::from_str(&json_text(&rows)).unwrap();
        assert_eq!(value["cards"].as_array().unwrap().len(), 4);
        assert_eq!(value["totals"]["by_hand"], 3);
    }

    #[test]
    fn nothing_to_do_is_said_before_the_total() {
        let text = table(&[row("A", 1, Support::Verified)]);
        assert!(text.starts_with("every card in the saved decks is fully automatic\n1 distinct"));
    }
}
