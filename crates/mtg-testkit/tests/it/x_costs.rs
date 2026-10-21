//! Goal: an `{X}` in the cost of a spell or an activated ability and the `-X` of a loyalty ability are announced with `ChooseNumber` from 0 to the most the player can pay (the counters the planeswalker holds for `-X`), the question is left out when only 0 is possible, a spell or ability that cannot pay even X = 0 is not offered, an answer above the maximum is refused and asked again, X adds generic mana to the total cost once for each `{X}` beside hybrid symbols and colours, the announcement can be backed out of with no trace, the X announced is kept in the choices of the spell or ability on the stack and read by its effects when it resolves, a `-X` removes that many counters and its ability resolves without its planeswalker, and the log of such games replays to the live state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common;

use common::pid;
use mtg_cards::value::CounterKind;
use mtg_engine::choose::NumberChoice;
use mtg_engine::decision::{DecisionKind, PayChoice};
use mtg_engine::zones::Zone;
use mtg_engine::{Answer, EngineError, Event, GameSetup, replay};
use mtg_testkit::cards::{island, mountain, plains};
use mtg_testkit::x_cards::{
    blaze, hybrid_font, stroke, twin_font, twin_well, x_pinger, x_walker,
};
use mtg_testkit::{Driver, Scenario, ScriptError, ready, with_counters};
use rstest::rstest;

fn life(driver: &Driver, seat: usize) -> i32 {
    driver.state().player(pid(seat)).unwrap().life()
}

fn replays(driver: &Driver, setup: &GameSetup) {
    let replayed = replay(setup, driver.engine().events()).unwrap();
    assert_eq!(replayed.state_hash(), driver.state().state_hash());
}

/// Answers passively until nothing is on the stack.
fn settle(driver: &mut Driver) -> Result<(), ScriptError> {
    for _ in 0..200 {
        if driver.state().stack().is_empty() && driver.state().resolving().is_none() {
            return Ok(());
        }
        driver.answer_passively()?;
    }
    Err(ScriptError::Stuck("the stack".into()))
}

/// The most X the pending question offers, or `None` when the next question is not about X.
fn most_x(driver: &Driver) -> Option<u32> {
    match &driver.decision().unwrap().kind {
        DecisionKind::ChooseNumber(options) => {
            assert_eq!(options.min, 0, "X starts at nothing");
            options.max
        }
        _ => None,
    }
}

fn say(driver: &mut Driver, x: u32) -> Result<Vec<Event>, ScriptError> {
    driver.answer(Answer::ChooseNumber(NumberChoice::Number(x)))
}

/// The choices of the spell cast or ability put on the stack in these events.
fn announced(events: &[Event]) -> mtg_engine::choose::Choices {
    events
        .iter()
        .find_map(|event| match event {
            Event::SpellCast { choices, .. } => Some(choices.clone()),
            Event::AbilityPutOnStack(put) => Some(put.choices.clone()),
            _ => None,
        })
        .expect("a spell was cast or an ability put on the stack")
}

/// The generic mana of the payment the driver is asked for.
fn generic_to_pay(driver: &Driver) -> u32 {
    driver.pay_options().unwrap().cost.generic()
}

fn lands(count: usize, land: fn() -> std::sync::Arc<mtg_cards::CardDefinition>) -> Vec<mtg_engine::PresetPermanent> {
    (0..count).map(|_| ready(land())).collect()
}

fn blaze_with(mountains: usize) -> Scenario {
    Scenario::new(2)
        .hand(0, [blaze().unwrap()])
        .battlefield(0, lands(mountains, mountain))
}

// The question

#[rstest]
#[case::one_land_pays_the_colour_and_nothing_more(1, 0)]
#[case::two_lands(2, 1)]
#[case::three_lands(3, 2)]
#[case::six_lands(6, 5)]
fn x_is_asked_up_to_what_is_left_after_the_rest_of_the_cost(
    #[case] mountains: usize,
    #[case] most: u32,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(blaze_with(mountains))?;
    driver.begin_cast("Blaze")?;
    if most == 0 {
        // Nothing to choose between: X is 0 and the targets come first.
        driver.expect_decision(0, "choose-targets")?;
    } else {
        assert_eq!(most_x(&driver), Some(most));
        let DecisionKind::ChooseNumber(options) = &driver.decision()?.kind else {
            unreachable!()
        };
        assert!(options.can_cancel, "nothing has been paid");
        assert_eq!(options.source, driver.state().zone(Zone::Hand(pid(0)))[0]);
    }
    Ok(())
}

#[test]
fn a_spell_that_cannot_pay_for_x_of_nothing_is_not_offered() -> Result<(), ScriptError> {
    // {X}{R} with no red source, and with no land at all.
    for scenario in [
        Scenario::new(2)
            .hand(0, [blaze().unwrap()])
            .battlefield(0, lands(3, island)),
        blaze_with(0),
    ] {
        let mut driver = Driver::from_scenario(scenario)?;
        let (_, options) = (0, driver.decision()?.kind.clone());
        let DecisionKind::Priority(options) = options else {
            panic!("priority was expected");
        };
        assert!(options.castable.is_empty());
        let refused = driver.begin_cast("Blaze");
        assert!(refused.is_err(), "{refused:?}");
    }
    Ok(())
}

#[test]
fn an_answer_above_the_most_the_player_can_pay_is_refused_and_asked_again() -> Result<(), ScriptError>
{
    let mut driver = Driver::from_scenario(blaze_with(3))?;
    driver.begin_cast("Blaze")?;
    let before = driver.engine().events().len();
    let refused = say(&mut driver, 3);
    assert!(
        matches!(refused, Err(ScriptError::Engine(EngineError::Illegal(_)))),
        "{refused:?}"
    );
    assert_eq!(driver.engine().events().len(), before);
    assert_eq!(most_x(&driver), Some(2));
    Ok(())
}

#[test]
fn backing_out_at_x_leaves_no_trace() -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(blaze_with(4))?;
    let before = (driver.engine().events().len(), driver.engine().state_hash());
    driver.begin_cast("Blaze")?;
    driver.answer(Answer::ChooseNumber(NumberChoice::Cancel))?;
    assert_eq!(
        (driver.engine().events().len(), driver.engine().state_hash()),
        before
    );
    driver.expect_decision(0, "priority")?;
    // The same spell can be cast again from the start.
    driver.begin_cast("Blaze")?;
    assert_eq!(most_x(&driver), Some(3));
    Ok(())
}

// The total cost

#[rstest]
#[case::nothing(0, 1)]
#[case::one(1, 2)]
#[case::the_most(3, 4)]
fn x_adds_generic_mana_to_the_cost_and_the_spell_remembers_it_while_it_is_on_the_stack(
    #[case] x: u32,
    #[case] paid: usize,
) -> Result<(), ScriptError> {
    let scenario = || blaze_with(4);
    let setup = scenario().setup().unwrap();
    let mut driver = Driver::from_scenario(scenario())?;
    driver.begin_cast("Blaze")?;
    say(&mut driver, x)?;
    let foe = driver.target_player(1);
    driver.choose_targets(&[foe])?;
    assert_eq!(generic_to_pay(&driver), x, "X generic mana beside the {{R}}");
    let events = driver.answer(Answer::PayMana(PayChoice::Auto))?;
    let choices = announced(&events);
    assert_eq!(choices.x, Some(x));
    assert_eq!(choices.targets, [vec![foe]]);
    let spent = events
        .iter()
        .filter(|event| matches!(event, Event::PermanentTapped { .. }))
        .count();
    assert_eq!(spent, paid);
    // On the stack the spell holds the number it was cast for.
    let on_the_stack = driver.state().object(driver.state().stack()[0]).unwrap();
    assert_eq!(on_the_stack.choices().x, Some(x));
    settle(&mut driver)?;
    assert_eq!(life(&driver, 1), 20 - i32::try_from(x).unwrap());
    assert_eq!(
        common::names(driver.state(), Zone::Graveyard(pid(0))),
        ["Blaze"]
    );
    replays(&driver, &setup);
    Ok(())
}

#[rstest]
#[case::nothing(0)]
#[case::one(1)]
#[case::two(2)]
#[case::the_most(3)]
fn each_x_costs_x_so_a_double_x_spell_costs_twice_as_much(
    #[case] x: u32,
) -> Result<(), ScriptError> {
    // {X}{X}{W} with seven Plains: 2X + 1 is at most 7.
    let mut driver = Driver::from_scenario(
        Scenario::new(2)
            .hand(0, [twin_font().unwrap()])
            .battlefield(0, lands(7, plains)),
    )?;
    driver.begin_cast("Twin Font")?;
    assert_eq!(most_x(&driver), Some(3));
    say(&mut driver, x)?;
    assert_eq!(generic_to_pay(&driver), 2 * x);
    driver.answer(Answer::PayMana(PayChoice::Auto))?;
    settle(&mut driver)?;
    assert_eq!(life(&driver, 0), 20 + i32::try_from(x).unwrap());
    Ok(())
}

#[rstest]
#[case::an_odd_number_of_lands(6, 2)]
#[case::an_even_number(5, 2)]
#[case::just_the_colour(1, 0)]
fn a_double_x_is_bounded_by_half_of_what_is_left(
    #[case] plains_in_play: usize,
    #[case] most: u32,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(
        Scenario::new(2)
            .hand(0, [twin_font().unwrap()])
            .battlefield(0, lands(plains_in_play, plains)),
    )?;
    driver.begin_cast("Twin Font")?;
    if most == 0 {
        driver.expect_decision(0, "pay-mana")?;
    } else {
        assert_eq!(most_x(&driver), Some(most));
    }
    Ok(())
}

#[rstest]
#[case::paid_with_the_red_land(&["Mountain", "Island", "Island"], 2)]
#[case::paid_with_the_white_land(&["Plains", "Island", "Island", "Island"], 3)]
#[case::the_hybrid_symbol_alone(&["Mountain"], 0)]
fn a_hybrid_symbol_beside_x_is_paid_with_either_colour_and_x_with_anything(
    #[case] lands_in_play: &[&str],
    #[case] most: u32,
) -> Result<(), ScriptError> {
    let land = |name: &str| match name {
        "Mountain" => ready(mountain()),
        "Plains" => ready(plains()),
        _ => ready(island()),
    };
    let mut driver = Driver::from_scenario(
        Scenario::new(2)
            .hand(0, [hybrid_font().unwrap()])
            .battlefield(0, lands_in_play.iter().map(|name| land(name))),
    )?;
    driver.begin_cast("Hybrid Font")?;
    if most == 0 {
        driver.expect_decision(0, "pay-mana")?;
        return Ok(());
    }
    assert_eq!(most_x(&driver), Some(most));
    say(&mut driver, most)?;
    driver.answer(Answer::PayMana(PayChoice::Auto))?;
    settle(&mut driver)?;
    assert_eq!(life(&driver, 0), 20 + i32::try_from(most).unwrap());
    Ok(())
}

#[test]
fn a_hybrid_symbol_that_no_land_can_pay_keeps_the_spell_out_of_reach() -> Result<(), ScriptError> {
    let driver = Driver::from_scenario(
        Scenario::new(2)
            .hand(0, [hybrid_font().unwrap()])
            .battlefield(0, lands(4, island)),
    )?;
    let DecisionKind::Priority(options) = &driver.decision()?.kind else {
        panic!("priority was expected");
    };
    assert!(options.castable.is_empty());
    Ok(())
}

#[test]
fn x_is_asked_before_the_targets_and_the_payment_and_reads_in_the_effect() -> Result<(), ScriptError>
{
    let mut driver = Driver::from_scenario(
        Scenario::new(2)
            .hand(0, [stroke().unwrap()])
            .battlefield(0, lands(5, island)),
    )?;
    driver.begin_cast("Stroke")?;
    driver.expect_decision(0, "choose-number")?;
    say(&mut driver, 3)?;
    driver.expect_decision(0, "choose-targets")?;
    let me = driver.target_player(0);
    driver.choose_targets(&[me])?;
    driver.expect_decision(0, "pay-mana")?;
    assert_eq!(generic_to_pay(&driver), 3);
    // The spell leaves the hand when it is paid for, and three cards come in.
    let hand = driver.state().zone(Zone::Hand(pid(0))).len();
    driver.answer(Answer::PayMana(PayChoice::Auto))?;
    settle(&mut driver)?;
    assert_eq!(driver.state().zone(Zone::Hand(pid(0))).len(), hand - 1 + 3);
    Ok(())
}

// Abilities

fn pinger(lands_in_play: usize) -> Scenario {
    let mut permanents = vec![ready(x_pinger().unwrap())];
    permanents.extend(lands(lands_in_play, mountain));
    Scenario::new(2).battlefield(0, permanents)
}

#[rstest]
#[case::nothing(0)]
#[case::one(1)]
#[case::the_most(3)]
fn an_ability_with_x_in_its_mana_cost_asks_x_and_pays_for_it(
    #[case] x: u32,
) -> Result<(), ScriptError> {
    let scenario = || pinger(4);
    let setup = scenario().setup().unwrap();
    let mut driver = Driver::from_scenario(scenario())?;
    driver.begin_activation("X Pinger")?;
    assert_eq!(most_x(&driver), Some(3), "{{X}}{{R}} of four lands");
    say(&mut driver, x)?;
    let foe = driver.target_player(1);
    driver.choose_targets(&[foe])?;
    assert_eq!(generic_to_pay(&driver), x);
    let events = driver.answer(Answer::PayMana(PayChoice::Auto))?;
    assert_eq!(announced(&events).x, Some(x));
    assert!(
        events
            .iter()
            .any(|event| matches!(event, Event::PermanentTapped { .. }))
    );
    // The ability on the stack holds its X, and the permanent it came from is tapped.
    let ability = driver.state().object(driver.state().stack()[0]).unwrap();
    assert_eq!(ability.choices().x, Some(x));
    settle(&mut driver)?;
    assert_eq!(life(&driver, 1), 20 - i32::try_from(x).unwrap());
    replays(&driver, &setup);
    Ok(())
}

#[test]
fn an_ability_that_needs_a_tap_does_not_pay_from_its_own_source() -> Result<(), ScriptError> {
    // {X}{R}, {T} from an artifact that makes no mana: the lands pay all of it.
    let mut driver = Driver::from_scenario(pinger(2))?;
    driver.begin_activation("X Pinger")?;
    assert_eq!(most_x(&driver), Some(1));
    Ok(())
}

#[test]
fn an_ability_that_cannot_pay_x_of_nothing_is_not_offered() -> Result<(), ScriptError> {
    let driver = Driver::from_scenario(pinger(0))?;
    let DecisionKind::Priority(options) = &driver.decision()?.kind else {
        panic!("priority was expected");
    };
    assert!(options.abilities.is_empty());
    Ok(())
}

#[test]
fn a_double_x_ability_with_nothing_else_is_offered_for_nothing_and_asks_no_number()
-> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(
        Scenario::new(2).battlefield(0, [ready(twin_well().unwrap())]),
    )?;
    driver.begin_activation("Twin Well")?;
    // X can only be 0: nothing to ask.
    let events = driver.answer(Answer::PayMana(PayChoice::Auto))?;
    assert_eq!(announced(&events).x, Some(0));
    settle(&mut driver)?;
    assert_eq!(life(&driver, 0), 20);
    Ok(())
}

#[rstest]
#[case::two_lands(2, 1)]
#[case::five_lands(5, 2)]
fn a_double_x_ability_costs_twice_x(
    #[case] mountains: usize,
    #[case] most: u32,
) -> Result<(), ScriptError> {
    let mut permanents = vec![ready(twin_well().unwrap())];
    permanents.extend(lands(mountains, mountain));
    let mut driver = Driver::from_scenario(Scenario::new(2).battlefield(0, permanents))?;
    driver.begin_activation("Twin Well")?;
    assert_eq!(most_x(&driver), Some(most));
    say(&mut driver, most)?;
    assert_eq!(generic_to_pay(&driver), 2 * most);
    driver.answer(Answer::PayMana(PayChoice::Auto))?;
    settle(&mut driver)?;
    assert_eq!(life(&driver, 0), 20 + i32::try_from(most).unwrap());
    Ok(())
}

// Loyalty

const LOYALTY: CounterKind = CounterKind::Loyalty;

fn walker(loyalty: u32) -> Scenario {
    Scenario::new(2).battlefield(
        0,
        [with_counters(ready(x_walker().unwrap()), LOYALTY, loyalty)],
    )
}

fn loyalty_of(driver: &Driver) -> Option<u32> {
    let id = driver.id_of("X Walker").ok()?;
    driver
        .state()
        .object(id)
        .and_then(|object| object.permanent())
        .map(|permanent| permanent.counters_of(&LOYALTY))
}

#[rstest]
#[case::nothing(0, 4, 0)]
#[case::some(2, 4, 2)]
#[case::all_of_it(4, 4, 4)]
fn a_minus_x_ability_removes_x_counters_and_deals_x_damage_after_its_planeswalker_has_gone_or_not(
    #[case] x: u32,
    #[case] loyalty: u32,
    #[case] damage: i32,
) -> Result<(), ScriptError> {
    let scenario = || walker(loyalty);
    let setup = scenario().setup().unwrap();
    let mut driver = Driver::from_scenario(scenario())?;
    driver.begin_ability("X Walker", 1)?;
    assert_eq!(most_x(&driver), Some(loyalty), "up to the counters");
    say(&mut driver, x)?;
    let foe = driver.target_player(1);
    let events = driver.choose_targets(&[foe])?;
    assert_eq!(announced(&events).x, Some(x));
    let removed: Vec<i32> = events
        .iter()
        .filter_map(|event| match event {
            Event::CountersChanged(changed) if changed.counter == LOYALTY => Some(changed.delta),
            _ => None,
        })
        .collect();
    assert_eq!(
        removed,
        if x == 0 {
            Vec::new()
        } else {
            vec![-i32::try_from(x).unwrap()]
        },
        "the counters are paid with the cost, before the ability is put on the stack"
    );
    assert!(
        events
            .iter()
            .any(|event| matches!(event, Event::AbilityActivated { .. })),
        "X = 0 still activates the ability"
    );
    settle(&mut driver)?;
    assert_eq!(life(&driver, 1), 20 - damage);
    if x == loyalty {
        assert_eq!(loyalty_of(&driver), None, "no loyalty, no planeswalker");
        assert_eq!(
            common::names(driver.state(), Zone::Graveyard(pid(0))),
            ["X Walker"]
        );
    } else {
        assert_eq!(loyalty_of(&driver), Some(loyalty - x));
    }
    replays(&driver, &setup);
    Ok(())
}

#[test]
fn a_minus_x_ability_is_used_once_each_turn_like_the_others() -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(walker(4))?;
    driver.begin_ability("X Walker", 1)?;
    say(&mut driver, 1)?;
    let foe = driver.target_player(1);
    driver.choose_targets(&[foe])?;
    settle(&mut driver)?;
    assert_eq!(driver.offered_abilities("X Walker")?, Vec::<usize>::new());
    Ok(())
}

#[test]
fn a_planeswalker_with_loyalty_offers_minus_x_beside_its_other_abilities() -> Result<(), ScriptError>
{
    let driver = Driver::from_scenario(walker(1))?;
    assert_eq!(driver.offered_abilities("X Walker")?, vec![0, 1]);
    Ok(())
}

#[test]
fn backing_out_of_a_minus_x_leaves_the_counters_and_the_log_alone() -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(walker(4))?;
    let before = (driver.engine().events().len(), driver.engine().state_hash());
    driver.begin_ability("X Walker", 1)?;
    driver.answer(Answer::ChooseNumber(NumberChoice::Cancel))?;
    assert_eq!(
        (driver.engine().events().len(), driver.engine().state_hash()),
        before
    );
    assert_eq!(loyalty_of(&driver), Some(4));
    driver.expect_decision(0, "priority")?;
    Ok(())
}

#[test]
fn an_answer_above_the_loyalty_is_refused() -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(walker(3))?;
    driver.begin_ability("X Walker", 1)?;
    let refused = say(&mut driver, 4);
    assert!(
        matches!(refused, Err(ScriptError::Engine(EngineError::Illegal(_)))),
        "{refused:?}"
    );
    assert_eq!(most_x(&driver), Some(3));
    assert_eq!(loyalty_of(&driver), Some(3));
    Ok(())
}
