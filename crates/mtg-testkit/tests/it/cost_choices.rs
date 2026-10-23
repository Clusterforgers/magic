//! Goal: a spell whose required additional cost is a choice between costs ("discard a card or pay 3 life", "reveal a Dinosaur card from your hand or pay {1}") offers each cost as a way to cast it at 2, 3 and 4 seats, asks which when more than one can be paid and not when only one can, pays the one chosen (the life, the discard, the extra mana, or a reveal that every seat sees and that leaves the card in the hand), leaves out a cost the caster cannot give, does not offer the spell when none can be paid, can be backed out of, and replays to the state it was played to.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use crate::common::casting::{lands, pay, ways};
use crate::common::{name_of, names, pid};
use mtg_cards::CardDefinition;
use mtg_core::card::CardType;
use mtg_core::mana::ManaCost;
use mtg_engine::choose::{CardsChoice, OptionChoice};
use mtg_engine::decision::{Answer, PayChoice};
use mtg_engine::zones::Zone;
use mtg_engine::{Event, replay};
use mtg_testkit::cards::{forest, free_creature, swamp};
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

fn corpus(name: &str) -> Arc<CardDefinition> {
    mtg_testkit::corpus::card(name).unwrap()
}

/// A Dinosaur creature card that costs nothing.
fn raptor(name: &str) -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new(name)
            .with_type(CardType::Creature)
            .with_subtype("Dinosaur")
            .with_power_toughness(2, 2),
    )
}

fn replays(driver: &Driver, scenario: Scenario) {
    let replayed = replay(&scenario.setup().unwrap(), driver.engine().events()).unwrap();
    assert_eq!(replayed.state_hash(), driver.state().state_hash());
}

fn count(driver: &Driver, zone: Zone, name: &str) -> usize {
    names(driver.state(), zone)
        .iter()
        .filter(|found| *found == name)
        .count()
}

/// Seat 0 holds `hand`, controls two Swamps and may destroy the Bear of seat 1; every seat has a library.
fn bitter(seats: usize, hand: Vec<Arc<CardDefinition>>, life: i32) -> Scenario {
    let mut table = Scenario::new(seats)
        .hand(0, hand)
        .battlefield(0, lands(2, swamp))
        .battlefield(1, [ready(free_creature("Bear", 2, 2))])
        .library(0, (0..8).map(|_| forest()))
        .life(0, life);
    for seat in 1..seats {
        table = table.library(seat, (0..8).map(|_| forest()));
    }
    table
}

fn bitter_hand(others: usize) -> Vec<Arc<CardDefinition>> {
    std::iter::once(corpus("Bitter Triumph"))
        .chain((0..others).map(|_| forest()))
        .collect()
}

fn aim_and_pay(driver: &mut Driver) -> Result<Vec<Event>, ScriptError> {
    let target = driver.target_named("Bear")?;
    driver.choose_targets(&[target])?;
    pay(driver)
}

#[rstest]
#[case::the_discard(0, 1, 20)]
#[case::the_life(1, 0, 17)]
fn the_cost_chosen_is_the_one_paid(
    #[case] pick: usize,
    #[case] discarded: usize,
    #[case] life: i32,
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let scenario = || bitter(seats, bitter_hand(1), 20);
    let mut driver = Driver::from_scenario(scenario())?;
    driver.begin_cast("Bitter Triumph")?;
    assert_eq!(
        ways(&driver)?,
        [
            "Cast for {1}{B}, discard a card",
            "Cast for {1}{B}, pay 3 life"
        ]
    );
    driver.answer(Answer::ChooseOption(OptionChoice(pick)))?;
    let events = aim_and_pay(&mut driver)?;
    let seen = events
        .iter()
        .filter(|event| matches!(event, Event::CardDiscarded { .. }))
        .count();
    assert_eq!(seen, discarded);
    assert_eq!(driver.state().player(pid(0)).unwrap().life(), life);
    assert_eq!(count(&driver, Zone::Graveyard(pid(0)), "Forest"), discarded);
    for _ in 0..seats {
        driver.pass()?;
    }
    assert_eq!(count(&driver, Zone::Battlefield, "Bear"), 0, "the spell resolved");
    replays(&driver, scenario());
    Ok(())
}

#[rstest]
#[case::no_other_card(0, 20, "pay 3 life")]
#[case::too_little_life(1, 2, "discard a card")]
fn a_cost_the_caster_cannot_give_is_not_offered(
    #[case] others: usize,
    #[case] life: i32,
    #[case] only: &str,
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(bitter(seats, bitter_hand(others), life))?;
    driver.begin_cast("Bitter Triumph")?;
    // One way is left, so nothing is asked about how.
    let target = driver.target_named("Bear")?;
    driver.choose_targets(&[target])?;
    let events = pay(&mut driver)?;
    let discards = events
        .iter()
        .filter(|event| matches!(event, Event::CardDiscarded { .. }))
        .count();
    assert_eq!(discards, usize::from(only == "discard a card"));
    Ok(())
}

#[rstest]
fn a_spell_with_no_cost_to_give_is_not_offered(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(bitter(seats, bitter_hand(0), 2))?;
    assert!(driver.begin_cast("Bitter Triumph").is_err());
    Ok(())
}

#[rstest]
fn backing_out_of_the_payment_after_the_choice_leaves_no_trace(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(bitter(seats, bitter_hand(1), 20))?;
    let logged = driver.engine().events().len();
    driver.begin_cast("Bitter Triumph")?;
    driver.answer(Answer::ChooseOption(OptionChoice(1)))?;
    let target = driver.target_named("Bear")?;
    driver.choose_targets(&[target])?;
    driver.answer(Answer::PayMana(PayChoice::Cancel))?;
    assert_eq!(driver.engine().events().len(), logged);
    assert_eq!(count(&driver, Zone::Hand(pid(0)), "Bitter Triumph"), 1);
    Ok(())
}

/// Seat 0 holds Thunderherd Migration and `others` and controls three Forests; every seat has a library.
fn thunderherd(seats: usize, others: Vec<Arc<CardDefinition>>) -> Scenario {
    let mut table = Scenario::new(seats)
        .hand(
            0,
            std::iter::once(corpus("Thunderherd Migration")).chain(others),
        )
        .battlefield(0, lands(3, forest))
        .library(0, (0..8).map(|_| forest()));
    for seat in 1..seats {
        table = table.library(seat, (0..8).map(|_| forest()));
    }
    table
}

#[rstest]
fn a_reveal_is_shown_to_every_seat_and_leaves_the_card_in_the_hand(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let scenario = || thunderherd(seats, vec![raptor("Raptor"), raptor("Compy")]);
    let mut driver = Driver::from_scenario(scenario())?;
    driver.begin_cast("Thunderherd Migration")?;
    assert_eq!(
        ways(&driver)?,
        [
            "Cast for {1}{G}, reveal a Dinosaur card from your hand",
            "Cast for {1}{G}, pay {1}"
        ]
    );
    driver.answer(Answer::ChooseOption(OptionChoice(0)))?;
    // Two Dinosaurs qualify, so the caster picks one.
    let raptor = *driver
        .state()
        .zone(Zone::Hand(pid(0)))
        .iter()
        .find(|id| name_of(driver.state(), **id) == "Raptor")
        .unwrap();
    driver.answer(Answer::ChooseCards(CardsChoice(vec![raptor])))?;
    assert_eq!(driver.pay_options()?.cost, ManaCost::parse("{1}{G}").unwrap());
    let events = pay(&mut driver)?;
    let revealed: Vec<_> = events
        .iter()
        .filter_map(|event| match event {
            Event::Revealed(revealed) => Some(revealed),
            _ => None,
        })
        .collect();
    assert_eq!(revealed.len(), 1, "one reveal, by the caster");
    assert_eq!(revealed[0].player, pid(0));
    assert_eq!(revealed[0].cards, vec![raptor]);
    assert_eq!(count(&driver, Zone::Hand(pid(0)), "Raptor"), 1, "it stays");
    replays(&driver, scenario());
    Ok(())
}

#[rstest]
#[case::with_no_dinosaur(vec![])]
#[case::with_a_card_that_is_not_one(vec![forest()])]
fn mana_pays_when_there_is_nothing_to_reveal(
    #[case] others: Vec<Arc<CardDefinition>>,
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(thunderherd(seats, others))?;
    driver.begin_cast("Thunderherd Migration")?;
    // The reveal is not offered, so the one cost left is paid without a question: {1}{G} and {1}.
    assert_eq!(
        driver.pay_options()?.cost,
        ManaCost::parse("{2}{G}").unwrap()
    );
    Ok(())
}

/// The kinds of event that pay the parts of a cast, in the run directly before `SpellCast`.
#[derive(Debug, Default, Clone, Copy)]
struct Paid {
    discards: u32,
    sacrifices: u32,
    reveals: u32,
    life: u32,
}

fn paid_before(events: &[Event], cast: usize) -> Paid {
    use mtg_engine::events::ZoneCause;
    let mut paid = Paid::default();
    for event in events[..cast].iter().rev() {
        match event {
            Event::CardDiscarded { .. } => paid.discards += 1,
            Event::Revealed(_) => paid.reveals += 1,
            Event::LifePaid { amount, .. } => paid.life += amount,
            Event::ZoneChanged {
                cause: ZoneCause::Sacrificed,
                ..
            } => paid.sacrifices += 1,
            _ => break,
        }
    }
    paid
}

#[test]
fn bot_games_with_the_choice_cards_pay_exactly_one_cost_on_every_cast_and_keep_every_invariant() {
    use crate::common::seeds::{for_each_seed, fuzz_games};
    use mtg_testkit::{PlayOptions, cost_choice_fuzz_setup, play_game_with};
    // How often Bitter Triumph was paid with a discard and with life, and Thunderherd Migration with a reveal and with mana.
    let seen = std::sync::Mutex::new([0u64; 4]);
    for_each_seed(0..fuzz_games(16), |seed| {
        for players in [2, 3, 4] {
            let setup = cost_choice_fuzz_setup(players, seed);
            let outcome = play_game_with(setup, seed, PlayOptions::default())
                .unwrap_or_else(|failure| panic!("{failure}"));
            let replayed = replay(&cost_choice_fuzz_setup(players, seed), &outcome.events).unwrap();
            assert_eq!(replayed.state_hash(), outcome.final_hash, "seed {seed}");
            let mut seen = seen.lock().unwrap();
            for (at, event) in outcome.events.iter().enumerate() {
                let Event::SpellCast { object, .. } = event else {
                    continue;
                };
                let Some(name) = replayed
                    .object(*object)
                    .map(|spell| spell.card().name.clone())
                else {
                    continue;
                };
                let paid = paid_before(&outcome.events, at);
                match name.as_str() {
                    "Bitter Triumph" => {
                        assert_eq!(
                            (paid.discards, paid.life),
                            if paid.life == 3 { (0, 3) } else { (1, 0) },
                            "seed {seed}, {players} seats: Bitter Triumph paid {paid:?}"
                        );
                        seen[usize::from(paid.life == 3)] += 1;
                    }
                    "Dusk Mangler" => assert_eq!(
                        paid.discards + paid.sacrifices + u32::from(paid.life == 4),
                        1,
                        "seed {seed}, {players} seats: Dusk Mangler paid {paid:?}"
                    ),
                    "Thunderherd Migration" => {
                        assert!(paid.reveals <= 1 && paid.discards == 0 && paid.life == 0);
                        seen[2 + usize::from(paid.reveals == 0)] += 1;
                    }
                    _ => {}
                }
            }
        }
    });
    let seen = seen.into_inner().unwrap();
    assert!(
        seen[0] > 0 && seen[1] > 0,
        "Bitter Triumph was paid both ways: {seen:?}"
    );
    assert!(
        seen[2] + seen[3] > 0,
        "Thunderherd Migration was cast: {seen:?}"
    );
}
