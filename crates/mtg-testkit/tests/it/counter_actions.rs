//! Goal: monstrosity, adapt and bolster play whole at tables of two, three and four seats as the rules say: a creature becomes monstrous once and keeps what it is given while it is (not after it has left), an adapt puts its counters only on a creature without a +1/+1 counter, a bolster puts them on the creature of the least toughness among those the player controls (the player chooses among ties, and an opponent's creature is not theirs), a replacement effect that adds counters applies to all three, a creature gone in response gets nothing, and the log replays to the state it was played to.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_cards::keyword::Keyword;
use mtg_cards::value::CounterKind;
use mtg_core::color::Color;
use mtg_engine::choose::{CardsChoice, NumberChoice};
use mtg_engine::decision::{Answer, DecisionKind, PayChoice};
use mtg_engine::keywords::has_keyword;
use mtg_engine::{Event, PresetPermanent, replay};
use mtg_testkit::cards::{free_creature, land};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready, with_counters};
use rstest::rstest;

use crate::common::pid;

/// `count` ready basic lands of the color.
fn lands(color: Color, count: usize) -> Vec<PresetPermanent> {
    (0..count).map(|_| ready(land(color))).collect()
}

fn named(name: &str) -> PresetPermanent {
    ready(card(name).unwrap())
}

/// Passes priority and answers each choice of a creature with the option at `pick` until nothing is on the stack.
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

/// Activates the only ability of the permanent, pays for it and lets it resolve.
fn activate(driver: &mut Driver, name: &str) -> Result<(), ScriptError> {
    driver.begin_activation(name)?;
    driver.answer(Answer::PayMana(PayChoice::Auto))?;
    resolve(driver, 0)
}

fn counters(driver: &Driver, name: &str) -> u32 {
    let id = driver.id_of(name).unwrap();
    driver
        .state()
        .object(id)
        .and_then(|object| object.permanent())
        .map_or(0, |held| held.counters_of(&CounterKind::PlusOnePlusOne))
}

fn monstrous(driver: &Driver, name: &str) -> bool {
    let id = driver.id_of(name).unwrap();
    driver
        .state()
        .object(id)
        .and_then(|object| object.permanent())
        .is_some_and(|held| held.monstrous())
}

fn count(driver: &Driver, wanted: fn(&Event) -> bool) -> usize {
    driver
        .engine()
        .events()
        .iter()
        .filter(|event| wanted(event))
        .count()
}

fn monstrosities(driver: &Driver) -> usize {
    count(driver, |event| matches!(event, Event::Monstrous { .. }))
}

fn adaptations(driver: &Driver) -> usize {
    count(driver, |event| matches!(event, Event::Adapted { .. }))
}

fn bolsterings(driver: &Driver) -> usize {
    count(driver, |event| matches!(event, Event::Bolstered { .. }))
}

fn assert_replays(setup: &mtg_engine::GameSetup, driver: &Driver) {
    let replayed = replay(setup, driver.engine().events()).unwrap();
    assert_eq!(replayed.state_hash(), driver.engine().state_hash());
}

#[rstest]
#[case::a_cyclops("Ill-Tempered Cyclops", Color::Red, 6, 3)]
#[case::an_asp("Nessian Asp", Color::Green, 7, 4)]
fn a_creature_becomes_monstrous_once_and_gets_the_counters_once(
    #[values(2, 3, 4)] seats: usize,
    #[case] creature: &str,
    #[case] color: Color,
    #[case] cost: usize,
    #[case] granted: u32,
) -> Result<(), ScriptError> {
    let mut board = lands(color, 2 * cost);
    board.push(named(creature));
    let scenario = Scenario::new(seats).battlefield(0, board);
    let setup = scenario.clone().setup().unwrap();
    let mut driver = Driver::from_scenario(scenario)?;
    assert!(!monstrous(&driver, creature));
    activate(&mut driver, creature)?;
    assert_eq!(
        (
            counters(&driver, creature),
            monstrous(&driver, creature),
            monstrosities(&driver)
        ),
        (granted, true, 1)
    );
    // Monstrosity again: the creature is monstrous already, so nothing happens.
    activate(&mut driver, creature)?;
    assert_eq!(
        (counters(&driver, creature), monstrosities(&driver)),
        (granted, 1)
    );
    assert_replays(&setup, &driver);
    Ok(())
}

#[rstest]
#[case::menace("Sinuous Vermin", Color::Black, 5, Keyword::Menace)]
#[case::hexproof("Skittering Crustacean", Color::Blue, 7, Keyword::Hexproof)]
#[case::trample("Domesticated Hydra", Color::Green, 3, Keyword::Trample)]
fn a_creature_has_what_it_is_given_while_monstrous_only(
    #[values(2, 3, 4)] seats: usize,
    #[case] creature: &str,
    #[case] color: Color,
    #[case] cost: usize,
    #[case] given: Keyword,
) -> Result<(), ScriptError> {
    let mut board = lands(color, cost + 1);
    board.push(named(creature));
    let mut driver = Driver::from_scenario(Scenario::new(seats).battlefield(0, board))?;
    let id = driver.id_of(creature)?;
    assert!(!has_keyword(driver.state(), id, &given));
    if creature == "Domesticated Hydra" {
        // {X}{G}{G}{G}: with X = 1 the Hydra gets one counter.
        driver.begin_activation(creature)?;
        loop {
            match &driver.decision()?.kind {
                DecisionKind::PayMana(_) => break,
                DecisionKind::ChooseNumber(_) => {
                    driver.answer(Answer::ChooseNumber(NumberChoice::Number(1)))?;
                }
                other => panic!("unexpected {}", other.name()),
            }
        }
        driver.answer(Answer::PayMana(PayChoice::Auto))?;
        resolve(&mut driver, 0)?;
    } else {
        activate(&mut driver, creature)?;
    }
    assert!(monstrous(&driver, creature));
    assert!(has_keyword(driver.state(), id, &given));
    Ok(())
}

#[rstest]
#[case::hardened_scales("Hardened Scales", 4)]
#[case::doubling_season("Doubling Season", 6)]
fn a_replacement_effect_applies_to_the_counters_of_a_monstrosity(
    #[values(2, 3, 4)] seats: usize,
    #[case] enchantment: &str,
    #[case] granted: u32,
) -> Result<(), ScriptError> {
    let mut board = lands(Color::Red, 6);
    board.extend([named("Ill-Tempered Cyclops"), named(enchantment)]);
    let setup = Scenario::new(seats)
        .battlefield(0, board.clone())
        .setup()
        .unwrap();
    let mut driver = Driver::from_scenario(Scenario::new(seats).battlefield(0, board))?;
    activate(&mut driver, "Ill-Tempered Cyclops")?;
    assert_eq!(counters(&driver, "Ill-Tempered Cyclops"), granted);
    assert!(monstrous(&driver, "Ill-Tempered Cyclops"));
    assert_replays(&setup, &driver);
    Ok(())
}

#[rstest]
fn a_creature_that_is_gone_in_response_does_not_become_monstrous(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut board = lands(Color::Red, 7);
    board.push(named("Ill-Tempered Cyclops"));
    let scenario = Scenario::new(seats)
        .hand(0, [card("Lightning Bolt").unwrap()])
        .battlefield(0, board);
    let setup = scenario.clone().setup().unwrap();
    let mut driver = Driver::from_scenario(scenario)?;
    driver.begin_activation("Ill-Tempered Cyclops")?;
    driver.answer(Answer::PayMana(PayChoice::Auto))?;
    let target = driver.target_named("Ill-Tempered Cyclops")?;
    driver.cast_at("Lightning Bolt", &[target])?;
    resolve(&mut driver, 0)?;
    assert_eq!(monstrosities(&driver), 0);
    assert!(driver.id_of("Ill-Tempered Cyclops").is_err() || !monstrous(&driver, "Ill-Tempered Cyclops"));
    assert_replays(&setup, &driver);
    Ok(())
}

#[rstest]
fn a_trigger_on_becoming_monstrous_resolves_after_the_counters(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut board = lands(Color::White, 7);
    board.push(named("Kalemne's Captain"));
    let scenario = Scenario::new(seats)
        .battlefield(0, board)
        .battlefield(1, [named("Hardened Scales")]);
    let setup = scenario.clone().setup().unwrap();
    let mut driver = Driver::from_scenario(scenario)?;
    assert!(driver.id_of("Hardened Scales").is_ok());
    activate(&mut driver, "Kalemne's Captain")?;
    assert_eq!(counters(&driver, "Kalemne's Captain"), 3);
    // "Exile all artifacts and enchantments": the enchantment of the opponent has left the battlefield.
    assert!(driver.id_of("Hardened Scales").is_err());
    assert_replays(&setup, &driver);
    Ok(())
}

#[rstest]
#[case::no_counter("Skitter Eel", Color::Blue, 3, 0, 2, 1)]
#[case::a_counter_already("Skitter Eel", Color::Blue, 3, 1, 1, 0)]
#[case::many_counters_already("Skitter Eel", Color::Blue, 3, 4, 4, 0)]
#[case::four_counters("Sauroform Hybrid", Color::Green, 6, 0, 4, 1)]
fn an_adapt_puts_the_counters_only_on_a_creature_without_a_plus_one_counter(
    #[values(2, 3, 4)] seats: usize,
    #[case] creature: &str,
    #[case] color: Color,
    #[case] cost: usize,
    #[case] held: u32,
    #[case] expected: u32,
    #[case] adapted: usize,
) -> Result<(), ScriptError> {
    let mut on_board = ready(card(creature).unwrap());
    if held > 0 {
        on_board = with_counters(on_board, CounterKind::PlusOnePlusOne, held);
    }
    let mut board = lands(color, 2 * cost);
    board.push(on_board);
    let scenario = Scenario::new(seats).battlefield(0, board);
    let setup = scenario.clone().setup().unwrap();
    let mut driver = Driver::from_scenario(scenario)?;
    activate(&mut driver, creature)?;
    assert_eq!(
        (counters(&driver, creature), adaptations(&driver)),
        (expected, adapted)
    );
    // Adapting again changes nothing: it holds a +1/+1 counter now, or did.
    activate(&mut driver, creature)?;
    assert_eq!(
        (counters(&driver, creature), adaptations(&driver)),
        (expected, adapted)
    );
    assert_replays(&setup, &driver);
    Ok(())
}

#[rstest]
#[case::hardened_scales("Hardened Scales", 3)]
#[case::doubling_season("Doubling Season", 4)]
fn a_replacement_effect_applies_to_the_counters_of_an_adapt(
    #[values(2, 3, 4)] seats: usize,
    #[case] enchantment: &str,
    #[case] granted: u32,
) -> Result<(), ScriptError> {
    let mut board = lands(Color::Blue, 3);
    board.extend([named("Skitter Eel"), named(enchantment)]);
    let mut driver = Driver::from_scenario(Scenario::new(seats).battlefield(0, board))?;
    activate(&mut driver, "Skitter Eel")?;
    assert_eq!(counters(&driver, "Skitter Eel"), granted);
    assert_eq!(adaptations(&driver), 1);
    Ok(())
}

/// Seat 0 controls three Forests, creatures with these toughnesses (named "C0", "C1", ...) and Cached Defenses in hand;
/// seat 1 controls a 1/1.
fn bolster_table(seats: usize, toughness: &[i32], extra: Vec<PresetPermanent>) -> Scenario {
    let mut board = lands(Color::Green, 3);
    board.extend(
        toughness
            .iter()
            .enumerate()
            .map(|(n, t)| ready(free_creature(&format!("C{n}"), 1, *t))),
    );
    board.extend(extra);
    Scenario::new(seats)
        .hand(0, [card("Cached Defenses").unwrap()])
        .battlefield(0, board)
        .battlefield(1, [ready(free_creature("Their Mouse", 1, 1))])
}

fn bolstered(driver: &Driver, names: usize) -> Vec<u32> {
    (0..names)
        .map(|n| counters(driver, &format!("C{n}")))
        .collect()
}

#[rstest]
#[case::the_weakest(&[3, 2, 4], 0, &[0, 3, 0])]
#[case::the_only_creature(&[5], 0, &[3])]
#[case::a_tie_and_the_first_chosen(&[2, 2, 5], 0, &[3, 0, 0])]
#[case::a_tie_and_the_second_chosen(&[2, 2, 5], 1, &[0, 3, 0])]
#[case::three_tied(&[1, 1, 1], 2, &[0, 0, 3])]
fn a_bolster_puts_the_counters_on_the_creature_of_the_least_toughness_and_the_player_chooses_among_ties(
    #[values(2, 3, 4)] seats: usize,
    #[case] toughness: &[i32],
    #[case] pick: usize,
    #[case] expected: &[u32],
) -> Result<(), ScriptError> {
    let scenario = bolster_table(seats, toughness, Vec::new());
    let setup = scenario.clone().setup().unwrap();
    let mut driver = Driver::from_scenario(scenario)?;
    driver.cast("Cached Defenses")?;
    resolve(&mut driver, pick)?;
    assert_eq!(bolstered(&driver, toughness.len()), expected);
    // The opponent's smaller creature is not the player's to bolster.
    assert_eq!(counters(&driver, "Their Mouse"), 0);
    assert_eq!(bolsterings(&driver), 1);
    assert_replays(&setup, &driver);
    Ok(())
}

#[rstest]
fn the_toughness_counts_as_the_counters_have_left_it(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let creatures = vec![
        with_counters(
            ready(free_creature("C0", 1, 2)),
            CounterKind::PlusOnePlusOne,
            2,
        ),
        ready(free_creature("C1", 1, 3)),
    ];
    let scenario = bolster_table(seats, &[], creatures);
    let mut driver = Driver::from_scenario(scenario)?;
    driver.cast("Cached Defenses")?;
    resolve(&mut driver, 0)?;
    assert_eq!(bolstered(&driver, 2), [2, 3]);
    Ok(())
}

#[rstest]
fn a_player_without_a_creature_bolsters_nothing(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let scenario = bolster_table(seats, &[], Vec::new());
    let mut driver = Driver::from_scenario(scenario)?;
    driver.cast("Cached Defenses")?;
    resolve(&mut driver, 0)?;
    assert_eq!(bolsterings(&driver), 0);
    assert_eq!(counters(&driver, "Their Mouse"), 0);
    Ok(())
}

#[rstest]
#[case::hardened_scales("Hardened Scales", 4)]
#[case::doubling_season("Doubling Season", 6)]
fn a_replacement_effect_applies_to_the_counters_of_a_bolster(
    #[values(2, 3, 4)] seats: usize,
    #[case] enchantment: &str,
    #[case] granted: u32,
) -> Result<(), ScriptError> {
    let scenario = bolster_table(seats, &[2], vec![named(enchantment)]);
    let mut driver = Driver::from_scenario(scenario)?;
    driver.cast("Cached Defenses")?;
    resolve(&mut driver, 0)?;
    assert_eq!(counters(&driver, "C0"), granted);
    Ok(())
}

#[rstest]
fn a_creature_that_bolsters_as_it_enters_counts_among_the_creatures(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut board = lands(Color::White, 3);
    board.push(ready(free_creature("Big", 4, 4)));
    let scenario = Scenario::new(seats)
        .hand(0, [card("Sandcrafter Mage").unwrap()])
        .battlefield(0, board);
    let setup = scenario.clone().setup().unwrap();
    let mut driver = Driver::from_scenario(scenario)?;
    driver.cast("Sandcrafter Mage")?;
    resolve(&mut driver, 0)?;
    assert_eq!(
        (counters(&driver, "Sandcrafter Mage"), counters(&driver, "Big")),
        (1, 0)
    );
    assert_eq!(bolsterings(&driver), 1);
    assert_eq!(driver.state().object(driver.id_of("Big")?).unwrap().controller(), pid(0));
    assert_replays(&setup, &driver);
    Ok(())
}
