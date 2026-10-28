//! Goal: Laelia, the Blade Reforged gets one +1/+1 counter for each batch of its owner's cards put into exile from their library and/or graveyard, at tables of two, three and four seats: one counter for a library card alone, a graveyard alone or both at once, two for two instructions one after the other, none when no card of its owner is exiled, and every game replays to the same state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use crate::c4_control::replays;
use crate::common::removal::resolve_the_stack;
use mtg_cards::ability::Ability;
use mtg_cards::value::CounterKind;
use mtg_engine::turn::Step;
use mtg_testkit::cards::{free_creature, instant};
use mtg_testkit::corpus::card;
use mtg_testkit::trigger_cards::with_rules;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

/// A free instant that exiles what the effects say.
fn sweep(effects: &str) -> Arc<mtg_cards::CardDefinition> {
    let spell: Ability =
        serde_json::from_str(&format!(r#"{{"Spell":{{"effects":[{effects}]}}}}"#)).unwrap();
    with_rules(&instant("Sweep", 0, &[]), vec![spell])
}

const TOP: &str = r#"{"ExileTop":{"as":"t"}}"#;
const OWN_YARD: &str = r#"{"Exile":{"what":{"Each":[{"In":"Graveyard"},{"OwnedBy":"You"}]}}}"#;
const BOTH: &str = r#"{"Exile":{"what":{"Each":[{"AnyOf":[{"In":"Graveyard"},{"In":"Library"}]},{"OwnedBy":"You"}]}}}"#;
const ONE_THEN_THE_OTHER: &str = r#"{"ExileTop":{"as":"t"}},{"Exile":{"what":{"Each":[{"In":"Graveyard"},{"OwnedBy":"You"}]}}}"#;
const THEIR_YARDS: &str =
    r#"{"Exile":{"what":{"Each":[{"In":"Graveyard"},{"OwnedBy":"Opponents"}]}}}"#;

fn laelia_counters(driver: &Driver) -> u32 {
    let state = driver.state();
    state
        .objects()
        .find(|o| o.card().name == "Laelia, the Blade Reforged")
        .and_then(|o| o.permanent())
        .map_or(0, |p| p.counters_of(&CounterKind::PlusOnePlusOne))
}

#[rstest]
#[case::the_top_of_the_library(&[TOP], 1)]
#[case::the_graveyard(&[OWN_YARD], 1)]
#[case::both_at_once(&[BOTH], 1)]
#[case::one_instruction_then_another(&[ONE_THEN_THE_OTHER], 2)]
#[case::both_in_two_spells(&[TOP, OWN_YARD], 2)]
#[case::only_cards_of_the_opponents(&[THEIR_YARDS], 0)]
fn c6_gap_laelia_counts_each_batch_of_cards_exiled_from_library_and_or_graveyard(
    #[case] spells: &[&str],
    #[case] counters: u32,
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut scenario = Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .battlefield(0, [ready(card("Laelia, the Blade Reforged").unwrap())])
        .hand(0, spells.iter().map(|effects| sweep(effects)))
        .library(0, (0..3).map(|i| free_creature(&format!("Top {i}"), 1, 1)))
        .graveyard(0, [free_creature("Old Bones", 1, 1)]);
    for seat in 1..seats {
        scenario = scenario.graveyard(seat, [free_creature("Their Bones", 1, 1)]);
    }
    let mut driver = Driver::from_scenario(scenario)?;
    for _ in spells {
        driver.cast("Sweep")?;
        resolve_the_stack(&mut driver)?;
    }
    assert_eq!(laelia_counters(&driver), counters);
    replays(&driver);
    Ok(())
}
