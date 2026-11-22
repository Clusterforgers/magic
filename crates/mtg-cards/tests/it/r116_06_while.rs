//! Goal: every corpus card with a trigger worded "<event> while <condition>" holds the condition as checked only when
//! the event happens (`AtTrigger`) and reads back as its printed text, and the same card with the condition as an
//! intervening "if" (checked again on resolution) does not.
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
#[case("Bridled Bighorn")]
#[case("Giant Beaver")]
#[case("Hologram Janeway")]
#[case("Kirk, Commanding Officer")]
#[case("Pugnacious Hammerskull")]
#[case("Rambling Possum")]
#[case("Regal Behemoth")]
#[case("Una Chin-Riley, Number One")]
#[case("Worf, Honor-Bound Warrior")]
#[case("Brazen Blademaster")]
#[case("Courageous Goblin")]
#[case("Fire Lord Azula")]
#[case("Hand That Feeds")]
#[case("Kirk, Enterprising Captain")]
#[case("La'An Noonien-Singh, Security")]
#[case("Moon-Boy, Dino Rider")]
#[case("Nighthowl Pursuer")]
#[case("Ravening Warg")]
#[case("Ruby, Daring Tracker")]
#[case("Seasoned Warrenguard")]
#[case("Tactical Officer")]
#[case("The Chief Warg")]
#[case("Wilderland Scrounger")]
fn r116_06_a_while_trigger_is_checked_as_it_triggers_only(#[case] name: &str) {
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
        "{name}: an intervening \"if\" is no \"while\""
    );
}
