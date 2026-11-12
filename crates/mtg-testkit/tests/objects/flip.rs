//! Goal: a flip card flips on the battlefield as the same object, at two to four seats: Jushi Apprentice when its draw leaves nine cards in hand, Orochi Eggwatcher when its Snake makes ten creatures, and the ki-counter cards at the end step with two ki counters and the player's yes (a Spirit spell cast puts the counters on); the flipped half has its own name, types, power, toughness and abilities with the top half's cost and colour, keeps its counters, tapped state and summoning sickness, stays flipped, and is the top half again after it leaves the battlefield; a card with one face or a card in a hand cannot be flipped; and a log with a flip replays to the same state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_cards::keyword::Keyword;
use mtg_cards::value::CounterKind;
use mtg_core::card::Supertype;
use mtg_core::ids::ObjectId;
use mtg_engine::characteristics::characteristics;
use mtg_engine::choose::YesNoChoice;
use mtg_engine::decision::{Answer, DecisionKind};
use mtg_engine::keywords::has_keyword;
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_engine::{Event, apply_event};
use mtg_testkit::cards::{forest, island};
use mtg_testkit::corpus::card;
use mtg_testkit::flip_cards::spirit;
use mtg_testkit::{Driver, Scenario, ScriptError, ready, sick, with_counters};
use rstest::rstest;

use crate::common::{name_of, names, pid};

const JUSHI: &str = "Jushi Apprentice // Tomoya the Revealer";
const EGGWATCHER: &str = "Orochi Eggwatcher // Shidako, Broodmistress";
const PUPIL: &str = "Budoka Pupil // Ichiga, Who Topples Oaks";
const MUSCLE: &str = "Hired Muscle // Scarmaker";

fn ki() -> CounterKind {
    CounterKind::Other("ki".into())
}

fn stats(driver: &Driver, id: ObjectId) -> (i32, i32) {
    let found = characteristics(driver.state(), id).unwrap();
    (found.power.unwrap(), found.toughness.unwrap())
}

/// Every seat passes once: the top of the stack resolves.
fn resolve_top(driver: &mut Driver, seats: usize) -> Result<(), ScriptError> {
    for _ in 0..seats {
        driver.pass()?;
    }
    Ok(())
}

fn others(seats: usize) -> Scenario {
    let mut scenario = Scenario::new(seats);
    for seat in 1..seats {
        scenario = scenario.library(seat, vec![forest(); 12]);
    }
    scenario
}

#[rstest]
#[case::eight_in_hand_draw_the_ninth(8, true)]
#[case::nine_in_hand(9, true)]
#[case::seven_in_hand_stay_short(7, false)]
fn jushi_apprentice_flips_when_the_draw_leaves_nine_cards_in_hand(
    #[values(2, 3, 4)] seats: usize,
    #[case] in_hand: usize,
    #[case] flips: bool,
) -> Result<(), ScriptError> {
    let scenario = others(seats)
        .at(1, 0, Step::Main1)
        .library(0, vec![forest(); 12])
        .hand(0, vec![forest(); in_hand])
        .battlefield(
            0,
            std::iter::once(ready(card(JUSHI).unwrap())).chain((0..3).map(|_| ready(island()))),
        );
    let mut driver = Driver::from_scenario(scenario)?;
    let apprentice = driver.id_of("Jushi Apprentice")?;
    driver.begin_activation("Jushi Apprentice")?;
    driver.pay_tapping(&["Island", "Island", "Island"], 0)?;
    resolve_top(&mut driver, seats)?;
    assert_eq!(driver.state().zone(Zone::Hand(pid(0))).len(), in_hand + 1);
    let object = driver.state().object(apprentice).unwrap();
    if flips {
        assert_eq!(object.card().name, "Tomoya the Revealer");
        assert_eq!(object.face(), Some(1));
        assert_eq!(stats(&driver, apprentice), (2, 3));
        assert!(object.card().supertypes.contains(&Supertype::Legendary));
    } else {
        assert_eq!(object.card().name, "Jushi Apprentice");
        assert_eq!(object.face(), None);
        assert_eq!(stats(&driver, apprentice), (1, 2));
    }
    Ok(())
}

#[rstest]
#[case::eight_others_and_the_snake(8, true)]
#[case::seven_others_stay_short(7, false)]
fn orochi_eggwatcher_flips_when_its_snake_makes_ten_creatures(
    #[values(2, 3, 4)] seats: usize,
    #[case] bears: usize,
    #[case] flips: bool,
) -> Result<(), ScriptError> {
    let scenario = others(seats).at(1, 0, Step::Main1).battlefield(
        0,
        std::iter::repeat_n("Grizzly Bears", bears)
            .chain([EGGWATCHER])
            .map(|name| ready(card(name).unwrap()))
            .chain((0..3).map(|_| ready(forest()))),
    );
    let mut driver = Driver::from_scenario(scenario)?;
    let eggwatcher = driver.id_of("Orochi Eggwatcher")?;
    driver.begin_activation("Orochi Eggwatcher")?;
    driver.pay_tapping(&["Forest", "Forest", "Forest"], 0)?;
    resolve_top(&mut driver, seats)?;
    let object = driver.state().object(eggwatcher).unwrap();
    let snakes = names(driver.state(), Zone::Battlefield)
        .iter()
        .filter(|name| name.as_str() == "Snake")
        .count();
    assert_eq!(snakes, 1);
    assert_eq!(
        object.card().name,
        if flips {
            "Shidako, Broodmistress"
        } else {
            "Orochi Eggwatcher"
        }
    );
    assert_eq!(
        stats(&driver, eggwatcher),
        if flips { (3, 3) } else { (1, 1) }
    );
    Ok(())
}

/// The end step of turn 1 with the ki card holding `counters`: the trigger is on the stack, if there is one.
fn at_end_step(seats: usize, name: &str, counters: u32) -> Result<Driver, ScriptError> {
    let pupil = with_counters(ready(card(name).unwrap()), ki(), counters);
    let mut driver =
        Driver::from_scenario(others(seats).at(1, 0, Step::Main1).battlefield(0, [pupil]))?;
    driver.idle_until(1, Step::End)?;
    Ok(driver)
}

#[rstest]
#[case::two_counters_and_yes(2, true, true)]
#[case::three_counters_and_yes(3, true, true)]
#[case::two_counters_and_no(2, false, false)]
fn a_ki_card_flips_at_the_end_step_with_two_counters_if_its_controller_says_yes(
    #[values(2, 3, 4)] seats: usize,
    #[values(PUPIL, MUSCLE)] name: &str,
    #[case] counters: u32,
    #[case] answer: bool,
    #[case] flips: bool,
) -> Result<(), ScriptError> {
    let mut driver = at_end_step(seats, name, counters)?;
    let id = driver.id_of(name.split(" // ").next().unwrap())?;
    resolve_top(&mut driver, seats)?;
    assert!(matches!(driver.decision()?.kind, DecisionKind::YesNo(_)));
    driver.answer(Answer::YesNo(YesNoChoice(answer)))?;
    let object = driver.state().object(id).unwrap();
    assert_eq!(object.face(), flips.then_some(1));
    Ok(())
}

#[rstest]
fn a_ki_card_with_one_counter_does_not_trigger(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = at_end_step(seats, PUPIL, 1)?;
    let id = driver.id_of("Budoka Pupil")?;
    assert_eq!(driver.state().stack(), []);
    resolve_top(&mut driver, seats)?;
    assert_eq!(driver.state().object(id).unwrap().face(), None);
    Ok(())
}

#[rstest]
fn a_spirit_spell_puts_a_ki_counter_on_a_ki_card(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let scenario = others(seats)
        .at(1, 0, Step::Main1)
        .hand(0, vec![spirit()])
        .battlefield(0, [ready(card(PUPIL).unwrap()), ready(island())]);
    let mut driver = Driver::from_scenario(scenario)?;
    let pupil = driver.id_of("Budoka Pupil")?;
    driver.cast_tapping("Wisp", &["Island"])?;
    // The trigger is above the spell: it resolves first, and its controller says yes.
    resolve_top(&mut driver, seats)?;
    driver.answer(Answer::YesNo(YesNoChoice(true)))?;
    let held: u32 = driver
        .state()
        .object(pupil)
        .unwrap()
        .permanent()
        .unwrap()
        .counters()
        .filter(|(kind, _)| **kind == ki())
        .map(|(_, held)| held)
        .sum();
    assert_eq!(held, 1);
    Ok(())
}

#[test]
fn the_flipped_half_is_the_same_object_with_the_top_halfs_cost_and_colour()
-> Result<(), ScriptError> {
    let mut apprentice = with_counters(sick(card(JUSHI).unwrap()), ki(), 3);
    apprentice.tapped = true;
    let driver = Driver::from_scenario(Scenario::new(2).battlefield(0, [apprentice]))?;
    let id = driver.id_of("Jushi Apprentice")?;
    let before = driver
        .state()
        .object(id)
        .unwrap()
        .permanent()
        .unwrap()
        .clone();
    let mut flipped = driver.state().clone();
    apply_event(&mut flipped, &Event::Flipped { object: id }).unwrap();
    let now = flipped.object(id).unwrap();
    assert_eq!(now.card().name, "Tomoya the Revealer");
    let kept = now.permanent().unwrap();
    assert_eq!(
        (
            kept.tapped(),
            kept.summoning_sick(),
            kept.counters().count()
        ),
        (
            before.tapped(),
            before.summoning_sick(),
            before.counters().count()
        )
    );
    assert!(kept.tapped() && kept.summoning_sick());
    // The bottom half prints no cost: the flipped permanent has the top half's mana value and blue colour.
    assert_eq!(now.mana_value(), 2);
    assert_eq!(
        characteristics(&flipped, id).unwrap().colors,
        characteristics(driver.state(), id).unwrap().colors
    );
    Ok(())
}

#[test]
fn a_flipped_permanent_that_leaves_the_battlefield_is_its_top_half_again() -> Result<(), ScriptError>
{
    let driver = Driver::from_scenario(Scenario::new(2).battlefield_named(0, [JUSHI]))?;
    let id = driver.id_of("Jushi Apprentice")?;
    let mut state = driver.state().clone();
    apply_event(&mut state, &Event::Flipped { object: id }).unwrap();
    assert_eq!(name_of(&state, id), "Tomoya the Revealer");
    apply_event(
        &mut state,
        &Event::CreatureDied {
            object: id,
            cause: mtg_engine::events::DeathCause::LethalDamage,
        },
    )
    .unwrap();
    assert_eq!(names(&state, Zone::Graveyard(pid(0))), ["Jushi Apprentice"]);
    assert_eq!(state.object(id).unwrap().face(), None);
    Ok(())
}

#[test]
fn only_a_flip_card_on_the_battlefield_is_flipped_by_the_fold() {
    let driver = Driver::from_scenario(Scenario::new(2).hand_named(0, [JUSHI]).battlefield_named(
        0,
        ["Grizzly Bears", "Delver of Secrets // Insectile Aberration"],
    ))
    .unwrap();
    let in_hand = driver.state().zone(Zone::Hand(pid(0)))[0];
    let bears = driver.id_of("Grizzly Bears").unwrap();
    let delver = driver.id_of("Delver of Secrets").unwrap();
    for id in [in_hand, bears, delver] {
        let mut state = driver.state().clone();
        assert!(apply_event(&mut state, &Event::Flipped { object: id }).is_err());
        assert_eq!(state.object(id).unwrap().face(), None);
    }
}

#[test]
fn a_flipped_permanent_has_the_abilities_of_its_bottom_half() -> Result<(), ScriptError> {
    let driver = Driver::from_scenario(Scenario::new(2).battlefield_named(0, [PUPIL]))?;
    let id = driver.id_of("Budoka Pupil")?;
    assert!(!has_keyword(driver.state(), id, &Keyword::Trample));
    let mut state = driver.state().clone();
    apply_event(&mut state, &Event::Flipped { object: id }).unwrap();
    assert!(has_keyword(&state, id, &Keyword::Trample));
    assert_eq!(characteristics(&state, id).unwrap().toughness, Some(3));
    Ok(())
}

#[rstest]
fn a_game_with_a_flip_replays_to_the_same_state(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = at_end_step(seats, PUPIL, 2)?;
    resolve_top(&mut driver, seats)?;
    driver.answer(Answer::YesNo(YesNoChoice(true)))?;
    let setup = driver.engine().setup().clone();
    let replayed = mtg_engine::replay(&setup, driver.engine().events()).unwrap();
    assert_eq!(replayed.state_hash(), driver.engine().state_hash());
    assert!(
        driver
            .engine()
            .events()
            .iter()
            .any(|event| matches!(event, Event::Flipped { .. }))
    );
    Ok(())
}
