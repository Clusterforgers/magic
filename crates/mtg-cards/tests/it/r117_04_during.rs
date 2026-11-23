//! Goal: every corpus card with a trigger worded "<event> during your turn" or "during an opponent's turn" holds the turn
//! as checked only when the event happens (`AtTrigger`) and reads back as its printed text, and the same card with the
//! turn as an intervening "if" (checked again on resolution) does not.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::collections::BTreeSet;

use crate::common;

use mtg_cards::Caps;
use mtg_cards::load::{Library, slug};
use mtg_cards::proposal::{Verdict, check};
use mtg_cards::support::PrintedCard;
use rstest::rstest;

fn names(cards: &[PrintedCard]) -> BTreeSet<String> {
    cards
        .iter()
        .flat_map(|card| std::iter::once(&card.name).chain(card.faces.iter().map(|f| &f.name)))
        .map(|name| name.to_lowercase())
        .collect()
}

fn verdict_of(name: &str, text: &str) -> Verdict {
    let cards = common::printed();
    let tokens = Library::from_sources(&common::token_sources()).unwrap();
    let card = cards
        .iter()
        .find(|card| card.name == name)
        .unwrap_or_else(|| panic!("the corpus has {name}"));
    check(text, card, &tokens, &names(&cards), &Caps::everything()).verdict()
}

/// The rules file of a corpus card as a JSON value.
fn rules(name: &str) -> serde_json::Value {
    let path = common::cards_dir().join(format!("{}.json", slug(name)));
    serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap()
}

/// Collects the `if` of every triggered ability in the value; with `unwrap`, takes each out of its `AtTrigger`.
fn conditions(value: &mut serde_json::Value, out: &mut Vec<serde_json::Value>, unwrap: bool) {
    match value {
        serde_json::Value::Array(items) => {
            for item in items {
                conditions(item, out, unwrap);
            }
        }
        serde_json::Value::Object(map) => {
            if let Some(held) = map.get_mut("Triggered").and_then(|t| t.get_mut("if")) {
                out.push(held.clone());
                if unwrap && let Some(inner) = held.get("AtTrigger").cloned() {
                    *held = inner;
                }
            }
            for item in map.values_mut() {
                conditions(item, out, unwrap);
            }
        }
        _ => {}
    }
}

#[rstest]
#[case("Alela, Cunning Conqueror")]
#[case("Breath of the Sleepless")]
#[case("Kefka, Court Mage // Kefka, Ruler of Ruin")]
#[case("Ketramose, the New Dawn")]
#[case("Kishla Skimmer")]
#[case("Nymris, Oona's Trickster")]
#[case("Oni-Cult Anvil")]
#[case("Starfleet Security")]
#[case("Szarel, Genesis Shepherd")]
#[case("Troi, Winging It")]
#[case("Wavebreak Hippocamp")]
fn r117_04_a_during_trigger_is_checked_as_it_triggers_only(#[case] name: &str) {
    let mut card = rules(name);
    let mut found = Vec::new();
    conditions(&mut card, &mut found, false);
    assert!(
        !found.is_empty() && found.iter().all(|held| held.get("AtTrigger").is_some()),
        "{name}: {found:?}"
    );
    assert_eq!(
        verdict_of(name, &serde_json::to_string(&card).unwrap()),
        Verdict::Accepted,
        "{name}"
    );
    let mut plain = rules(name);
    conditions(&mut plain, &mut Vec::new(), true);
    assert_ne!(
        verdict_of(name, &serde_json::to_string(&plain).unwrap()),
        Verdict::Accepted,
        "{name}: an intervening \"if\" is no \"during\""
    );
}
