//! Goal: "the number of times this creature has mutated" counts the cards merged into the permanent (Insatiable Hemophage, Huntmaster Liger, Porcuparrot): each trigger or activation reads the count it finds, a permanent destroyed in response to its trigger still counts by last known information, and a merged permanent leaves a snapshot that is copied by its topmost card alone and read for its abilities as the cards together, at 2 to 4 seats.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use crate::common::casting::{choose_way, lands, pay, resolve};
use crate::common::pid;
use mtg_cards::CardDefinition;
use mtg_engine::choose::{CardsChoice, YesNoChoice};
use mtg_engine::turn::Step;
use mtg_engine::decision::{Answer, DecisionKind};
use mtg_engine::zones::Zone;
use mtg_engine::{GameState, characteristics, replay, snapshot};
use mtg_testkit::cards::{bear, forest, free_creature, island, mountain, plains, swamp};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, check_state, ready};
use rstest::rstest;

const BEARS: &str = "Grizzly Bears";

/// A mutate creature with the land its mutate cost asks for.
struct Mutator {
    name: &'static str,
    land: fn() -> Arc<CardDefinition>,
}

const HEMOPHAGE: Mutator = Mutator {
    name: "Insatiable Hemophage",
    land: swamp,
};
const LIGER: Mutator = Mutator {
    name: "Huntmaster Liger",
    land: plains,
};
const PORCUPARROT: Mutator = Mutator {
    name: "Porcuparrot",
    land: mountain,
};

/// Seat 0 holds `count` copies of the creature with the lands for all their mutate costs, a Bear to mutate onto and an
/// Ogre beside it; seat 1 holds Doom Blade with two Swamps. Seats beyond the second have a library.
fn table(seats: usize, mutator: &Mutator, count: usize) -> Scenario {
    let mut mine = lands(3 * count + 4, mutator.land);
    mine.extend([ready(bear()), ready(free_creature("Ogre", 3, 3))]);
    let scenario = Scenario::new(seats)
        .hand(0, (0..count).map(|_| card(mutator.name).unwrap()))
        .hand(1, [card("Doom Blade").unwrap()])
        .battlefield(0, mine)
        .battlefield(1, lands(2, swamp));
    (2..seats).fold(scenario, |scenario, seat| {
        scenario.library(seat, (0..8).map(|_| forest()))
    })
}

fn name_of(driver: &Driver, id: mtg_core::ids::ObjectId) -> String {
    driver.state().object(id).unwrap().card().name.clone()
}

/// Seat 0 casts `card` for its mutate cost at `host` and pays, leaving the spell on the stack.
fn cast_mutating(
    driver: &mut Driver,
    card: &str,
    host: mtg_core::ids::ObjectId,
) -> Result<(), ScriptError> {
    let target = driver.target_named(&name_of(driver, host))?;
    driver.begin_cast(card)?;
    choose_way(driver, "Mutate")?;
    driver.choose_targets(&[target])?;
    pay(driver)?;
    Ok(())
}

/// Passes and answers passively until the stack is empty, putting a mutating spell over the creature when `over`.
fn settle(driver: &mut Driver, over: bool) -> Result<(), ScriptError> {
    for _ in 0..300 {
        let idle = driver.state().stack().is_empty()
            && driver.state().resolving().is_none()
            && driver.decision()?.kind.name() == "priority";
        if idle {
            return Ok(());
        }
        match &driver.decision()?.kind {
            DecisionKind::YesNo(options) if options.text.starts_with("Put ") => {
                driver.answer(Answer::YesNo(YesNoChoice(over)))?;
            }
            _ => {
                driver.answer_passively()?;
            }
        }
    }
    Err(ScriptError::Stuck("the stack".into()))
}

/// Seat 0 mutates `count` copies of the creature onto the Bear one after the other.
fn mutate_times(driver: &mut Driver, mutator: &Mutator, count: usize, over: bool) -> Result<(), ScriptError> {
    let host = driver.id_of(BEARS)?;
    for _ in 0..count {
        cast_mutating(driver, mutator.name, host)?;
        settle(driver, over)?;
    }
    Ok(())
}

fn life(state: &GameState, seat: usize) -> i32 {
    state.player(pid(seat)).unwrap().life()
}

fn replays(driver: &Driver, scenario: &Scenario) {
    let setup = scenario.clone().setup().unwrap();
    let replayed = replay(&setup, driver.engine().events()).unwrap();
    assert_eq!(replayed.state_hash(), driver.state().state_hash());
}

/// The count each mutation reads is the number of times the permanent has mutated so far, and a card merged earlier
/// reads it again: the j-th mutation finds j triggers of j each.
fn total(mutations: usize) -> i32 {
    (1..=i32::try_from(mutations).unwrap()).map(|j| j * j).sum()
}

#[rstest]
fn each_trigger_drains_the_number_of_times_the_creature_has_mutated(
    #[values(2, 3, 4)] seats: usize,
    #[values(1, 2, 3)] mutations: usize,
    #[values(true, false)] over: bool,
) -> Result<(), ScriptError> {
    let scenario = table(seats, &HEMOPHAGE, mutations);
    let mut driver = Driver::from_scenario(scenario.clone())?;
    mutate_times(&mut driver, &HEMOPHAGE, mutations, over)?;
    let state = driver.state();
    for seat in 1..seats {
        assert_eq!(life(state, seat), 20 - total(mutations), "seat {seat}");
    }
    assert_eq!(life(state, 0), 20 + total(mutations));
    check_state(state).unwrap();
    replays(&driver, &scenario);
    Ok(())
}

#[rstest]
fn the_pump_of_the_others_is_the_number_of_times_the_creature_has_mutated(
    #[values(2, 3, 4)] seats: usize,
    #[values(1, 2)] mutations: usize,
    #[values(true, false)] over: bool,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(table(seats, &LIGER, mutations))?;
    let ogre = driver.id_of("Ogre")?;
    let host = driver.id_of(BEARS)?;
    mutate_times(&mut driver, &LIGER, mutations, over)?;
    let size = |id| {
        let chars = characteristics(driver.state(), id).unwrap();
        (chars.power.unwrap(), chars.toughness.unwrap())
    };
    let pump = total(mutations);
    // Only the other creatures are pumped, the mutated one is what its top card is.
    assert_eq!(size(ogre), (3 + pump, 3 + pump));
    assert_eq!(size(host), if over { (3, 4) } else { (2, 2) });
    check_state(driver.state()).unwrap();
    Ok(())
}

#[rstest]
fn the_damage_of_the_tap_ability_is_the_number_of_times_the_creature_has_mutated(
    #[values(2, 3, 4)] seats: usize,
    #[values(1, 2, 3)] mutations: usize,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(table(seats, &PORCUPARROT, mutations))?;
    mutate_times(&mut driver, &PORCUPARROT, mutations, true)?;
    // Before any mutation it would deal nothing; now it deals one for each card merged into the Bear.
    driver.begin_activation("Porcuparrot")?;
    let foe = driver.target_player(1);
    driver.choose_targets(&[foe])?;
    resolve(&mut driver)?;
    settle(&mut driver, true)?;
    assert_eq!(life(driver.state(), 1), 20 - i32::try_from(mutations).unwrap());
    check_state(driver.state()).unwrap();
    Ok(())
}

#[rstest]
fn a_permanent_destroyed_in_response_to_its_trigger_still_counts_by_last_known_information(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let scenario = table(seats, &HEMOPHAGE, 1);
    let mut driver = Driver::from_scenario(scenario.clone())?;
    let host = driver.id_of(BEARS)?;
    cast_mutating(&mut driver, HEMOPHAGE.name, host)?;
    // Both pass: the spell resolves and its controller puts it under the Bear, a green creature Doom Blade can take.
    while !matches!(driver.decision()?.kind, DecisionKind::YesNo(_)) {
        driver.pass()?;
    }
    driver.answer(Answer::YesNo(YesNoChoice(false)))?;
    assert_eq!(driver.state().stack().len(), 1, "the trigger waits");
    driver.pass()?;
    let target = driver.target_named(BEARS)?;
    driver.cast_at("Doom Blade", &[target])?;
    settle(&mut driver, false)?;
    let state = driver.state();
    assert!(!mtg_testkit_names(state).contains(&BEARS.to_owned()));
    // The permanent was gone when the trigger resolved; it had mutated once.
    assert_eq!(life(state, 1), 19);
    assert_eq!(life(state, 0), 21);
    check_state(state).unwrap();
    replays(&driver, &scenario);
    Ok(())
}

fn mtg_testkit_names(state: &GameState) -> Vec<String> {
    state
        .zone(Zone::Battlefield)
        .iter()
        .map(|id| state.object(*id).unwrap().card().name.clone())
        .collect()
}

#[rstest]
fn a_snapshot_of_a_merged_permanent_is_copied_by_its_top_card_alone_and_read_as_all_the_cards(
    #[values(2, 3, 4)] seats: usize,
    #[values(1, 2)] mutations: usize,
    #[values(true, false)] over: bool,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(table(seats, &HEMOPHAGE, mutations))?;
    let host = driver.id_of(BEARS)?;
    mutate_times(&mut driver, &HEMOPHAGE, mutations, over)?;
    let shot = snapshot(driver.state(), host).unwrap();
    let rules = card(HEMOPHAGE.name).unwrap().rules.len();
    assert_eq!(shot.mutations, u32::try_from(mutations).unwrap());
    // What a copy of it takes: the Bear's nothing, or the Hemophage's rules once, never those of the cards under it.
    let copy = shot.copy.as_deref().unwrap();
    assert_eq!(copy.name, if over { HEMOPHAGE.name } else { BEARS });
    assert_eq!(copy.rules.len(), if over { rules } else { 0 });
    // What its abilities read after it has left: the abilities of every card.
    let merged = shot.merged.as_deref().unwrap();
    assert_eq!(merged.rules.len(), rules * mutations);
    Ok(())
}

/// Seat 0 holds Insatiable Hemophage and Cytoshape with lands for both and Trumpeting Gnarr on the battlefield; seat 1 has
/// a Hill Giant to copy.
fn copy_table(seats: usize) -> Scenario {
    let mut mine = lands(7, swamp);
    mine.extend(lands(2, forest));
    mine.extend(lands(2, island));
    mine.push(ready(card("Trumpeting Gnarr").unwrap()));
    let scenario = Scenario::new(seats)
        .hand(
            0,
            [card(HEMOPHAGE.name).unwrap(), card("Cytoshape").unwrap()],
        )
        .battlefield(0, mine)
        .battlefield(1, [ready(free_creature("Hill Giant", 3, 4))]);
    (2..seats).fold(scenario, |scenario, seat| {
        scenario.library(seat, (0..8).map(|_| forest()))
    })
}

/// Seat 0 casts Cytoshape at `host`, which becomes a copy of the Hill Giant until the end of the turn.
fn become_a_giant(driver: &mut Driver, host: mtg_core::ids::ObjectId) -> Result<(), ScriptError> {
    let giant = driver.id_of("Hill Giant")?;
    let target = driver.target_named(&name_of(driver, host))?;
    driver.cast_at("Cytoshape", &[target])?;
    while !matches!(driver.decision()?.kind, DecisionKind::ChooseCards(_)) {
        driver.pass()?;
    }
    driver.answer(Answer::ChooseCards(CardsChoice(vec![giant])))?;
    settle(driver, true)
}

#[rstest]
fn a_merged_permanent_under_a_copy_effect_keeps_the_abilities_of_its_other_cards(
    #[values(2, 3, 4)] seats: usize,
    #[values(true, false)] over: bool,
    #[values(true, false)] copy_first: bool,
) -> Result<(), ScriptError> {
    let scenario = copy_table(seats);
    let mut driver = Driver::from_scenario(scenario.clone())?;
    let host = driver.id_of("Trumpeting Gnarr")?;
    if copy_first {
        become_a_giant(&mut driver, host)?;
    }
    cast_mutating(&mut driver, HEMOPHAGE.name, host)?;
    settle(&mut driver, over)?;
    if !copy_first {
        become_a_giant(&mut driver, host)?;
    }
    let (gnarr, hemophage) = (
        card("Trumpeting Gnarr").unwrap(),
        card(HEMOPHAGE.name).unwrap(),
    );
    // The copy takes the place of the top card: its values, and the abilities of the card under it.
    let shown = driver.state().object(host).unwrap().card();
    assert_eq!(shown.name, "Hill Giant");
    assert_eq!(
        shown.rules,
        if over { gnarr.rules.clone() } else { hemophage.rules.clone() }
    );
    let chars = characteristics(driver.state(), host).unwrap();
    assert_eq!((chars.power, chars.toughness), (Some(3), Some(4)));
    // A copy of the permanent is a copy of the copy, with none of the abilities of the cards.
    let shot = snapshot(driver.state(), host).unwrap();
    assert_eq!(shot.copy.as_deref().unwrap().name, "Hill Giant");
    assert!(shot.copy.as_deref().unwrap().rules.is_empty());
    check_state(driver.state()).unwrap();
    // The cleanup step ends the copy: the abilities of both cards, and the top card's characteristics.
    driver.idle_until(2, Step::Upkeep)?;
    let shown = driver.state().object(host).unwrap().card();
    assert_eq!(
        shown.name,
        if over { HEMOPHAGE.name } else { "Trumpeting Gnarr" }
    );
    assert_eq!(shown.rules.len(), gnarr.rules.len() + hemophage.rules.len());
    replays(&driver, &scenario);
    Ok(())
}
