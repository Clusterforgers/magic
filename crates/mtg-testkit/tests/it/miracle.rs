//! Goal: a card with miracle that its owner draws as the first card of the turn (in the draw step or by an effect) puts a trigger on the stack at once, the trigger asks whether to cast it for its miracle cost and casts it inside the resolution, a second draw of the turn, a decline, a card that left the hand in response, a cost that cannot be paid and an opponent's draw each leave the card as it was, at two, three and four seats.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_cards::value::CastWay;
use mtg_core::ids::ObjectId;
use mtg_engine::choose::{CardsChoice, YesNoChoice};
use mtg_engine::decision::{Answer, DecisionKind};
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_engine::{Event, PresetPermanent};
use mtg_testkit::cards::{forest, island, mountain, study};
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

use crate::common::casting::lands;
use crate::common::{name_of, names, pid};

/// Seat 0 holds a Study and Rummaging Goblin is on the table with an Island and `mountains` Mountains; the library
/// holds `top` and then Forests. Turn 1 begins in the main step.
fn table(seats: usize, top: &[&str], mountains: usize) -> Scenario {
    let mut battlefield: Vec<PresetPermanent> = vec![ready(island())];
    battlefield.extend(lands(mountains, mountain));
    let mut library: Vec<_> = top
        .iter()
        .map(|name| mtg_testkit::corpus::card(name).expect("the corpus has the card"))
        .collect();
    library.extend(vec![forest(); 8]);
    let mut scenario = Scenario::new(seats)
        .hand(0, vec![study("Study")])
        .library(0, library)
        .battlefield(0, battlefield);
    for seat in 1..seats {
        scenario = scenario.library(seat, vec![forest(); 8]);
    }
    scenario
}

/// Everyone passes: the top of the stack resolves.
fn resolve(driver: &mut Driver, seats: usize) -> Result<(), ScriptError> {
    for _ in 0..seats {
        driver.pass()?;
    }
    Ok(())
}

/// Seat 0 casts Study, which draws two cards, and the stack is left with what that triggered.
fn study_resolved(seats: usize, top: &[&str], mountains: usize) -> Result<Driver, ScriptError> {
    let mut driver = Driver::from_scenario(table(seats, top, mountains))?;
    driver.cast("Study")?;
    resolve(&mut driver, seats)?;
    Ok(driver)
}

fn stack(driver: &Driver) -> Vec<String> {
    names(driver.state(), Zone::Stack)
}

fn yes(driver: &mut Driver, yes: bool) -> Result<Vec<Event>, ScriptError> {
    driver.answer(Answer::YesNo(YesNoChoice(yes)))
}

/// The text of the question pending.
fn question(driver: &Driver) -> String {
    let DecisionKind::YesNo(question) = &driver.decision().unwrap().kind else {
        panic!(
            "a question is asked: {:?}",
            driver.decision().unwrap().kind.name()
        );
    };
    question.text.clone()
}

/// The way of the first spell cast in `events`.
fn cast_way(events: &[Event]) -> Option<Option<CastWay>> {
    events.iter().find_map(|event| match event {
        Event::SpellCast { choices, .. } => Some(choices.way),
        _ => None,
    })
}

#[rstest]
fn the_first_card_drawn_by_an_effect_is_cast_for_its_miracle_cost(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = study_resolved(seats, &["Thunderous Wrath"], 1)?;
    assert_eq!(stack(&driver), ["Thunderous Wrath"]);
    resolve(&mut driver, seats)?;
    assert_eq!(
        question(&driver),
        "Cast Thunderous Wrath for its miracle cost {R}?"
    );
    yes(&mut driver, true)?;
    let target = driver.target_player(1);
    driver.choose_targets(&[target])?;
    let events = driver.answer(Answer::PayMana(mtg_engine::decision::PayChoice::Auto))?;
    assert_eq!(cast_way(&events), Some(Some(CastWay::Miracle)));
    assert_eq!(stack(&driver), ["Thunderous Wrath"]);
    assert_eq!(names(driver.state(), Zone::Hand(pid(0))), ["Forest"]);
    resolve(&mut driver, seats)?;
    assert_eq!(driver.state().player(pid(1)).unwrap().life(), 15);
    assert_eq!(
        names(driver.state(), Zone::Graveyard(pid(0))),
        ["Study", "Thunderous Wrath"]
    );
    Ok(())
}

#[rstest]
fn a_card_drawn_second_has_no_miracle(#[values(2, 3, 4)] seats: usize) -> Result<(), ScriptError> {
    let driver = study_resolved(seats, &["Forest", "Thunderous Wrath"], 1)?;
    assert!(stack(&driver).is_empty());
    assert_eq!(
        names(driver.state(), Zone::Hand(pid(0))),
        ["Forest", "Thunderous Wrath"]
    );
    Ok(())
}

#[rstest]
fn a_miracle_that_is_declined_stays_in_the_hand(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = study_resolved(seats, &["Thunderous Wrath"], 1)?;
    resolve(&mut driver, seats)?;
    yes(&mut driver, false)?;
    assert!(stack(&driver).is_empty());
    assert_eq!(
        names(driver.state(), Zone::Hand(pid(0))),
        ["Thunderous Wrath", "Forest"]
    );
    assert_eq!(driver.state().player(pid(1)).unwrap().life(), 20);
    Ok(())
}

#[rstest]
fn a_miracle_cost_that_cannot_be_paid_is_not_asked(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = study_resolved(seats, &["Thunderous Wrath"], 0)?;
    assert_eq!(stack(&driver), ["Thunderous Wrath"]);
    resolve(&mut driver, seats)?;
    assert!(stack(&driver).is_empty());
    assert!(names(driver.state(), Zone::Hand(pid(0))).contains(&"Thunderous Wrath".to_owned()));
    Ok(())
}

#[rstest]
fn a_card_that_left_the_hand_in_response_is_not_cast(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let goblin = mtg_testkit::corpus::card("Rummaging Goblin").unwrap();
    let mut scenario = table(seats, &["Thunderous Wrath"], 1);
    scenario = scenario.battlefield(0, {
        let mut all: Vec<PresetPermanent> = vec![ready(island()), ready(mountain())];
        all.insert(0, ready(goblin));
        all
    });
    let mut driver = Driver::from_scenario(scenario)?;
    driver.cast("Study")?;
    resolve(&mut driver, seats)?;
    assert_eq!(stack(&driver), ["Thunderous Wrath"]);
    // The Goblin discards the card the trigger names, from the hand, in response.
    driver.begin_activation("Rummaging Goblin")?;
    let wrath = in_hand(&driver, "Thunderous Wrath");
    driver.answer(Answer::ChooseCards(CardsChoice(vec![wrath])))?;
    resolve(&mut driver, seats)?;
    resolve(&mut driver, seats)?;
    assert!(stack(&driver).is_empty(), "no cast was offered");
    assert!(
        names(driver.state(), Zone::Graveyard(pid(0))).contains(&"Thunderous Wrath".to_owned())
    );
    assert_eq!(driver.state().player(pid(1)).unwrap().life(), 20);
    Ok(())
}

#[rstest]
fn the_card_of_the_draw_step_has_miracle_and_the_card_after_it_has_not(
    #[values(2, 3, 4)] seats: usize,
    #[values(0, 1)] second_draw: usize,
) -> Result<(), ScriptError> {
    let miracle = mtg_testkit::corpus::card("Thunderous Wrath").unwrap();
    let mut library = vec![forest(); 6];
    library.insert(second_draw, miracle);
    let mut scenario = Scenario::new(seats)
        .at(2, 0, Step::Upkeep)
        .hand(0, vec![study("Study")])
        .library(0, library)
        .battlefield(0, vec![ready(island()), ready(mountain())]);
    for seat in 1..seats {
        scenario = scenario.library(seat, vec![forest(); 6]);
    }
    let mut driver = Driver::from_scenario(scenario)?;
    driver.pass_until(Step::Draw)?;
    // The draw step draws the first card; the miracle card is on top when `second_draw` is 0.
    assert_eq!(stack(&driver).len(), usize::from(second_draw == 0));
    if second_draw == 0 {
        resolve(&mut driver, seats)?;
        yes(&mut driver, false)?;
    }
    driver.pass_until(Step::Main1)?;
    driver.cast("Study")?;
    resolve(&mut driver, seats)?;
    // The card Study draws is the second of the turn at the earliest: never a miracle.
    assert!(stack(&driver).is_empty());
    Ok(())
}

#[rstest]
fn an_opponents_draw_gives_the_trigger_and_the_question_to_that_opponent(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut library = vec![mtg_testkit::corpus::card("Thunderous Wrath").unwrap()];
    library.extend(vec![forest(); 6]);
    let mut scenario = Scenario::new(seats)
        .at(2, 1, Step::Upkeep)
        .library(1, library)
        .battlefield(1, vec![ready(mountain())]);
    for seat in [0].into_iter().chain(2..seats) {
        scenario = scenario.library(seat, vec![forest(); 6]);
    }
    let mut driver = Driver::from_scenario(scenario)?;
    driver.pass_until(Step::Draw)?;
    assert_eq!(stack(&driver), ["Thunderous Wrath"]);
    let ability = driver.state().zone(Zone::Stack)[0];
    assert_eq!(driver.state().object(ability).unwrap().controller(), pid(1));
    resolve(&mut driver, seats)?;
    assert_eq!(driver.decision()?.player, pid(1));
    Ok(())
}

#[rstest]
fn a_creature_with_miracle_enters_the_battlefield_for_its_miracle_cost(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut scenario = table(seats, &["Sister Repentia"], 0);
    scenario = scenario.battlefield(0, {
        let mut all: Vec<PresetPermanent> = lands(1, island);
        all.extend(lands(1, mtg_testkit::cards::plains));
        all.extend(lands(1, mtg_testkit::cards::swamp));
        all
    });
    let mut driver = Driver::from_scenario(scenario)?;
    driver.cast("Study")?;
    resolve(&mut driver, seats)?;
    resolve(&mut driver, seats)?;
    assert_eq!(
        question(&driver),
        "Cast Sister Repentia for its miracle cost {W}{B}?"
    );
    yes(&mut driver, true)?;
    driver.answer(Answer::PayMana(mtg_engine::decision::PayChoice::Auto))?;
    resolve(&mut driver, seats)?;
    assert_eq!(
        names(driver.state(), Zone::Battlefield)
            .last()
            .map(String::as_str),
        Some("Sister Repentia")
    );
    Ok(())
}

/// The card called `name` in seat 0's hand.
fn in_hand(driver: &Driver, name: &str) -> ObjectId {
    driver
        .state()
        .zone(Zone::Hand(pid(0)))
        .iter()
        .copied()
        .find(|id| name_of(driver.state(), *id) == name)
        .expect("the card is in hand")
}
