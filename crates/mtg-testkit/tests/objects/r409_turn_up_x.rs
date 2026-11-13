//! Goal: a morph cost with `{X}` plays whole at two to four seats: turning the permanent face up asks X first, costs X
//! mana for each `{X}` on top of the rest, and its "when turned face up" trigger reads that X (Warbreak Trumpeter makes X
//! Goblins, Bane of the Living gives every creature -X/-X); a cost the lands cannot pay for that X is refused.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_core::ids::ObjectId;
use mtg_engine::PresetPermanent;
use mtg_engine::choose::NumberChoice;
use mtg_engine::decision::{Answer, DecisionKind, PriorityAction};
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{free_creature, mountain, swamp};
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

use crate::common::casting::{choose_way, pay};
use crate::common::names;

/// `count` lands of one kind for seat 0, untapped.
fn r409_lands(
    count: usize,
    land: fn() -> std::sync::Arc<mtg_cards::CardDefinition>,
) -> Vec<PresetPermanent> {
    (0..count).map(|_| ready(land())).collect()
}

/// `seats` players; seat 0 holds `card` and controls `lands`, seat 1 a 2/2 and a 5/5.
fn r409_table(seats: usize, card: &str, lands: Vec<PresetPermanent>) -> Scenario {
    Scenario::new(seats)
        .hand_named(0, [card])
        .battlefield(0, lands)
        .battlefield(
            1,
            [
                ready(free_creature("Bear", 2, 2)),
                ready(free_creature("Giant", 5, 5)),
            ],
        )
}

/// Priority passes until the stack is empty.
fn r409_settle(driver: &mut Driver) -> Result<(), ScriptError> {
    for _ in 0..40 {
        if !matches!(driver.decision()?.kind, DecisionKind::Priority(_))
            || driver.state().stack().is_empty()
        {
            break;
        }
        driver.pass()?;
    }
    Ok(())
}

/// Seat 0 casts `name` face down for {3} and lets it resolve; the face-down permanent.
fn r409_face_down(driver: &mut Driver, name: &str) -> Result<ObjectId, ScriptError> {
    driver.begin_cast(name)?;
    if matches!(driver.decision()?.kind, DecisionKind::ChooseOption(_)) {
        choose_way(driver, "Cast face down")?;
    }
    pay(driver)?;
    r409_settle(driver)?;
    Ok(driver
        .state()
        .zone(Zone::Battlefield)
        .iter()
        .copied()
        .find(|id| driver.state().object(*id).unwrap().face_down().is_some())
        .expect("a face-down permanent"))
}

/// Turns `id` face up for X = `x`, paying with the lands: the X asked, then the payment of `per_x` mana per X on top of
/// `rest`.
fn r409_turn_up(
    driver: &mut Driver,
    id: ObjectId,
    x: u32,
    per_x: u32,
    rest: u32,
) -> Result<(), ScriptError> {
    driver.answer(Answer::Priority(PriorityAction::TurnFaceUp(id)))?;
    let DecisionKind::ChooseNumber(options) = &driver.decision()?.kind else {
        panic!("turning up asks X first");
    };
    assert_eq!((options.min, options.per_x), (0, per_x));
    driver.answer(Answer::ChooseNumber(NumberChoice::Number(x)))?;
    assert_eq!(driver.pay_options()?.cost.mana_value(), rest + per_x * x);
    pay(driver)?;
    r409_settle(driver)
}

fn r409_count(driver: &Driver, name: &str) -> usize {
    names(driver.state(), Zone::Battlefield)
        .iter()
        .filter(|found| *found == name)
        .count()
}

#[rstest]
#[case::two_goblins(2, 2)]
#[case::no_goblin(3, 0)]
#[case::three_goblins_four_seats(4, 3)]
fn r409_warbreak_trumpeter_makes_x_goblins(
    #[case] seats: usize,
    #[case] x: u32,
) -> Result<(), ScriptError> {
    let lands = r409_lands(3 + 2 * x as usize + 1, mountain);
    let mut driver = Driver::from_scenario(r409_table(seats, "Warbreak Trumpeter", lands))?;
    let id = r409_face_down(&mut driver, "Warbreak Trumpeter")?;
    r409_turn_up(&mut driver, id, x, 2, 1)?;
    assert!(driver.state().object(id).unwrap().face_down().is_none());
    assert_eq!(r409_count(&driver, "Goblin"), x as usize);
    Ok(())
}

#[rstest]
#[case::the_bear_dies(2, 2, &["Bane of the Living", "Giant"])]
#[case::nothing_dies(3, 0, &["Bane of the Living", "Bear", "Giant"])]
#[case::bane_dies_too(4, 3, &["Giant"])]
fn r409_bane_of_the_living_shrinks_every_creature(
    #[case] seats: usize,
    #[case] x: u32,
    #[case] left: &[&str],
) -> Result<(), ScriptError> {
    let lands = r409_lands(3 + 2 + x as usize, swamp);
    let mut driver = Driver::from_scenario(r409_table(seats, "Bane of the Living", lands))?;
    let id = r409_face_down(&mut driver, "Bane of the Living")?;
    r409_turn_up(&mut driver, id, x, 1, 2)?;
    for name in ["Bane of the Living", "Bear", "Giant"] {
        assert_eq!(
            r409_count(&driver, name),
            usize::from(left.contains(&name)),
            "{name}"
        );
    }
    Ok(())
}

/// X more than the lands pay for is refused at the payment.
#[test]
fn r409_x_the_lands_cannot_pay_is_refused() -> Result<(), ScriptError> {
    let lands = r409_lands(3 + 2, swamp);
    let mut driver = Driver::from_scenario(r409_table(2, "Bane of the Living", lands))?;
    let id = r409_face_down(&mut driver, "Bane of the Living")?;
    driver.answer(Answer::Priority(PriorityAction::TurnFaceUp(id)))?;
    driver.answer(Answer::ChooseNumber(NumberChoice::Number(1)))?;
    assert!(driver.state().object(id).unwrap().face_down().is_some());
    Ok(())
}
