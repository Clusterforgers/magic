//! Goal: an amass plays whole at tables of two, three and four seats as the rules say: with no Army the player creates a 0/0 black Army token of the subtype and puts the counters on it, with an Army they put them on it (the player chooses among several, and an opponent's Army is not theirs), the Army gets the subtype it lacks, a replacement effect that adds counters or doubles tokens applies, a creature that is gone when its trigger resolves still amasses, and the log replays to the state it was played to.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::value::CounterKind;
use mtg_core::card::{CardType, Subtype};
use mtg_core::color::Color;
use mtg_engine::characteristics::characteristics;
use mtg_engine::choose::CardsChoice;
use mtg_engine::decision::{Answer, DecisionKind};
use mtg_engine::{Event, PresetPermanent, replay};
use mtg_testkit::cards::land;
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready, with_counters};
use rstest::rstest;

use crate::common::pid;

/// An Army of the subtype that is a creature with no printed size, as a permanent that was already there.
fn army(name: &str, subtype: &str) -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new(name)
            .with_type(CardType::Creature)
            .with_subtype("Army")
            .with_subtype(subtype)
            .with_power_toughness(0, 0),
    )
}

/// Seat 0 holds the spell and four Islands; `board` is what seat 0 controls besides.
fn table(seats: usize, spell: &str, board: Vec<PresetPermanent>) -> Scenario {
    let mut lands: Vec<_> = (0..4).map(|_| ready(land(Color::Blue))).collect();
    lands.extend(board);
    Scenario::new(seats)
        .hand(0, [card(spell).unwrap()])
        .battlefield(0, lands)
}

/// Passes priority and answers each choice of an Army with the option at `pick` until nothing is on the stack.
fn resolve(driver: &mut Driver, pick: usize) -> Result<(), ScriptError> {
    for _ in 0..40 {
        match &driver.decision()?.kind {
            DecisionKind::Priority(_) if driver.state().stack().is_empty() => return Ok(()),
            DecisionKind::Priority(_) => {
                driver.pass()?;
            }
            DecisionKind::ChooseCards(options) => {
                let chosen = options.from[pick.min(options.from.len() - 1)];
                driver.answer(Answer::ChooseCards(CardsChoice(vec![chosen])))?;
            }
            other => panic!("unexpected {}", other.name()),
        }
    }
    panic!("the stack did not empty");
}

/// The Armies on the battlefield seat `seat` controls, as (name, +1/+1 counters, subtypes), in the order of their ids.
fn armies(driver: &Driver, seat: usize) -> Vec<(String, u32, Vec<String>)> {
    let state = driver.state();
    let mut found: Vec<_> = state
        .objects()
        .filter(|object| object.controller() == pid(seat) && object.permanent().is_some())
        .filter_map(|object| {
            let chars = characteristics(state, object.id())?;
            let army = chars.has_subtype(&object.card().subtypes, &Subtype("Army".into()));
            army.then(|| {
                let mut subtypes: Vec<String> = chars
                    .subtypes
                    .iter()
                    .chain(object.card().subtypes.iter())
                    .map(|subtype| subtype.0.clone())
                    .collect();
                subtypes.sort();
                subtypes.dedup();
                (
                    object.id(),
                    object.card().name.clone(),
                    object
                        .permanent()
                        .map_or(0, |p| p.counters_of(&CounterKind::PlusOnePlusOne)),
                    subtypes,
                )
            })
        })
        .collect();
    found.sort_by_key(|(id, ..)| *id);
    found
        .into_iter()
        .map(|(_, name, counters, subtypes)| (name, counters, subtypes))
        .collect()
}

fn amassed(driver: &Driver) -> usize {
    driver
        .engine()
        .events()
        .iter()
        .filter(|event| matches!(event, Event::Amassed { .. }))
        .count()
}

fn names(list: &[&str]) -> Vec<String> {
    list.iter().map(|name| (*name).to_owned()).collect()
}

#[rstest]
fn with_no_army_a_black_army_token_of_the_subtype_gets_the_counters(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let scenario = table(seats, "Relentless Advance", Vec::new());
    let setup = scenario.clone().setup().unwrap();
    let mut driver = Driver::from_scenario(scenario)?;
    driver.cast("Relentless Advance")?;
    resolve(&mut driver, 0)?;
    assert_eq!(
        armies(&driver, 0),
        [("Zombie Army".to_owned(), 3, names(&["Army", "Zombie"]))]
    );
    let token = driver.id_of("Zombie Army")?;
    let object = driver.state().object(token).unwrap();
    let chars = characteristics(driver.state(), token).unwrap();
    assert_eq!((chars.power, chars.toughness), (Some(3), Some(3)));
    assert!(chars.colors.contains(Color::Black) && object.permanent().is_some());
    assert_eq!(amassed(&driver), 1);
    for seat in 1..seats {
        assert_eq!(armies(&driver, seat).len(), 0);
    }
    let replayed = replay(&setup, driver.engine().events()).unwrap();
    assert_eq!(replayed.state_hash(), driver.engine().state_hash());
    Ok(())
}

#[rstest]
#[case::the_subtype_it_lacks(army("Orc Army", "Orc"), 2, 5, &["Army", "Orc", "Zombie"])]
#[case::the_subtype_it_has(army("Zombie Army", "Zombie"), 0, 3, &["Army", "Zombie"])]
#[case::counters_it_already_holds(army("Zombie Army", "Zombie"), 4, 7, &["Army", "Zombie"])]
fn with_an_army_the_counters_go_on_it_and_it_becomes_the_subtype_too(
    #[values(2, 3, 4)] seats: usize,
    #[case] present: Arc<CardDefinition>,
    #[case] held: u32,
    #[case] counters: u32,
    #[case] subtypes: &[&str],
) -> Result<(), ScriptError> {
    let name = present.name.clone();
    let mut army = ready(present);
    if held > 0 {
        army = with_counters(army, CounterKind::PlusOnePlusOne, held);
    }
    let scenario = table(seats, "Relentless Advance", vec![army]);
    let setup = scenario.clone().setup().unwrap();
    let mut driver = Driver::from_scenario(scenario)?;
    driver.cast("Relentless Advance")?;
    resolve(&mut driver, 0)?;
    assert_eq!(armies(&driver, 0), [(name, counters, names(subtypes))]);
    assert_eq!(amassed(&driver), 1);
    let replayed = replay(&setup, driver.engine().events()).unwrap();
    assert_eq!(replayed.state_hash(), driver.engine().state_hash());
    Ok(())
}

#[rstest]
fn with_several_armies_the_player_chooses_the_one_that_gets_the_counters(
    #[values(2, 3, 4)] seats: usize,
    #[values(0, 1)] pick: usize,
) -> Result<(), ScriptError> {
    let board = vec![
        with_counters(
            ready(army("First Army", "Zombie")),
            CounterKind::PlusOnePlusOne,
            1,
        ),
        with_counters(
            ready(army("Second Army", "Zombie")),
            CounterKind::PlusOnePlusOne,
            2,
        ),
    ];
    let mut driver = Driver::from_scenario(table(seats, "Relentless Advance", board))?;
    driver.cast("Relentless Advance")?;
    while !matches!(driver.decision()?.kind, DecisionKind::ChooseCards(_)) {
        driver.pass()?;
    }
    resolve(&mut driver, pick)?;
    let counters: Vec<u32> = armies(&driver, 0).iter().map(|army| army.1).collect();
    assert_eq!(counters, if pick == 0 { [4, 2] } else { [1, 5] });
    assert_eq!(amassed(&driver), 1);
    Ok(())
}

#[rstest]
fn an_army_an_opponent_controls_is_not_the_players_to_put_counters_on(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let scenario = table(seats, "Relentless Advance", Vec::new()).battlefield(
        1,
        [with_counters(
            ready(army("Their Army", "Orc")),
            CounterKind::PlusOnePlusOne,
            2,
        )],
    );
    let mut driver = Driver::from_scenario(scenario)?;
    driver.cast("Relentless Advance")?;
    resolve(&mut driver, 0)?;
    assert_eq!(armies(&driver, 0).len(), 1);
    assert_eq!(armies(&driver, 0)[0].1, 3);
    assert_eq!(armies(&driver, 1)[0].1, 2);
    Ok(())
}

#[rstest]
#[case::hardened_scales_adds_one_to_the_new_army("Hardened Scales", false, 4)]
#[case::hardened_scales_adds_one_to_an_army("Hardened Scales", true, 4)]
#[case::doubling_season_doubles_the_counters("Doubling Season", true, 6)]
fn a_replacement_effect_applies_to_the_counters_put_on_the_army(
    #[values(2, 3, 4)] seats: usize,
    #[case] enchantment: &str,
    #[case] with_army: bool,
    #[case] counters: u32,
) -> Result<(), ScriptError> {
    let mut board = vec![ready(card(enchantment).unwrap())];
    if with_army {
        board.push(ready(army("Zombie Army", "Zombie")));
    }
    let setup = table(seats, "Relentless Advance", board.clone())
        .setup()
        .unwrap();
    let mut driver = Driver::from_scenario(table(seats, "Relentless Advance", board))?;
    driver.cast("Relentless Advance")?;
    resolve(&mut driver, 0)?;
    assert_eq!(armies(&driver, 0).len(), 1);
    assert_eq!(armies(&driver, 0)[0].1, counters);
    let replayed = replay(&setup, driver.engine().events()).unwrap();
    assert_eq!(replayed.state_hash(), driver.engine().state_hash());
    Ok(())
}

#[rstest]
fn a_replacement_effect_that_doubles_tokens_makes_two_armies_and_the_player_chooses_one(
    #[values(2, 3, 4)] seats: usize,
    #[values(0, 1)] pick: usize,
) -> Result<(), ScriptError> {
    let board = vec![ready(card("Parallel Lives").unwrap())];
    let mut driver = Driver::from_scenario(table(seats, "Relentless Advance", board))?;
    driver.cast("Relentless Advance")?;
    resolve(&mut driver, pick)?;
    // The other Army is a 0/0 and is gone.
    assert_eq!(armies(&driver, 0).len(), 1);
    assert_eq!(armies(&driver, 0)[0].1, 3);
    assert_eq!(amassed(&driver), 1);
    Ok(())
}

#[rstest]
fn a_creature_that_is_gone_when_its_trigger_resolves_still_amasses(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let scenario = Scenario::new(seats)
        .hand(0, [card("Lightning Bolt").unwrap()])
        .battlefield(
            0,
            [
                ready(land(Color::Red)),
                ready(card("Easterling Vanguard").unwrap()),
            ],
        );
    let setup = scenario.clone().setup().unwrap();
    let mut driver = Driver::from_scenario(scenario)?;
    let target = driver.target_named("Easterling Vanguard")?;
    driver.cast_at("Lightning Bolt", &[target])?;
    resolve(&mut driver, 0)?;
    assert_eq!(
        armies(&driver, 0),
        [("Orc Army".to_owned(), 1, names(&["Army", "Orc"]))]
    );
    assert_eq!(amassed(&driver), 1);
    let replayed = replay(&setup, driver.engine().events()).unwrap();
    assert_eq!(replayed.state_hash(), driver.engine().state_hash());
    Ok(())
}
