//! Goal: mutate (CR 702.140, 729) is an alternative cost from the hand or the command zone that casts a creature spell targeting a non-Human creature with the same owner; it is offered only with such a creature and the mana, the controller puts the spell over or under the creature as it resolves, the result is the same object with the characteristics of the top card and the abilities of both (counters stay, "whenever this creature mutates" triggers), a target that is gone makes the spell resolve as a creature spell, and when the merged permanent leaves the battlefield each card is an object of its own in the new zone, a commander card going on to the command zone alone; at 2 to 4 seats.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use crate::common::casting::{choose_way, pay, ways};
use crate::common::{names, pid};
use mtg_cards::CardDefinition;
use mtg_cards::value::CounterKind;
use mtg_core::card::CardType;
use mtg_core::ids::ObjectId;
use mtg_engine::choose::YesNoChoice;
use mtg_engine::decision::{Answer, DecisionKind};
use mtg_engine::format::Commander;
use mtg_engine::zones::Zone;
use mtg_engine::{Event, GameState, PresetPermanent, characteristics, replay};
use mtg_testkit::cards::{bear, forest, free_creature, island, swamp};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, check_state, ready, with_counters};
use rstest::rstest;

const GNARR: &str = "Trumpeting Gnarr";
const BEARS: &str = "Grizzly Bears";

fn human() -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new("Village Peasant")
            .with_type(CardType::Creature)
            .with_subtype("Human")
            .with_power_toughness(1, 1),
    )
}

/// The lands for the mutate cost of the Gnarr, {3}{G/U}{G/U}: a Forest, an Island, then Forests and Islands, `count` in all.
fn lands(count: usize) -> Vec<PresetPermanent> {
    (0..count)
        .map(|i| {
            ready(if i % 2 == 0 || i > 3 {
                forest()
            } else {
                island()
            })
        })
        .collect()
}

/// Seat 0 holds the Gnarr and Doom Blade with `count` lands and two Swamps, and `mine` creatures; seat 1 holds Doom Blade
/// with two Swamps and `theirs`. Seats beyond the second have a library.
fn table(
    seats: usize,
    count: usize,
    mine: Vec<PresetPermanent>,
    theirs: Vec<PresetPermanent>,
) -> Scenario {
    let mut battlefield = lands(count);
    battlefield.extend([ready(swamp()), ready(swamp())]);
    battlefield.extend(mine);
    let mut other = vec![ready(swamp()), ready(swamp())];
    other.extend(theirs);
    let scenario = Scenario::new(seats)
        .hand(0, [card(GNARR).unwrap(), card("Doom Blade").unwrap()])
        .hand(1, [card("Doom Blade").unwrap()])
        .battlefield(0, battlefield)
        .battlefield(1, other);
    (2..seats).fold(scenario, |scenario, seat| {
        scenario.library(seat, (0..8).map(|_| forest()))
    })
}

fn in_hand(state: &GameState, seat: usize, name: &str) -> ObjectId {
    state
        .zone(Zone::Hand(pid(seat)))
        .iter()
        .copied()
        .find(|id| state.object(*id).unwrap().card().name == name)
        .unwrap()
}

/// Seat 0 casts the Gnarr for its mutate cost at the creature `host` and pays, leaving the spell on the stack.
fn cast_mutating(driver: &mut Driver, host: &str) -> Result<(), ScriptError> {
    let target = driver.target_named(host)?;
    driver.begin_cast(GNARR)?;
    choose_way(driver, "Mutate")?;
    driver.choose_targets(&[target])?;
    pay(driver)?;
    Ok(())
}

/// Passes and answers passively until the stack is empty, answering the question of a mutating spell with `over`; true if
/// it was asked.
fn settle(driver: &mut Driver, over: bool) -> Result<bool, ScriptError> {
    let mut asked = false;
    for _ in 0..300 {
        if driver.state().stack().is_empty() && driver.state().resolving().is_none() {
            return Ok(asked);
        }
        match &driver.decision()?.kind {
            DecisionKind::YesNo(options) if options.text.starts_with("Put ") => {
                asked = true;
                driver.answer(Answer::YesNo(YesNoChoice(over)))?;
            }
            _ => {
                driver.answer_passively()?;
            }
        }
    }
    Err(ScriptError::Stuck("the stack".into()))
}

/// Seat 1 answers the mutating spell with Doom Blade at `victim`.
fn blade_in_response(driver: &mut Driver, victim: &str) -> Result<(), ScriptError> {
    driver.pass()?;
    let target = driver.target_named(victim)?;
    driver.cast_at("Doom Blade", &[target])?;
    Ok(())
}

/// Seat 0 casts its own Doom Blade at `victim` and lets it resolve.
fn blade(driver: &mut Driver, victim: &str) -> Result<(), ScriptError> {
    let target = driver.target_named(victim)?;
    driver.cast_at("Doom Blade", &[target])?;
    settle(driver, true)?;
    Ok(())
}

fn stats(state: &GameState, id: ObjectId) -> (i32, i32) {
    let chars = characteristics(state, id).unwrap();
    (chars.power.unwrap(), chars.toughness.unwrap())
}

fn top_down(state: &GameState, id: ObjectId) -> Vec<String> {
    let object = state.object(id).unwrap();
    object
        .merged()
        .unwrap()
        .from_the_top()
        .iter()
        .map(|part| part.card().name.clone())
        .collect()
}

fn replays(driver: &Driver, seats: usize, scenario: &Scenario) {
    let setup = scenario.clone().setup().unwrap();
    let replayed = replay(&setup, driver.engine().events()).unwrap();
    assert_eq!(
        replayed.state_hash(),
        driver.state().state_hash(),
        "{seats}"
    );
}

#[rstest]
#[case::own_bear("bear", 5, &["Cast", "Mutate"])]
#[case::own_bear_too_little_mana_to_mutate("bear", 2, &[])]
#[case::only_a_human("human", 5, &[])]
#[case::only_the_opponents_creature("theirs", 5, &[])]
#[case::no_creature("none", 5, &[])]
fn mutate_is_offered_beside_the_mana_cost_only_with_a_non_human_creature_you_own_and_the_mana(
    #[values(2, 3, 4)] seats: usize,
    #[case] board: &str,
    #[case] count: usize,
    #[case] offered: &[&str],
) -> Result<(), ScriptError> {
    let (mine, theirs) = match board {
        "bear" => (vec![ready(bear())], vec![]),
        "human" => (vec![ready(human())], vec![]),
        "theirs" => (vec![], vec![ready(free_creature("Ogre", 3, 4))]),
        _ => (vec![], vec![]),
    };
    let mut driver = Driver::from_scenario(table(seats, count, mine, theirs))?;
    driver.begin_cast(GNARR)?;
    if offered.is_empty() {
        // Only the mana cost is left, so nothing is asked but the payment.
        assert!(matches!(driver.decision()?.kind, DecisionKind::PayMana(_)));
    } else {
        let labels = ways(&driver)?;
        let words: Vec<_> = labels
            .iter()
            .map(|label| label.split(' ').next().unwrap())
            .collect();
        assert_eq!(words, offered);
    }
    Ok(())
}

#[rstest]
fn the_spell_goes_over_or_under_the_creature_and_the_result_is_the_same_object_with_both_cards_abilities(
    #[values(2, 3, 4)] seats: usize,
    #[values(true, false)] over: bool,
) -> Result<(), ScriptError> {
    let scenario = table(
        seats,
        5,
        vec![with_counters(ready(bear()), CounterKind::PlusOnePlusOne, 1)],
        vec![],
    );
    let mut driver = Driver::from_scenario(scenario.clone())?;
    let bear_id = driver.id_of(BEARS)?;
    let spell = in_hand(driver.state(), 0, GNARR);
    cast_mutating(&mut driver, BEARS)?;
    assert!(settle(&mut driver, over)?, "the controller is asked");
    let state = driver.state();
    // One object: the spell's card is part of the permanent and no object of the game.
    assert!(state.object(spell).is_none());
    assert_eq!(
        names(state, Zone::Battlefield)
            .iter()
            .filter(|name| *name == BEARS || *name == GNARR)
            .count(),
        1
    );
    assert_eq!(
        top_down(state, bear_id),
        if over { [GNARR, BEARS] } else { [BEARS, GNARR] }
    );
    // The characteristics of the top card, with the counter the permanent kept.
    let (power, toughness) = if over { (3, 3) } else { (2, 2) };
    assert_eq!(stats(state, bear_id), (power + 1, toughness + 1));
    assert_eq!(
        state
            .object(bear_id)
            .unwrap()
            .permanent()
            .unwrap()
            .counters_of(&CounterKind::PlusOnePlusOne),
        1
    );
    // The abilities of the card underneath work too: the Gnarr's "whenever this creature mutates" made a Beast either way.
    assert!(names(state, Zone::Battlefield).contains(&"Beast".to_owned()));
    let events = driver.engine().events();
    assert_eq!(
        events
            .iter()
            .filter(|event| matches!(event, Event::Mutated { over: o, .. } if *o == over))
            .count(),
        1
    );
    check_state(driver.state()).unwrap();
    replays(&driver, seats, &scenario);
    Ok(())
}

#[rstest]
fn a_target_that_is_gone_makes_the_spell_resolve_as_a_creature_spell(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(table(seats, 5, vec![ready(bear())], vec![]))?;
    cast_mutating(&mut driver, BEARS)?;
    blade_in_response(&mut driver, BEARS)?;
    assert!(
        !settle(&mut driver, true)?,
        "nothing is asked: there is nothing to merge with"
    );
    let state = driver.state();
    assert!(names(state, Zone::Battlefield).contains(&GNARR.to_owned()));
    assert!(names(state, Zone::Graveyard(pid(0))).contains(&BEARS.to_owned()));
    assert!(state.objects().all(|object| object.merged().is_none()));
    // The creature entered as a creature spell: no mutate trigger and no Beast.
    assert!(!names(state, Zone::Battlefield).contains(&"Beast".to_owned()));
    assert!(
        !driver
            .engine()
            .events()
            .iter()
            .any(|event| matches!(event, Event::Mutated { .. }))
    );
    check_state(state).unwrap();
    Ok(())
}

#[rstest]
fn a_merged_permanent_that_dies_puts_each_card_into_the_graveyard_as_an_object_of_its_own(
    #[values(2, 3, 4)] seats: usize,
    #[values(true, false)] over: bool,
) -> Result<(), ScriptError> {
    let scenario = table(seats, 5, vec![ready(bear())], vec![]);
    let mut driver = Driver::from_scenario(scenario.clone())?;
    let bear_id = driver.id_of(BEARS)?;
    let spell = in_hand(driver.state(), 0, GNARR);
    cast_mutating(&mut driver, BEARS)?;
    settle(&mut driver, over)?;
    let counter = driver.state().object(bear_id).unwrap().counter();
    blade(&mut driver, if over { GNARR } else { BEARS })?;
    let state = driver.state();
    let graveyard = state.zone(Zone::Graveyard(pid(0)));
    assert!(
        graveyard.contains(&bear_id) && graveyard.contains(&spell),
        "{graveyard:?}"
    );
    for id in [bear_id, spell] {
        let part = state.object(id).unwrap();
        assert!(part.merged().is_none() && part.zone() == Zone::Graveyard(pid(0)));
    }
    assert_eq!(state.object(bear_id).unwrap().counter(), counter + 1);
    // From the hand to the stack, then (as part of the permanent) to the graveyard.
    assert_eq!(state.object(spell).unwrap().counter(), 2);
    assert!(!names(state, Zone::Battlefield).contains(&BEARS.to_owned()));
    check_state(state).unwrap();
    replays(&driver, seats, &scenario);
    Ok(())
}

#[rstest]
fn a_commander_card_of_a_merged_permanent_goes_to_the_command_zone_and_the_other_card_stays(
    #[values(2, 3, 4)] seats: usize,
    #[values(true, false)] commander_is_the_spell: bool,
) -> Result<(), ScriptError> {
    // The commander is the Gnarr, cast onto a Bear, or a Bear cast first with the Gnarr mutating onto it.
    let commander = if commander_is_the_spell {
        card(GNARR).unwrap()
    } else {
        bear()
    };
    let mut board = lands(9);
    board.extend([ready(swamp()), ready(swamp())]);
    let mut hand = vec![card("Doom Blade").unwrap()];
    if commander_is_the_spell {
        board.push(ready(bear()));
    } else {
        hand.push(card(GNARR).unwrap());
    }
    let scenario = Scenario::new(seats)
        .format(Arc::new(Commander))
        .commanders(0, [commander])
        .battlefield(0, board)
        .battlefield(1, vec![ready(swamp())])
        .hand(0, hand);
    let scenario = (2..seats).fold(scenario, |scenario, seat| {
        scenario.library(seat, (0..8).map(|_| forest()))
    });
    let mut driver = Driver::from_scenario(scenario)?;
    if !commander_is_the_spell {
        // The Bear commander comes from the command zone; the Gnarr then mutates onto it.
        driver.begin_cast(BEARS)?;
        if matches!(driver.decision()?.kind, DecisionKind::ChooseOption(_)) {
            choose_way(&mut driver, "Cast")?;
        }
        pay(&mut driver)?;
        settle(&mut driver, true)?;
    }
    cast_mutating(&mut driver, BEARS)?;
    settle(&mut driver, true)?;
    let (host, commander_id, other) = {
        let state = driver.state();
        let host = state
            .objects()
            .find(|object| object.merged().is_some())
            .map(mtg_engine::object::GameObject::id)
            .unwrap();
        let parts: Vec<ObjectId> = state
            .object(host)
            .unwrap()
            .merged()
            .unwrap()
            .from_the_top()
            .iter()
            .map(|part| part.id())
            .collect();
        let commander = *parts.iter().find(|id| state.is_commander(**id)).unwrap();
        let other = *parts.iter().find(|id| **id != commander).unwrap();
        // The commander is the host or the card that merged with it, whichever the case.
        assert_eq!(commander == host, !commander_is_the_spell);
        (host, commander, other)
    };
    let victim = driver.state().object(host).unwrap().card().name.clone();
    blade(&mut driver, &victim)?;
    // The owner is asked about the commander card alone; yes sends it on.
    let DecisionKind::YesNo(options) = &driver.decision()?.kind else {
        panic!(
            "the commander is asked about: {:?}",
            driver.decision()?.kind.name()
        );
    };
    assert_eq!(options.source, commander_id);
    driver.answer(Answer::YesNo(YesNoChoice(true)))?;
    let state = driver.state();
    assert_eq!(
        state.object(commander_id).unwrap().zone(),
        Zone::Command(pid(0))
    );
    assert_eq!(state.object(other).unwrap().zone(), Zone::Graveyard(pid(0)));
    check_state(state).unwrap();
    Ok(())
}
