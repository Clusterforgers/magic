//! Goal: empower Jace plays at two to four seats as the Reality Fracture rulings say: with no Jace token a blue Jace
//! planeswalker token is created and gets the counters, a second empower adds to that token instead of making another;
//! Way of the Mind Sculptor draws only for a loyalty ability whose cost removed two or more counters; Theorist's Sanctum
//! enters tapped unless its controller beholds a Jace.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_cards::value::CounterKind;
use mtg_core::ids::PlayerId;
use mtg_engine::choose::{CardsChoice, YesNoChoice};
use mtg_engine::decision::Answer;
use mtg_engine::decision::DecisionKind;
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{free_creature, island};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

const ME: PlayerId = PlayerId(0);

/// Priority passes until the stack is empty.
fn r5508_settle(driver: &mut Driver) -> Result<(), ScriptError> {
    for _ in 0..60 {
        if !matches!(driver.decision()?.kind, DecisionKind::Priority(_))
            || driver.state().stack().is_empty()
        {
            break;
        }
        driver.pass()?;
    }
    Ok(())
}

/// Seat 0 with `islands` ready Islands, `battlefield` and `hand`, a library of twenty Islands, every other seat with a
/// 1/1, in seat 0's first main phase.
fn r5508_table(
    seats: usize,
    islands: usize,
    battlefield: &[&str],
    hand: &[&str],
) -> Result<Driver, ScriptError> {
    let mut mine: Vec<_> = (0..islands).map(|_| ready(island())).collect();
    mine.extend(battlefield.iter().map(|name| ready(card(name).unwrap())));
    let mut scenario = Scenario::new(seats)
        .battlefield(0, mine)
        .hand(0, hand.iter().map(|name| card(name).unwrap()))
        .library(0, (0..20).map(|_| island()));
    for seat in 1..seats {
        scenario = scenario.battlefield(seat, vec![ready(free_creature("Pip", 1, 1))]);
    }
    Driver::from_scenario(scenario.at(1, 0, Step::Main1))
}

/// The loyalty of each Jace token seat 0 controls.
fn r5508_jaces(driver: &Driver) -> Vec<u32> {
    let state = driver.state();
    state
        .zone(Zone::Battlefield)
        .iter()
        .filter_map(|id| state.object(*id))
        .filter(|object| object.card().name == "Jace" && object.controller() == ME)
        .filter_map(|object| {
            object
                .permanent()
                .map(|p| p.counters_of(&CounterKind::Loyalty))
        })
        .collect()
}

fn r5508_hand(driver: &Driver) -> usize {
    driver.state().zone(Zone::Hand(ME)).len()
}

/// Answers a yes-or-no question of the replacement.
fn r5508_say(driver: &mut Driver, yes: bool) -> Result<(), ScriptError> {
    assert!(matches!(driver.decision()?.kind, DecisionKind::YesNo(_)));
    driver.answer(Answer::YesNo(YesNoChoice(yes)))?;
    Ok(())
}

#[rstest]
#[case::two_seats(2)]
#[case::three_seats(3)]
#[case::four_seats(4)]
fn r5508_empower_creates_the_token_once_and_adds_to_it(
    #[case] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = r5508_table(
        seats,
        8,
        &["Theorist's Sanctum"],
        &["Way of the Mind Sculptor"],
    )?;
    driver.cast("Way of the Mind Sculptor")?;
    r5508_settle(&mut driver)?;
    assert_eq!(
        r5508_jaces(&driver),
        vec![5],
        "a new Jace token with five loyalty"
    );
    driver.begin_activation("Theorist's Sanctum")?;
    driver.pay_tapping(&["Island", "Island", "Island"], 0)?;
    r5508_settle(&mut driver)?;
    assert_eq!(
        r5508_jaces(&driver),
        vec![7],
        "the counters go on the token there is"
    );
    Ok(())
}

#[rstest]
#[case::surveil_two_seats(2, 0, 0)]
#[case::draw_two_seats(2, 1, 2)]
#[case::draw_three_seats(3, 1, 2)]
#[case::surveil_four_seats(4, 0, 0)]
fn r5508_way_draws_for_two_or_more_counters_removed(
    #[case] seats: usize,
    #[case] ability: usize,
    #[case] drawn: usize,
) -> Result<(), ScriptError> {
    let mut driver = r5508_table(seats, 5, &[], &["Way of the Mind Sculptor"])?;
    driver.cast("Way of the Mind Sculptor")?;
    r5508_settle(&mut driver)?;
    let before = r5508_hand(&driver);
    driver.begin_ability("Jace", ability)?;
    for _ in 0..10 {
        match driver.decision()?.kind {
            DecisionKind::Priority(_) if driver.state().stack().is_empty() => break,
            DecisionKind::Priority(_) => {
                driver.pass()?;
            }
            // Surveil 1: keep the card on top.
            DecisionKind::ChooseCards(_) => {
                driver.answer(Answer::ChooseCards(CardsChoice(Vec::new())))?;
            }
            ref other => panic!("unexpected {other:?}"),
        }
    }
    assert_eq!(r5508_hand(&driver) - before, drawn);
    Ok(())
}

#[rstest]
#[case::no_jace_two_seats(2, false, true)]
#[case::no_jace_four_seats(4, false, true)]
#[case::a_jace_token_three_seats(3, true, false)]
fn r5508_sanctum_enters_tapped_unless_a_jace_is_beheld(
    #[case] seats: usize,
    #[case] jace: bool,
    #[case] tapped: bool,
) -> Result<(), ScriptError> {
    let mut driver = r5508_table(
        seats,
        5,
        &[],
        &["Way of the Mind Sculptor", "Theorist's Sanctum"],
    )?;
    if jace {
        driver.cast("Way of the Mind Sculptor")?;
        r5508_settle(&mut driver)?;
    }
    driver.play_land("Theorist's Sanctum")?;
    r5508_say(&mut driver, jace)?;
    let state = driver.state();
    let sanctum = state
        .zone(Zone::Battlefield)
        .iter()
        .filter_map(|id| state.object(*id))
        .find(|object| object.card().name == "Theorist's Sanctum")
        .unwrap();
    assert_eq!(sanctum.permanent().unwrap().tapped(), tapped);
    let revealed = driver.state().zone(Zone::Hand(ME)).len();
    assert_eq!(
        revealed,
        usize::from(!jace),
        "the Way stays in hand only when it was not cast"
    );
    Ok(())
}
