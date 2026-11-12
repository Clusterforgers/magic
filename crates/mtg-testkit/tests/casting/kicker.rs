//! Goal: kicker and multikicker play whole: the optional costs are asked before the targets and offered only up to what the lands can pay for, the total cost holds them, a kicked Burst Lightning deals 4 and an unkicked one 2, a kicked Gatekeeper of Malakir makes a player sacrifice a creature and an unkicked one does not even trigger, Wolfbriar Elemental makes a Wolf for each time it was kicked, what was paid stays with the permanent and travels with its trigger (the permanent may leave first), an answer outside the options is refused, a cast backed out of at either question leaves no trace, and whole bot games with the kicked cards keep every invariant.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common;

use common::{names, pid};
use mtg_engine::choose::{CostsChoice, NumberChoice};
use mtg_engine::decision::{Answer, DecisionKind, PayChoice};
use mtg_engine::error::IllegalAnswer;
use mtg_engine::zones::Zone;
use mtg_engine::{EngineError, Event};
use mtg_testkit::cards::{forest, free_creature, mountain, swamp};
use mtg_testkit::corpus::card;
use mtg_testkit::{
    Driver, PlayOptions, Scenario, ScriptError, kicker_fuzz_setup, play_game_with, ready,
};
use rstest::rstest;

use crate::common::seeds::{for_each_seed, fuzz_games};

/// Seat 0 holds `name` and controls `lands`; seat 1 controls a Bear.
fn table(name: &str, lands: impl IntoIterator<Item = mtg_engine::PresetPermanent>) -> Scenario {
    Scenario::new(2)
        .hand(0, [card(name).unwrap()])
        .battlefield(0, lands)
        .battlefield(1, [ready(free_creature("Bear", 2, 2))])
}

fn lands(
    count: usize,
    land: fn() -> std::sync::Arc<mtg_cards::CardDefinition>,
) -> Vec<mtg_engine::PresetPermanent> {
    (0..count).map(|_| ready(land())).collect()
}

/// Answers the pending decision, which must be the optional costs, with `costs`.
fn kick(driver: &mut Driver, costs: &[usize]) -> Result<Vec<Event>, ScriptError> {
    driver.expect_decision(0, "choose-costs")?;
    driver.answer(Answer::ChooseCosts(CostsChoice::Costs(costs.to_vec())))
}

/// Answers the pending decision, which must be the times a cost is paid, with `times`.
fn times(driver: &mut Driver, times: u32) -> Result<Vec<Event>, ScriptError> {
    driver.expect_decision(0, "choose-number")?;
    driver.answer(Answer::ChooseNumber(NumberChoice::Number(times)))
}

fn pay(driver: &mut Driver) -> Result<Vec<Event>, ScriptError> {
    driver.expect_decision(0, "pay-mana")?;
    driver.answer(Answer::PayMana(PayChoice::Auto))
}

/// Both players pass: the top of the stack resolves.
fn resolve(driver: &mut Driver) -> Result<(), ScriptError> {
    driver.pass()?;
    driver.pass()?;
    Ok(())
}

fn life(driver: &Driver, seat: usize) -> i32 {
    driver.state().player(pid(seat)).unwrap().life()
}

fn tapped(driver: &Driver) -> usize {
    let state = driver.state();
    state
        .zone(Zone::Battlefield)
        .iter()
        .filter_map(|id| state.object(*id)?.permanent())
        .filter(|permanent| permanent.tapped())
        .count()
}

fn count(driver: &Driver, name: &str) -> usize {
    names(driver.state(), Zone::Battlefield)
        .iter()
        .filter(|found| *found == name)
        .count()
}

#[rstest]
#[case::one_land_cannot_kick(1, None, 2, 1)]
#[case::four_lands_cannot_pay_the_kicker_with_the_cost(4, None, 2, 1)]
#[case::five_lands_unkicked(5, Some(false), 2, 1)]
#[case::five_lands_kicked(5, Some(true), 4, 5)]
fn burst_lightning_deals_four_when_kicked_and_two_when_not_and_the_kicker_is_offered_only_when_it_can_be_paid(
    #[case] mountains: usize,
    #[case] kicked: Option<bool>,
    #[case] damage: i32,
    #[case] lands_tapped: usize,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(table("Burst Lightning", lands(mountains, mountain)))?;
    driver.begin_cast("Burst Lightning")?;
    match kicked {
        Some(kicked) => {
            let DecisionKind::ChooseCosts(options) = &driver.decision()?.kind else {
                panic!("the optional costs come first");
            };
            assert_eq!(options.optional.len(), 1);
            assert_eq!(options.optional[0].label, "Kicker {4}");
            assert!(options.can_cancel);
            kick(&mut driver, if kicked { &[0] } else { &[] })?;
        }
        None => driver.expect_decision(0, "choose-targets")?,
    }
    let foe = driver.target_player(1);
    driver.choose_targets(&[foe])?;
    let cost = driver.pay_options()?.cost.mana_value();
    assert_eq!(cost, if damage == 4 { 5 } else { 1 });
    pay(&mut driver)?;
    resolve(&mut driver)?;
    assert_eq!(life(&driver, 1), 20 - damage);
    assert_eq!(tapped(&driver), lands_tapped);
    Ok(())
}

#[rstest]
#[case::the_kicker_is_asked_before_the_targets(true)]
#[case::an_answer_not_offered_is_refused_and_asked_again(false)]
fn the_optional_costs_are_asked_first_and_the_answer_is_checked(
    #[case] valid: bool,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(table("Burst Lightning", lands(6, mountain)))?;
    driver.begin_cast("Burst Lightning")?;
    driver.expect_decision(0, "choose-costs")?;
    if valid {
        driver.answer(Answer::ChooseCosts(CostsChoice::Costs(vec![0])))?;
        driver.expect_decision(0, "choose-targets")?;
        return Ok(());
    }
    let before = driver.engine().events().len();
    for costs in [vec![1], vec![0, 0]] {
        let refused = driver.answer(Answer::ChooseCosts(CostsChoice::Costs(costs)));
        assert!(
            matches!(refused, Err(ScriptError::Engine(EngineError::Illegal(_)))),
            "{refused:?}"
        );
        assert_eq!(driver.engine().events().len(), before);
        driver.expect_decision(0, "choose-costs")?;
    }
    Ok(())
}

#[test]
fn backing_out_at_the_optional_costs_leaves_no_trace() -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(table("Burst Lightning", lands(5, mountain)))?;
    let before = (driver.engine().events().len(), driver.engine().state_hash());
    driver.begin_cast("Burst Lightning")?;
    driver.expect_decision(0, "choose-costs")?;
    driver.answer(Answer::ChooseCosts(CostsChoice::Cancel))?;
    assert_eq!(
        (driver.engine().events().len(), driver.engine().state_hash()),
        before
    );
    driver.expect_decision(0, "priority")?;
    Ok(())
}

#[rstest]
#[case::two_lands_cannot_kick(2, None, false)]
#[case::three_lands_unkicked(3, Some(false), false)]
#[case::three_lands_kicked(3, Some(true), true)]
fn gatekeeper_of_malakir_makes_a_player_sacrifice_a_creature_only_when_kicked(
    #[case] swamps: usize,
    #[case] kicked: Option<bool>,
    #[case] sacrifices: bool,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(table("Gatekeeper of Malakir", lands(swamps, swamp)))?;
    driver.begin_cast("Gatekeeper of Malakir")?;
    if let Some(kicked) = kicked {
        kick(&mut driver, if kicked { &[0] } else { &[] })?;
    }
    pay(&mut driver)?;
    resolve(&mut driver)?;
    assert_eq!(count(&driver, "Gatekeeper of Malakir"), 1);
    if sacrifices {
        // The trigger asks for its target as it goes on the stack.
        let DecisionKind::ChooseTargets(options) = &driver.decision()?.kind else {
            panic!("a kicked Gatekeeper triggers");
        };
        assert!(options.triggered);
        let foe = driver.target_player(1);
        driver.choose_targets(&[foe])?;
        resolve(&mut driver)?;
        assert_eq!(count(&driver, "Bear"), 0, "the Bear was sacrificed");
    } else {
        // Not kicked: the "if" is false as it enters, so nothing is put on the stack.
        assert_eq!(driver.state().stack().len(), 0);
        driver.expect_decision(0, "priority")?;
        assert_eq!(count(&driver, "Bear"), 1);
    }
    Ok(())
}

#[rstest]
#[case::four_lands_cannot_kick(4, 0, None, 0)]
#[case::five_lands_none(5, 0, Some(1), 0)]
#[case::five_lands_one(5, 1, Some(1), 1)]
#[case::seven_lands_none(7, 0, Some(3), 0)]
#[case::seven_lands_two(7, 2, Some(3), 2)]
#[case::seven_lands_all_three(7, 3, Some(3), 3)]
fn wolfbriar_elemental_makes_a_wolf_for_each_time_it_was_kicked_and_asks_up_to_what_the_lands_can_pay(
    #[case] forests: usize,
    #[case] kicks: u32,
    #[case] most: Option<u32>,
    #[case] wolves: usize,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(table("Wolfbriar Elemental", lands(forests, forest)))?;
    driver.begin_cast("Wolfbriar Elemental")?;
    if let Some(most) = most {
        let DecisionKind::ChooseNumber(options) = &driver.decision()?.kind else {
            panic!("the times come first");
        };
        assert_eq!((options.min, options.max), (0, Some(most)));
        assert_eq!(
            options.text,
            "Choose how many times to pay Multikicker {G}."
        );
        assert!(options.can_cancel);
        times(&mut driver, kicks)?;
    }
    let paid = driver.pay_options()?.cost.mana_value();
    assert_eq!(paid, 4 + kicks);
    pay(&mut driver)?;
    assert_eq!(tapped(&driver), (4 + kicks) as usize);
    resolve(&mut driver)?;
    // The trigger waits on the stack for its controller's pass.
    if wolves > 0 {
        resolve(&mut driver)?;
    }
    assert_eq!(count(&driver, "Wolfbriar Elemental"), 1);
    assert_eq!(count(&driver, "Wolf"), wolves);
    Ok(())
}

#[test]
fn a_number_of_times_beyond_what_can_be_paid_is_refused_and_backing_out_leaves_no_trace()
-> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(table("Wolfbriar Elemental", lands(6, forest)))?;
    let before = (driver.engine().events().len(), driver.engine().state_hash());
    driver.begin_cast("Wolfbriar Elemental")?;
    driver.expect_decision(0, "choose-number")?;
    let refused = driver.answer(Answer::ChooseNumber(NumberChoice::Number(3)));
    assert!(
        matches!(
            refused,
            Err(ScriptError::Engine(EngineError::Illegal(
                IllegalAnswer::NotOffered("number")
            )))
        ),
        "{refused:?}"
    );
    driver.expect_decision(0, "choose-number")?;
    driver.answer(Answer::ChooseNumber(NumberChoice::Cancel))?;
    assert_eq!(
        (driver.engine().events().len(), driver.engine().state_hash()),
        before
    );
    Ok(())
}

#[test]
fn what_was_paid_is_logged_with_the_cast_stays_with_the_permanent_and_travels_with_its_trigger()
-> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(table("Wolfbriar Elemental", lands(6, forest)))?;
    driver.begin_cast("Wolfbriar Elemental")?;
    times(&mut driver, 2)?;
    let events = pay(&mut driver)?;
    let cast = events
        .iter()
        .find_map(|event| match event {
            Event::SpellCast { choices, .. } => Some(choices.clone()),
            _ => None,
        })
        .expect("the spell was cast");
    assert_eq!(cast.optional_costs, [0, 0], "a payment each time");
    resolve(&mut driver)?;
    let id = driver.id_of("Wolfbriar Elemental")?;
    let permanent = driver.state().object(id).unwrap().permanent().unwrap();
    assert_eq!(permanent.paid(), [0, 0]);
    // The trigger is on the stack, carrying what its source paid.
    let top = *driver.state().stack().last().unwrap();
    let ability = driver.state().object(top).unwrap();
    assert_eq!(ability.choices().optional_costs, [0, 0]);
    Ok(())
}

#[test]
fn the_trigger_of_a_kicked_permanent_that_left_still_counts_the_kicks() -> Result<(), ScriptError> {
    let blade = card("Doom Blade").unwrap();
    let scenario = table("Wolfbriar Elemental", lands(6, forest))
        .hand(1, [blade])
        .battlefield(1, [ready(swamp()), ready(swamp())]);
    let mut driver = Driver::from_scenario(scenario)?;
    driver.begin_cast("Wolfbriar Elemental")?;
    times(&mut driver, 2)?;
    pay(&mut driver)?;
    resolve(&mut driver)?;
    // The enters trigger waits; its controller passes and the opponent destroys the Elemental in response.
    driver.pass()?;
    driver.expect_decision(1, "priority")?;
    let elemental = driver.target_named("Wolfbriar Elemental")?;
    driver.begin_cast("Doom Blade")?;
    driver.choose_targets(&[elemental])?;
    driver.answer(Answer::PayMana(PayChoice::Auto))?;
    driver.pass()?;
    driver.pass()?;
    assert_eq!(count(&driver, "Wolfbriar Elemental"), 0, "it was destroyed");
    // The trigger resolves with the kicks of the permanent as it was.
    resolve(&mut driver)?;
    assert_eq!(count(&driver, "Wolf"), 2);
    Ok(())
}

#[test]
fn bot_games_with_kicked_cards_keep_every_invariant_and_use_the_optional_costs() {
    let seen = std::sync::Mutex::new((0u64, 0u64, 0u64, 0u64));
    let games = fuzz_games(40);
    for_each_seed(0..games, |seed| {
        for players in [2, 4] {
            let outcome = play_game_with(
                kicker_fuzz_setup(players, seed),
                seed,
                PlayOptions::default(),
            )
            .unwrap_or_else(|failure| panic!("{failure}"));
            let mut seen = seen.lock().unwrap();
            for event in &outcome.events {
                match event {
                    Event::SpellCast { choices, .. } if choices.optional_costs.is_empty() => {
                        seen.0 += 1;
                    }
                    Event::SpellCast { choices, .. } => {
                        seen.1 += 1;
                        if choices.optional_costs.len() > 1 {
                            seen.2 += 1;
                        }
                    }
                    Event::AbilityPutOnStack(put) if !put.choices.optional_costs.is_empty() => {
                        seen.3 += 1;
                    }
                    _ => {}
                }
            }
        }
    });
    let (plain, kicked, many, triggers) = seen.into_inner().unwrap();
    assert!(plain > 0, "some spells were cast without a kick");
    assert!(kicked > 0, "some spells were kicked");
    assert!(many > 0, "some multikicker was paid more than once");
    assert!(triggers > 0, "some trigger carried what its source paid");
}
