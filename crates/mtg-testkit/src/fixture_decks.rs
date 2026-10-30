//! Whole decks from deck lists and Oracle records saved in `tests/fixtures/decks/`, read the way the server reads a deck.
//!
//! A list (`<name>.arena.txt`) is parsed and resolved against the records of `cards.json` by `mtg-import`; each distinct
//! card gets the rules file of this repository, or else the rules the compiler writes for it (checked as the `compile`
//! command checks them before it stores them), and is assembled for the capabilities of this engine, so a game plays
//! what the server would be given and [`FixtureDeck::statuses`] says which cards are `Verified`, `Partial` or `Manual`.
//! The records are real Oracle Cards records from the Scryfall cache, never typed by hand.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::{Path, PathBuf};
use std::sync::{Arc, OnceLock};

use mtg_cards::generated::{Generation, generate};
use mtg_cards::load::{GeneratedRules, Library};
use mtg_cards::support::Support;
use mtg_cards::{CardDefinition, PrintedCard, builtin};
use mtg_engine::GameSetup;
use mtg_engine::format::Commander;
use mtg_import::decklist::{self, Section};
use mtg_import::resolve::{CardIndex, resolve_deck};
use mtg_import::scryfall::{CardRecord, cards};

use crate::corpus::{CorpusError, printed_card};

/// The directory of the deck lists and `cards.json`.
pub fn deck_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/decks")
}

/// Where a card's rules came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RulesSource {
    /// A rules file of `mtg-cards/cards/`.
    HandWritten,
    /// Rules the compiler wrote and the checks accepted.
    Compiled,
    /// No rules: the compiler read nothing of the card, or what it wrote failed a check.
    None,
}

/// One distinct card of a deck and how the engine plays it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CardStatus {
    /// The card's name.
    pub name: String,
    /// Copies in the deck (the commander included).
    pub copies: u32,
    /// Where its rules came from.
    pub source: RulesSource,
    /// How well the engine plays it.
    pub support: Support,
    /// The clauses left to the table with their text, or why the card has no rules.
    pub reasons: Vec<String>,
}

/// A deck as the engine plays it.
#[derive(Debug, Clone)]
pub struct FixtureDeck {
    /// The name of the list file without its extension.
    pub name: String,
    /// The commanders, to start in the command zone.
    pub commanders: Vec<Arc<CardDefinition>>,
    /// The other cards, a definition for each copy in list order.
    pub cards: Vec<Arc<CardDefinition>>,
    /// Each distinct card in list order, the commanders first.
    pub statuses: Vec<CardStatus>,
}

impl FixtureDeck {
    /// The cards that are not `Verified`.
    pub fn not_verified(&self) -> impl Iterator<Item = &CardStatus> {
        self.statuses
            .iter()
            .filter(|status| status.support != Support::Verified)
    }
}

/// A Commander game with these decks, one seat each, every commander in its command zone.
pub fn commander_game(decks: &[&FixtureDeck], seed: u64) -> GameSetup {
    GameSetup::new(
        Arc::new(Commander),
        seed,
        decks.iter().map(|deck| deck.cards.clone()).collect(),
    )
    .with_commanders(decks.iter().map(|deck| deck.commanders.clone()).collect())
}

/// The records and the rules every fixture deck reads: assembled once.
struct Records {
    list: Vec<CardRecord>,
    by_oracle: HashMap<String, usize>,
    index: CardIndex,
    library: Library,
    /// The oracle ids whose compiled rules failed a check, with the first problem.
    invalid: BTreeMap<String, String>,
}

fn data(error: impl std::fmt::Display) -> CorpusError {
    CorpusError::Data(error.to_string())
}

fn records() -> Result<&'static Records, CorpusError> {
    static RECORDS: OnceLock<Result<Records, CorpusError>> = OnceLock::new();
    RECORDS
        .get_or_init(read_records)
        .as_ref()
        .map_err(Clone::clone)
}

fn read_records() -> Result<Records, CorpusError> {
    let path = deck_dir().join("cards.json");
    let bytes =
        std::fs::read(&path).map_err(|error| data(format!("{}: {error}", path.display())))?;
    let list = cards(bytes.as_slice())
        .collect::<Result<Vec<_>, _>>()
        .map_err(data)?;
    let hand_written = builtin::library().map_err(data)?;
    let printed: Vec<PrintedCard> = list.iter().map(printed_card).collect();
    let names = names_of(&printed);
    let mut rows = Vec::new();
    let mut invalid = BTreeMap::new();
    for card in &printed {
        if hand_written.card(&card.oracle_id).is_some() {
            continue;
        }
        match generate(card, hand_written, &names) {
            Generation::Rules(rules) => rows.push(GeneratedRules {
                oracle_id: card.oracle_id.clone(),
                rules: serde_json::to_value(&*rules).map_err(data)?,
            }),
            Generation::Invalid(problems) => {
                let first = problems.first().map(ToString::to_string);
                invalid.insert(card.oracle_id.clone(), first.unwrap_or_default());
            }
            Generation::Nothing => {}
        }
    }
    let (library, left_out) = hand_written.with_generated(&rows);
    if let Some(error) = left_out.first() {
        return Err(data(error));
    }
    Ok(Records {
        by_oracle: list
            .iter()
            .enumerate()
            .map(|(at, record)| (record.oracle_id.to_string(), at))
            .collect(),
        index: CardIndex::from_records(&list),
        list,
        library,
        invalid,
    })
}

/// The lowercase names of the cards and their faces: what a rule may name (as `mtg-cli compile` collects them).
fn names_of(printed: &[PrintedCard]) -> BTreeSet<String> {
    printed
        .iter()
        .flat_map(|card| {
            std::iter::once(&card.name).chain(card.faces.iter().map(|face| &face.name))
        })
        .map(|name| name.to_lowercase())
        .collect()
}

/// The deck of the list `<name>.arena.txt`: every line resolved to a record and assembled for this engine.
pub fn fixture_deck(name: &str) -> Result<FixtureDeck, CorpusError> {
    let path = deck_dir().join(format!("{name}.arena.txt"));
    let text = std::fs::read_to_string(&path)
        .map_err(|error| data(format!("{}: {error}", path.display())))?;
    let records = records()?;
    let parsed = decklist::parse(&text);
    if let Some(error) = parsed.errors.first() {
        return Err(data(format!("{name}: {error:?}")));
    }
    let resolution = resolve_deck(&records.index, &parsed);
    if let Some(entry) = resolution.unresolved.first() {
        return Err(data(format!(
            "{name}: {} is not resolved",
            entry.entry.name
        )));
    }
    let caps = mtg_engine::caps();
    let mut deck = FixtureDeck {
        name: name.to_owned(),
        commanders: Vec::new(),
        cards: Vec::new(),
        statuses: Vec::new(),
    };
    let mut made: HashMap<String, Arc<CardDefinition>> = HashMap::new();
    let mut ordered = resolution.resolved.clone();
    ordered.sort_by_key(|entry| entry.section != Section::Commander);
    for entry in &ordered {
        let id = entry.oracle_id.to_string();
        let record = records
            .by_oracle
            .get(&id)
            .and_then(|&at| records.list.get(at))
            .ok_or_else(|| data(format!("{name}: no record for {}", entry.name)))?;
        let printed = printed_card(record);
        let assembled = records.library.assemble(&printed, &caps);
        let definition = made
            .entry(id.clone())
            .or_insert_with(|| Arc::new(assembled.definition.clone()))
            .clone();
        for _ in 0..entry.count {
            if entry.section == Section::Commander {
                deck.commanders.push(Arc::clone(&definition));
            } else {
                deck.cards.push(Arc::clone(&definition));
            }
        }
        deck.statuses.push(CardStatus {
            name: record.name.clone(),
            copies: entry.count,
            source: source_of(records, &id),
            support: assembled.support.support,
            reasons: reasons_of(records, &printed, &assembled.support),
        });
    }
    Ok(deck)
}

fn source_of(records: &Records, oracle_id: &str) -> RulesSource {
    match records.library.file_of(oracle_id) {
        Some(file) if file.starts_with("card_rules/") => RulesSource::Compiled,
        Some(_) => RulesSource::HandWritten,
        None => RulesSource::None,
    }
}

/// The clauses left to the table with their text, or why the card has no rules at all.
fn reasons_of(
    records: &Records,
    printed: &PrintedCard,
    support: &mtg_cards::support::SupportRecord,
) -> Vec<String> {
    let mut reasons: Vec<String> = support
        .clauses
        .iter()
        .map(|clause| {
            let text = printed
                .faces
                .get(clause.face)
                .and_then(|face| clause.span.slice(&face.oracle_text))
                .unwrap_or("");
            format!("{} ({})", clause.reason, text.replace('\n', " / "))
        })
        .collect();
    if reasons.is_empty() && support.support != Support::Verified {
        reasons.push(match records.invalid.get(&support.oracle_id) {
            Some(problem) => format!("the compiled rules failed a check: {problem}"),
            None if support.support == Support::Stale => {
                "the rules file was written for other oracle text".to_owned()
            }
            None => "no rules: the compiler reads nothing of the text".to_owned(),
        });
    }
    reasons
}

#[cfg(test)]
mod tests {
    //! Goal: the owner's list resolves whole (a commander and 99 other cards, every line a record) and its basic lands play as printed.
    use super::*;

    #[test]
    fn the_owners_list_resolves_to_a_commander_and_ninety_nine_cards() {
        let deck = fixture_deck("owner").unwrap();
        assert_eq!(deck.commanders.len(), 1);
        assert_eq!(deck.commanders[0].name, "Y'shtola, Night's Blessed");
        assert_eq!(deck.cards.len(), 99);
        let islands = deck
            .cards
            .iter()
            .filter(|card| card.name == "Island")
            .count();
        assert_eq!(islands, 4);
    }
}
