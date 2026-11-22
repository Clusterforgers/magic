//! Goal: an activated ability of a spell on the stack that any player may activate (CR 113.6, 602.1) plays at tables of two to four seats: an opponent of Lightning Storm's caster discards a land card while Storm is on the stack, two charge counters go on the spell (CR 122.1), they may choose a new target for it, and the spell deals 3 plus its charge counters to the target it has as it resolves; the ability is offered to every player while the spell is on the stack only, a player without a land card is not offered it, and the log replays to the live state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_cards::value::CounterKind;
use mtg_engine::choose::YesNoChoice;
use mtg_engine::decision::{Answer, DecisionKind};
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_engine::{Event, replay};
use mtg_testkit::cards::{forest, mountain};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

fn table(seats: usize, lands_in_hand: bool) -> Scenario {
    let mut scenario = Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .hand(0, [card("Lightning Storm").unwrap()])
        .battlefield(0, (0..3).map(|_| ready(mountain())).collect::<Vec<_>>());
    for seat in 1..seats {
        if lands_in_hand {
            scenario = scenario.hand(seat, [forest(), forest()]);
        }
    }
    scenario
}

fn life(driver: &Driver, seat: usize) -> i32 {
    driver
        .state()
        .player(mtg_core::ids::PlayerId::from_index(seat))
        .unwrap()
        .life()
}

/// Whether the deciding player is offered an ability of the spell on the stack.
fn offered(driver: &Driver) -> bool {
    match &driver.engine().decision().unwrap().kind {
        DecisionKind::Priority(options) => options.abilities.iter().any(|ability| {
            driver
                .state()
                .object(ability.object)
                .map(mtg_engine::object::GameObject::zone)
                == Some(Zone::Stack)
        }),
        _ => false,
    }
}

/// Answers until priority comes with the stack empty: a new target is the seat `retarget`, other questions passively.
fn settle(driver: &mut Driver, retarget: Option<usize>) -> Result<(), ScriptError> {
    for _ in 0..80 {
        let decision = driver.decision()?.clone();
        match &decision.kind {
            DecisionKind::Priority(_)
                if driver.state().stack().is_empty()
                    && driver.state().pending_triggers().is_empty() =>
            {
                return Ok(());
            }
            DecisionKind::Priority(_) => {
                driver.pass()?;
            }
            DecisionKind::YesNo(_) => {
                driver.answer(Answer::YesNo(YesNoChoice(retarget.is_some())))?;
            }
            DecisionKind::ChooseTargets(_) if retarget.is_some() => {
                let to = driver.target_player(retarget.unwrap());
                driver.choose_targets(&[to])?;
            }
            _ => {
                driver.answer_passively()?;
            }
        }
    }
    panic!("the stack did not empty");
}

#[rstest]
fn an_opponent_charges_the_storm_and_turns_it(
    #[values(2, 3, 4)] seats: usize,
    #[values(None, Some(0))] retarget: Option<usize>,
) -> Result<(), ScriptError> {
    let scenario = table(seats, true);
    let mut driver = Driver::from_scenario(scenario.clone())?;
    assert!(!offered(&driver), "no ability before the spell is cast");
    let at = driver.target_player(1);
    driver.cast_at("Lightning Storm", &[at])?;
    let storm = driver.state().zone(Zone::Stack)[0];
    assert!(!offered(&driver), "its caster holds no land card");
    driver.pass()?;
    assert!(offered(&driver), "any player may activate it");
    driver.begin_activation("Lightning Storm")?;
    settle(&mut driver, retarget)?;
    let events = driver.engine().events();
    let charged = events.iter().any(|event| {
        matches!(event, Event::CountersChanged(change)
            if change.on.object().is_some_and(|on| on.id == storm)
                && change.counter == CounterKind::Charge
                && change.delta == 2)
    });
    assert!(charged, "two charge counters on the spell");
    let discarded = events
        .iter()
        .filter(|event| matches!(event, Event::CardDiscarded { player, .. } if player.0 == 1))
        .count();
    assert_eq!(discarded, 1, "a land card paid the cost");
    match retarget {
        Some(seat) => {
            assert_eq!(life(&driver, seat), 15);
            assert_eq!(life(&driver, 1), 20);
        }
        None => assert_eq!(life(&driver, 1), 15, "3 plus two charge counters"),
    }
    let setup = scenario.setup().unwrap();
    let replayed = replay(&setup, driver.engine().events()).unwrap();
    assert_eq!(replayed.state_hash(), driver.engine().state_hash());
    Ok(())
}

#[rstest]
fn without_a_land_card_nothing_is_offered(#[values(2, 3)] seats: usize) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(table(seats, false))?;
    let at = driver.target_player(1);
    driver.cast_at("Lightning Storm", &[at])?;
    driver.pass()?;
    assert!(!offered(&driver));
    settle(&mut driver, None)?;
    assert_eq!(life(&driver, 1), 17, "3 damage with no counters");
    Ok(())
}
