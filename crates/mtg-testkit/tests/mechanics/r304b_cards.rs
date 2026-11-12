//! Goal: pack r30-4b's cards play as printed at tables of two to four seats: Sands of Time skips every untap step and
//! flips each tapped and untapped artifact, creature and land of the player whose upkeep it is, Forbidden Crypt returns
//! a card from the graveyard instead of each draw (and loses the game with none) and exiles its owner's cards bound for
//! the graveyard, a creature enchanted by Airtight Alibi can't become suspected, and Polukranos enters with six +1/+1
//! counters when cast from a hand.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_cards::value::CounterKind;
use mtg_core::card::CardType;
use mtg_engine::replay;
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{bear, elf, forest};
use mtg_testkit::corpus::card as corpus_card;
use mtg_testkit::{Driver, Scenario, ScriptError, attached_to, ready, tapped};
use rstest::rstest;

use crate::common::pid;
use crate::macro_keywords::fight_on;

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

/// The testkit's name of the Bear.
fn bear_name(name: &str) -> String {
    if name == "Bear" {
        bear().name.clone()
    } else {
        name.to_owned()
    }
}

fn glyph() -> Arc<CardDefinition> {
    Arc::new(CardDefinition::new("Glyph").with_type(CardType::Enchantment))
}

fn is_tapped(driver: &Driver, seat: usize, name: &str) -> bool {
    driver
        .state()
        .zone(Zone::Battlefield)
        .iter()
        .filter_map(|id| driver.state().object(*id))
        .find(|object| object.card().name == name && object.controller() == pid(seat))
        .and_then(|object| object.permanent())
        .unwrap()
        .tapped()
}

fn replays(driver: &Driver) {
    let again = replay(driver.engine().setup(), driver.engine().events()).unwrap();
    assert_eq!(again.state_hash(), driver.state().state_hash());
}

#[rstest]
fn sands_of_time_skips_untap_steps_and_flips_the_upkeep_players_permanents(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut scenario = Scenario::new(seats)
        .at(1, seats - 1, Step::End)
        .battlefield(
            0,
            vec![
                ready(corpus_card("Sands of Time").unwrap()),
                tapped(forest()),
                ready(bear()),
                tapped(glyph()),
            ],
        );
    for seat in 0..seats {
        scenario = scenario.library(seat, vec![forest(), forest()]);
    }
    for seat in 1..seats {
        scenario = scenario.battlefield(seat, vec![tapped(forest()), ready(elf())]);
    }
    let mut driver = Driver::from_scenario(scenario)?;
    fight_on(&mut driver, Step::Draw)?;
    assert!(!is_tapped(&driver, 0, "Forest"), "the tapped land untaps");
    assert!(
        is_tapped(&driver, 0, &bear().name),
        "the untapped creature taps"
    );
    assert!(
        is_tapped(&driver, 0, "Glyph"),
        "the untap step is skipped: an enchantment stays tapped"
    );
    for seat in 1..seats {
        assert!(
            is_tapped(&driver, seat, "Forest"),
            "seat {seat} is not flipped"
        );
        assert!(
            !is_tapped(&driver, seat, &elf().name),
            "seat {seat} is not flipped"
        );
    }
    replays(&driver);
    Ok(())
}

#[rstest]
fn forbidden_crypt_returns_a_card_instead_of_a_draw_and_exiles_its_owners_cards(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let slay = r#"{"Spell":{"effects":[{"Destroy":{"what":{"Each":[{"Type":"Creature"}]}}}]}}"#;
    let mut scenario = Scenario::new(seats)
        .at(2, 0, Step::Upkeep)
        .battlefield(
            0,
            vec![ready(corpus_card("Forbidden Crypt").unwrap()), ready(elf())],
        )
        .graveyard(0, vec![bear()])
        .library(0, vec![forest()])
        .hand(0, vec![sorcery("Slay", slay)]);
    for seat in 1..seats {
        scenario = scenario.battlefield(seat, vec![ready(elf())]);
    }
    let mut driver = Driver::from_scenario(scenario)?;
    fight_on(&mut driver, Step::Main1)?;
    let hand = driver.state().zone(Zone::Hand(pid(0)));
    assert_eq!(hand.len(), 2, "the Bear came back instead of the draw");
    assert_eq!(driver.state().zone(Zone::Library(pid(0))).len(), 1);
    driver.cast("Slay")?;
    fight_on(&mut driver, Step::Main1)?;
    assert!(
        driver.state().zone(Zone::Graveyard(pid(0))).is_empty(),
        "the Elf and the Slay are exiled"
    );
    assert_eq!(driver.state().zone(Zone::Exile).len(), 2);
    for seat in 1..seats {
        assert_eq!(driver.state().zone(Zone::Graveyard(pid(seat))).len(), 1);
    }
    replays(&driver);
    Ok(())
}

#[rstest]
fn forbidden_crypt_loses_the_game_when_no_card_can_return(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut scenario = Scenario::new(seats)
        .at(2, 0, Step::Upkeep)
        .battlefield(0, vec![ready(corpus_card("Forbidden Crypt").unwrap())])
        .library(0, vec![forest()]);
    for seat in 1..seats {
        scenario = scenario.library(seat, vec![forest()]);
    }
    let mut driver = Driver::from_scenario(scenario)?;
    for _ in 0..400 {
        if !driver.state().is_alive(pid(0)) {
            break;
        }
        driver.answer_passively()?;
    }
    assert!(
        !driver.state().is_alive(pid(0)),
        "the draw with an empty graveyard loses"
    );
    for seat in 1..seats {
        assert!(driver.state().is_alive(pid(seat)), "seat {seat} plays on");
    }
    Ok(())
}

#[rstest]
fn airtight_alibi_keeps_its_creature_from_becoming_suspected(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let suspect = r#"{"Spell":{"effects":[{"Suspect":{"what":{"Each":[{"Type":"Creature"}]}}}]}}"#;
    let mut scenario = Scenario::new(seats)
        .battlefield(
            0,
            vec![
                ready(bear()),
                attached_to(ready(corpus_card("Airtight Alibi").unwrap()), 0, 0),
                ready(elf()),
            ],
        )
        .hand(0, vec![sorcery("Suspicion", suspect)]);
    for seat in 1..seats {
        scenario = scenario.battlefield(seat, vec![ready(elf())]);
    }
    let mut driver = Driver::from_scenario(scenario)?;
    driver.cast("Suspicion")?;
    fight_on(&mut driver, Step::Main1)?;
    let suspected = |name: &str| {
        let id = driver.id_of(&bear_name(name)).unwrap();
        driver
            .state()
            .object(id)
            .unwrap()
            .permanent()
            .unwrap()
            .suspected()
    };
    assert!(!suspected("Bear"), "the enchanted Bear is not suspected");
    let elves = driver
        .state()
        .zone(Zone::Battlefield)
        .iter()
        .filter_map(|id| driver.state().object(*id))
        .filter(|object| object.card().name == elf().name)
        .filter(|object| {
            object
                .permanent()
                .is_some_and(mtg_engine::object::PermanentState::suspected)
        })
        .count();
    assert_eq!(elves, seats, "every Elf is suspected");
    let bear = driver.id_of(&bear().name)?;
    let toughness = mtg_engine::characteristics::characteristics(driver.state(), bear)
        .unwrap()
        .toughness;
    assert_eq!(toughness, Some(4), "the Bear gets +2/+2");
    replays(&driver);
    Ok(())
}

#[rstest]
fn polukranos_enters_with_six_counters_from_a_hand(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let card = free(&corpus_card("Polukranos, Unchained").unwrap());
    let scenario = Scenario::new(seats).hand(0, vec![card]);
    let mut driver = Driver::from_scenario(scenario)?;
    driver.cast("Polukranos, Unchained")?;
    fight_on(&mut driver, Step::Main1)?;
    let id = driver.id_of("Polukranos, Unchained")?;
    let counters = driver
        .state()
        .object(id)
        .unwrap()
        .permanent()
        .unwrap()
        .counters_of(&CounterKind::PlusOnePlusOne);
    assert_eq!(counters, 6, "a cast from the hand is no escape");
    replays(&driver);
    Ok(())
}
