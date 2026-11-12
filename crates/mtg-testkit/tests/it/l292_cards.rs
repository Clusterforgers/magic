//! Goal: pack l29-2's cards play as printed at tables of two to four seats: Hypnox exiles one target opponent's whole hand and gives it back when it leaves, and "each opponent skips their next untap step" (Brine Elemental) makes every opponent skip one untap step, not only the first, and Crazed Firecat gets one +1/+1 counter for each coin flip it won.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_core::card::CardType;
use mtg_engine::choose::Target;
use mtg_engine::replay;
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{forest, free_creature};
use mtg_testkit::corpus::card as corpus_card;
use mtg_testkit::{Driver, Scenario, ScriptError, tapped};
use rstest::rstest;

use crate::common::pid;
use crate::macro_keywords::fight_on;

const SLAY: &str = r#"{"Spell":{"targets":[{"what":{"Object":[{"Type":"Creature"}]}}],"effects":[{"Destroy":{"what":{"Target":0}}}]}}"#;

/// The card with its mana cost made {0}.
fn free(card: &Arc<CardDefinition>) -> Arc<CardDefinition> {
    let mut def = (**card).clone();
    def.mana_cost = Some(mtg_core::mana::ManaCost::parse("{0}").unwrap());
    Arc::new(def)
}

fn sorcery(name: &str, rules: &str) -> Arc<CardDefinition> {
    let mut def = CardDefinition::new(name).with_type(CardType::Sorcery);
    def.mana_cost = Some(mtg_core::mana::ManaCost::parse("{0}").unwrap());
    def.rules = vec![Arc::new(serde_json::from_str::<Ability>(rules).unwrap())];
    Arc::new(def)
}

fn hand_size(driver: &Driver, seat: usize) -> usize {
    driver.state().zone(Zone::Hand(pid(seat))).len()
}

#[rstest]
fn hypnox_exiles_a_hand_and_gives_it_back_when_it_leaves(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut scenario = Scenario::new(seats).hand(
        0,
        vec![free(&corpus_card("Hypnox").unwrap()), sorcery("Slay", SLAY)],
    );
    for seat in 1..seats {
        scenario = scenario.hand(seat, vec![forest(), forest()]);
    }
    let mut driver = Driver::from_scenario(scenario)?;
    driver.cast("Hypnox")?;
    fight_on(&mut driver, Step::Main1)?;
    let emptied: Vec<usize> = (1..seats)
        .filter(|seat| hand_size(&driver, *seat) == 0)
        .collect();
    assert_eq!(emptied.len(), 1, "one target opponent's hand is exiled");
    let hypnox = driver.id_of("Hypnox")?;
    let named = driver.state().object(hypnox).unwrap().reference();
    driver.cast_at("Slay", &[Target::Object(named)])?;
    fight_on(&mut driver, Step::Main1)?;
    for seat in 1..seats {
        assert_eq!(hand_size(&driver, seat), 2, "seat {seat} has its hand back");
    }
    let again = replay(driver.engine().setup(), driver.engine().events()).unwrap();
    assert_eq!(again.state_hash(), driver.state().state_hash());
    Ok(())
}

/// "When ~ enters, each opponent skips their next untap step." as Brine Elemental's file writes it.
const EACH_SKIPS: &str = r#"{"Triggered":{"when":{"ZoneChange":{"what":["This"],"to":"Battlefield"}},"effects":[{"RepeatEach":{"over":"Opponents","as":"foe","effects":[{"Rule":{"rule":{"Cant":{"who":{"Bound":"foe"},"action":{"SkipStep":"Untap"}}},"duration":"UntilSkipped"}}]}}]}}"#;

#[rstest]
fn each_opponent_skips_their_own_next_untap_step(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut skipper = (*free_creature("Skipper", 1, 1)).clone();
    skipper.rules = vec![Arc::new(
        serde_json::from_str::<Ability>(EACH_SKIPS).unwrap(),
    )];
    let mut scenario = Scenario::new(seats).hand(0, vec![Arc::new(skipper)]);
    for seat in 0..seats {
        scenario = scenario
            .library(seat, (0..12).map(|_| forest()))
            .battlefield(
                seat,
                vec![tapped(free_creature(&format!("Held {seat}"), 1, 1))],
            );
    }
    let mut driver = Driver::from_scenario(scenario)?;
    driver.cast("Skipper")?;
    fight_on(&mut driver, Step::Main1)?;
    let is_tapped = |driver: &Driver, seat: usize| {
        let id = driver.id_of(&format!("Held {seat}")).unwrap();
        driver
            .state()
            .object(id)
            .unwrap()
            .permanent()
            .unwrap()
            .tapped()
    };
    let turns = u32::try_from(seats).unwrap();
    for seat in 1..seats {
        driver.idle_until(1 + u32::try_from(seat).unwrap(), Step::Main1)?;
        assert!(
            is_tapped(&driver, seat),
            "seat {seat} skipped its untap step"
        );
    }
    driver.idle_until(turns + 1, Step::Main1)?;
    assert!(!is_tapped(&driver, 0), "the controller untaps");
    for seat in 1..seats {
        driver.idle_until(turns + 1 + u32::try_from(seat).unwrap(), Step::Main1)?;
        assert!(
            !is_tapped(&driver, seat),
            "seat {seat} untaps on its next turn"
        );
    }
    Ok(())
}

#[rstest]
fn crazed_firecat_gets_a_counter_for_each_flip_won(
    #[values(2, 3, 4)] seats: usize,
    #[values(1, 2, 3, 5, 8, 13)] seed: u64,
) -> Result<(), ScriptError> {
    let scenario = Scenario::new(seats)
        .seed(seed)
        .hand(0, vec![free(&corpus_card("Crazed Firecat").unwrap())]);
    let mut driver = Driver::from_scenario(scenario)?;
    driver.cast("Crazed Firecat")?;
    fight_on(&mut driver, Step::Main1)?;
    let flips: Vec<bool> = driver
        .engine()
        .events()
        .iter()
        .filter_map(|event| match event {
            mtg_engine::events::Event::CoinFlipped { won, .. } => Some(*won),
            _ => None,
        })
        .collect();
    assert_eq!(flips.last(), Some(&false), "the flips end with a loss");
    let won = flips.iter().filter(|won| **won).count();
    let cat = driver.id_of("Crazed Firecat")?;
    let counters = driver
        .state()
        .object(cat)
        .unwrap()
        .permanent()
        .unwrap()
        .counters_of(&mtg_cards::value::CounterKind::PlusOnePlusOne);
    assert_eq!(counters, u32::try_from(won).unwrap());
    Ok(())
}
