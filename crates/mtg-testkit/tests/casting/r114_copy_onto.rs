//! Goal: a copy of a spell aimed at a chosen player or permanent the spell could target (`Effect::R114CopySpellOnto`, CR
//! 707.10c) at two to four seats: Exterminator Magmarch copies a spell that targets only one opponent's nonland
//! permanent onto a nonland permanent of another opponent, also once the spell has left the stack, and copies nothing
//! with no other opponent or for a spell aimed at its controller's creature; Zevlor's delayed trigger copies the next such
//! spell once for each other opponent, aimed at that player or a permanent they control that the spell could target; each
//! log replays to the live state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_core::card::CardType;
use mtg_engine::decision::{Answer, PayChoice};
use mtg_engine::replay;
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{creature, plains};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ready};
use rstest::rstest;

use crate::common::copying::life;
use crate::common::spell;

const MAGMARCH: &str = "Exterminator Magmarch";
const ZEVLOR: &str = "Zevlor, Elturel Exile";

/// "Tap target creature."
fn r114_zap() -> Arc<CardDefinition> {
    spell(
        "Zap",
        CardType::Instant,
        r#"[{"what":{"Object":[{"Type":"Creature"}]}}]"#,
        r#"[{"Tap":{"what":{"Target":0}}}]"#,
    )
}

/// "Target player loses 1 life."
fn r114_drain() -> Arc<CardDefinition> {
    spell(
        "Drain",
        CardType::Instant,
        r#"[{"what":"Player"}]"#,
        r#"[{"LoseLife":{"who":{"Target":0},"amount":1}}]"#,
    )
}

/// A creature whose trigger returns each instant or sorcery spell its controller casts to its owner's hand.
fn r114_returner() -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new("Returner")
            .with_type(CardType::Creature)
            .with_power_toughness(2, 2)
            .with_rules(
                serde_json::from_str(
                    r#"{"Triggered":{"when":{"SpellCast":{"by":"You","what":[{"AnyOf":[{"Type":"Instant"},{"Type":"Sorcery"}]}]}},"effects":[{"Move":{"what":"Triggering","to":"Hand"}}]}}"#,
                )
                .unwrap(),
            ),
    )
}

/// Seat 0 is active with `copier`, a Bear, two Plains and the spells; every other seat controls an Ogre.
fn r114_table(seats: usize, copier: &str, returner: bool) -> Scenario {
    let mut own = vec![
        ready(card(copier).unwrap()),
        ready(creature("Bear", 1, &[], 2, 2)),
        ready(plains()),
        ready(plains()),
    ];
    if returner {
        own.push(ready(r114_returner()));
    }
    let mut scenario = Scenario::new(seats)
        .at(2, 0, Step::Main1)
        .hand(0, vec![r114_zap(), r114_drain()])
        .battlefield(0, own);
    for seat in 1..seats {
        scenario = scenario.battlefield(seat, [ready(creature("Ogre", 2, &[], 3, 3))]);
    }
    scenario
}

/// The tapped creatures on the battlefield (Ogres and Bears).
fn r114_tapped(driver: &Driver) -> usize {
    let state = driver.state();
    state
        .zone(Zone::Battlefield)
        .iter()
        .filter(|id| {
            state.object(**id).is_some_and(|o| {
                matches!(o.card().name.as_str(), "Ogre" | "Bear")
                    && o.permanent()
                        .is_some_and(mtg_engine::object::PermanentState::tapped)
            })
        })
        .count()
}

/// Answers every question passively until the stack is empty.
fn r114_settle(driver: &mut Driver) {
    for _ in 0..300 {
        if driver.state().stack().is_empty() && driver.state().resolving().is_none() {
            return;
        }
        driver.answer_passively().unwrap();
    }
    panic!("the stack does not settle");
}

/// The first Ogre, of seat 1.
fn r114_ogre(driver: &Driver) -> mtg_engine::choose::Target {
    driver.target_named("Ogre").unwrap()
}

fn r114_replays(driver: &Driver, setup: &mtg_engine::GameSetup) {
    let replayed = replay(setup, driver.engine().events()).unwrap();
    assert_eq!(replayed.state_hash(), driver.engine().state_hash());
}

#[rstest]
#[case::at_an_opponents_ogre(false, false)]
#[case::at_its_own_bear(true, false)]
#[case::returned_to_hand_first(false, true)]
fn r114_magmarch_copies_onto_another_opponents_permanent(
    #[values(2, 3, 4)] seats: usize,
    #[case] own: bool,
    #[case] returned: bool,
) {
    let scenario = r114_table(seats, MAGMARCH, returned);
    let setup = scenario.clone().setup().unwrap();
    let mut driver = Driver::from_scenario(scenario).unwrap();
    let aim = if own {
        driver.target_named("Bear").unwrap()
    } else {
        r114_ogre(&driver)
    };
    driver.cast_at("Zap", &[aim]).unwrap();
    r114_settle(&mut driver);
    let copied = usize::from(!own && seats > 2);
    // The spell resolves unless the Returner took it back; the copy taps one other opponent's Ogre.
    assert_eq!(r114_tapped(&driver), usize::from(!returned) + copied);
    r114_replays(&driver, &setup);
}

#[rstest]
#[case::a_creature(false)]
#[case::a_player(true)]
fn r114_zevlor_copies_for_each_other_opponent(
    #[values(2, 3, 4)] seats: usize,
    #[case] player: bool,
) {
    let scenario = r114_table(seats, ZEVLOR, false);
    let setup = scenario.clone().setup().unwrap();
    let mut driver = Driver::from_scenario(scenario).unwrap();
    driver.begin_activation(ZEVLOR).unwrap();
    driver.answer(Answer::PayMana(PayChoice::Auto)).unwrap();
    r114_settle(&mut driver);
    if player {
        let seat1 = driver.target_player(1);
        driver.cast_at("Drain", &[seat1]).unwrap();
    } else {
        let ogre = r114_ogre(&driver);
        driver.cast_at("Zap", &[ogre]).unwrap();
    }
    r114_settle(&mut driver);
    if player {
        for seat in 1..seats {
            assert_eq!(life(&driver, seat), 19, "seat {seat} loses 1 life");
        }
        assert_eq!(life(&driver, 0), 20);
    } else {
        assert_eq!(
            r114_tapped(&driver),
            seats - 1,
            "each opponent's Ogre is tapped"
        );
    }
    r114_replays(&driver, &setup);
}

#[rstest]
fn r114_zevlor_waits_for_a_spell_aimed_at_an_opponent(#[values(2, 3, 4)] seats: usize) {
    let scenario = r114_table(seats, ZEVLOR, false);
    let setup = scenario.clone().setup().unwrap();
    let mut driver = Driver::from_scenario(scenario).unwrap();
    driver.begin_activation(ZEVLOR).unwrap();
    driver.answer(Answer::PayMana(PayChoice::Auto)).unwrap();
    r114_settle(&mut driver);
    let bear = driver.target_named("Bear").unwrap();
    driver.cast_at("Zap", &[bear]).unwrap();
    r114_settle(&mut driver);
    assert_eq!(
        r114_tapped(&driver),
        1,
        "a spell at its own Bear is not copied"
    );
    let seat1 = driver.target_player(1);
    driver.cast_at("Drain", &[seat1]).unwrap();
    r114_settle(&mut driver);
    for seat in 1..seats {
        assert_eq!(
            life(&driver, seat),
            19,
            "the next spell at an opponent is copied"
        );
    }
    r114_replays(&driver, &setup);
}
