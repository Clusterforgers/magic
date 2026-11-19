//! Goal: "the next time" shields and next-spell rules play as their cards say at tables of two to four seats: Soulfire
//! Grand Master returns only the next instant or sorcery cast from hand, Cosmic Intervention exiles the caster's dying
//! permanents and returns them at the next end step while other players' die, Storyweave and Mystic Reflection change
//! only the first entering after them, and Pyramids saves its land from one destruction only.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::ability::Ability;
use mtg_cards::definition::CardDefinition;
use mtg_core::card::CardType;
use mtg_engine::characteristics::{characteristics, controller_of};
use mtg_engine::choose::ModeChoice;
use mtg_engine::decision::{Answer, DecisionKind, PayChoice};
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_engine::{GameSetup, replay};
use mtg_testkit::cards::{forest, free_creature, island, plains};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

use crate::common::pid;

fn replays(driver: &Driver, setup: &GameSetup) {
    let replayed = replay(setup, driver.engine().events()).unwrap();
    assert_eq!(replayed.state_hash(), driver.state().state_hash());
}

/// Seat 0's main phase of turn 2 at a table of `seats`; every library holds ten creatures.
fn r109_table(seats: usize) -> Scenario {
    (0..seats).fold(
        Scenario::new(seats).at(2, 0, Step::Main1),
        |scenario, seat| {
            scenario.library(
                seat,
                (0..10).map(|i| free_creature(&format!("Pad {seat} {i}"), 1, 1)),
            )
        },
    )
}

/// Answers the decisions of the stack until seat 0 holds priority with nothing on it: the mode `mode` when one is asked,
/// `targets` when targets are, the mana paid automatically, the rest passively.
fn r109_settle(
    driver: &mut Driver,
    mode: Option<usize>,
    targets: &[mtg_engine::choose::Target],
) -> Result<(), ScriptError> {
    for _ in 0..200 {
        let decision = driver.decision()?.clone();
        match decision.kind {
            DecisionKind::Priority(_) if driver.state().stack().is_empty() => return Ok(()),
            DecisionKind::Priority(_) => {
                driver.pass()?;
            }
            DecisionKind::ChooseMode(_) if mode.is_some() => {
                driver.answer(Answer::ChooseMode(ModeChoice::Modes(vec![mode.unwrap()])))?;
            }
            DecisionKind::ChooseTargets(_) if !targets.is_empty() => {
                driver.choose_targets(targets)?;
            }
            DecisionKind::PayMana(_) => {
                driver.answer(Answer::PayMana(PayChoice::Auto))?;
            }
            _ => {
                driver.answer_passively()?;
            }
        }
    }
    Ok(())
}

/// The names of the cards in a zone.
fn r109_names(driver: &Driver, zone: Zone) -> Vec<String> {
    let state = driver.state();
    state
        .zone(zone)
        .iter()
        .filter_map(|id| state.object(*id))
        .map(|object| object.card().name.clone())
        .collect()
}

/// A free instant with one ability given as JSON.
fn r109_instant(name: &str, spell: &str) -> Arc<CardDefinition> {
    let ability: Ability = serde_json::from_str(spell).unwrap();
    Arc::new(
        CardDefinition::new(name)
            .with_type(CardType::Instant)
            .with_mana_cost(mtg_testkit::cards::cost(0, &[]))
            .with_rules(ability),
    )
}

#[rstest]
fn r109_soulfire_returns_only_the_next_instant_from_hand(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let scenario = || {
        r109_table(seats)
            .battlefield(
                0,
                std::iter::once(ready(card("Soulfire Grand Master").unwrap()))
                    .chain((0..6).map(|_| ready(island()))),
            )
            .hand(0, [card("Opt").unwrap(), card("Opt").unwrap()])
    };
    let setup = scenario().setup().unwrap();
    let mut driver = Driver::from_scenario(scenario())?;
    driver.begin_activation("Soulfire Grand Master")?;
    r109_settle(&mut driver, None, &[])?;
    driver.cast("Opt")?;
    r109_settle(&mut driver, None, &[])?;
    let hand = r109_names(&driver, Zone::Hand(pid(0)));
    assert_eq!(
        hand.iter().filter(|name| *name == "Opt").count(),
        2,
        "{hand:?}"
    );
    assert!(r109_names(&driver, Zone::Graveyard(pid(0))).is_empty());
    // The rule is spent: the second Opt goes to the graveyard.
    driver.cast("Opt")?;
    r109_settle(&mut driver, None, &[])?;
    assert_eq!(r109_names(&driver, Zone::Graveyard(pid(0))), vec!["Opt"]);
    assert_eq!(
        r109_names(&driver, Zone::Hand(pid(0)))
            .iter()
            .filter(|name| *name == "Opt")
            .count(),
        1
    );
    replays(&driver, &setup);
    Ok(())
}

#[rstest]
fn r109_cosmic_intervention_returns_the_casters_permanents_at_the_end_step(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let scenario = || {
        (0..seats).fold(
            r109_table(seats).hand(
                0,
                [
                    card("Cosmic Intervention").unwrap(),
                    card("Wrath of God").unwrap(),
                ],
            ),
            |scenario, seat| {
                let lands = if seat == 0 { 8 } else { 0 };
                scenario.battlefield(
                    seat,
                    std::iter::once(ready(free_creature(&format!("Bear {seat}"), 2, 2)))
                        .chain((0..lands).map(|_| ready(plains()))),
                )
            },
        )
    };
    let setup = scenario().setup().unwrap();
    let mut driver = Driver::from_scenario(scenario())?;
    driver.cast("Cosmic Intervention")?;
    r109_settle(&mut driver, None, &[])?;
    driver.cast("Wrath of God")?;
    r109_settle(&mut driver, None, &[])?;
    assert_eq!(r109_names(&driver, Zone::Exile), vec!["Bear 0"]);
    for seat in 1..seats {
        assert_eq!(
            r109_names(&driver, Zone::Graveyard(pid(seat))),
            vec![format!("Bear {seat}")]
        );
    }
    driver.pass_until(Step::End)?;
    r109_settle(&mut driver, None, &[])?;
    let field = r109_names(&driver, Zone::Battlefield);
    assert!(field.contains(&"Bear 0".to_owned()), "{field:?}");
    assert!(r109_names(&driver, Zone::Exile).is_empty());
    replays(&driver, &setup);
    Ok(())
}

/// A free enchantment creature.
fn r109_enchantment_creature(name: &str) -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new(name)
            .with_type(CardType::Enchantment)
            .with_type(CardType::Creature)
            .with_mana_cost(mtg_testkit::cards::cost(0, &[]))
            .with_power_toughness(1, 1),
    )
}

#[rstest]
fn r109_storyweave_adds_counters_to_the_next_entering_only(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let scenario = || {
        r109_table(seats)
            .battlefield(
                0,
                std::iter::once(ready(card("History of Benalia").unwrap()))
                    .chain((0..3).map(|_| ready(forest()))),
            )
            .hand(
                0,
                [
                    card("Storyweave").unwrap(),
                    free_creature("Plain Bear", 2, 2),
                    r109_enchantment_creature("Nymph A"),
                    r109_enchantment_creature("Nymph B"),
                ],
            )
    };
    let setup = scenario().setup().unwrap();
    let mut driver = Driver::from_scenario(scenario())?;
    driver.begin_cast("Storyweave")?;
    let saga = driver.target_named("History of Benalia")?;
    r109_settle(&mut driver, Some(1), &[saga])?;
    let counters = |driver: &Driver, name: &str| {
        let id = driver.id_of(name).unwrap();
        characteristics(driver.state(), id).unwrap().power.unwrap()
    };
    // A creature that is no enchantment does not use the shield up.
    driver.cast("Plain Bear")?;
    r109_settle(&mut driver, None, &[])?;
    assert_eq!(counters(&driver, "Plain Bear"), 2);
    driver.cast("Nymph A")?;
    r109_settle(&mut driver, None, &[])?;
    assert_eq!(counters(&driver, "Nymph A"), 3);
    driver.cast("Nymph B")?;
    r109_settle(&mut driver, None, &[])?;
    assert_eq!(counters(&driver, "Nymph B"), 1);
    replays(&driver, &setup);
    Ok(())
}

#[rstest]
fn r109_mystic_reflection_copies_the_chosen_creature_once(
    #[values(2, 3, 4)] seats: usize,
    #[values(false, true)] gone: bool,
) -> Result<(), ScriptError> {
    let last = seats - 1;
    let scenario = || {
        r109_table(seats)
            .battlefield(0, (0..2).map(|_| ready(island())))
            .battlefield(last, [ready(free_creature("Giant", 5, 5))])
            .hand(
                0,
                [
                    card("Mystic Reflection").unwrap(),
                    free_creature("Pad A", 1, 1),
                    free_creature("Pad B", 1, 1),
                    r109_instant(
                        "Slay",
                        r#"{"Spell":{"targets":[{"what":{"Object":[{"Type":"Creature"}]}}],"effects":[{"Destroy":{"what":{"Target":0}}}]}}"#,
                    ),
                ],
            )
    };
    let setup = scenario().setup().unwrap();
    let mut driver = Driver::from_scenario(scenario())?;
    driver.begin_cast("Mystic Reflection")?;
    let giant = driver.target_named("Giant")?;
    r109_settle(&mut driver, None, &[giant])?;
    // Gone, the creature is copied as it last existed on the battlefield.
    if gone {
        driver.begin_cast("Slay")?;
        r109_settle(&mut driver, None, &[giant])?;
        assert!(!r109_names(&driver, Zone::Battlefield).contains(&"Giant".to_owned()));
    }
    driver.cast("Pad A")?;
    r109_settle(&mut driver, None, &[])?;
    // The powers of seat 0's creatures: a copy takes the copied name, so it is found by what it is.
    let powers = |driver: &Driver| {
        let state = driver.state();
        let mut powers: Vec<i32> = state
            .zone(Zone::Battlefield)
            .iter()
            .filter(|id| controller_of(state, **id) == Some(pid(0)))
            .filter_map(|id| characteristics(state, *id))
            .filter_map(|chars| chars.power)
            .collect();
        powers.sort_unstable();
        powers
    };
    assert_eq!(powers(&driver), vec![5]);
    driver.cast("Pad B")?;
    r109_settle(&mut driver, None, &[])?;
    assert_eq!(powers(&driver), vec![1, 5]);
    replays(&driver, &setup);
    Ok(())
}

#[rstest]
fn r109_pyramids_saves_its_land_from_the_next_destruction(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let raze = || {
        r109_instant(
            "Raze",
            r#"{"Spell":{"targets":[{"what":{"Object":[{"Type":"Land"}]}}],"effects":[{"Destroy":{"what":{"Target":0}}}]}}"#,
        )
    };
    let scenario = || {
        r109_table(seats)
            .battlefield(
                0,
                std::iter::once(ready(card("Pyramids").unwrap()))
                    .chain((0..2).map(|_| ready(island()))),
            )
            .battlefield(seats - 1, [ready(plains())])
            .hand(0, [raze(), raze()])
    };
    let setup = scenario().setup().unwrap();
    let mut driver = Driver::from_scenario(scenario())?;
    let plains = driver.target_named("Plains")?;
    driver.begin_activation("Pyramids")?;
    r109_settle(&mut driver, Some(1), &[plains])?;
    driver.begin_cast("Raze")?;
    r109_settle(&mut driver, None, &[plains])?;
    assert!(r109_names(&driver, Zone::Battlefield).contains(&"Plains".to_owned()));
    driver.begin_cast("Raze")?;
    r109_settle(&mut driver, None, &[plains])?;
    assert!(!r109_names(&driver, Zone::Battlefield).contains(&"Plains".to_owned()));
    assert_eq!(
        r109_names(&driver, Zone::Graveyard(pid(seats - 1))),
        vec!["Plains"]
    );
    replays(&driver, &setup);
    Ok(())
}
