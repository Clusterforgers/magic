//! Goal: going back passes over turn-based steps that reveal and decide nothing, and each irreversible kind of event
//! or answer since the pass names its own refusal.
use mtg_core::ids::{ObjectId, PlayerId};
use rstest::rstest;

use super::{GoBackRefusal, PassPoint, refusal_in};
use crate::combat::AttackTarget;
use crate::events::Event;
use crate::take_back::TakeBackKind;
use crate::turn::Step;

const SEAT: PlayerId = PlayerId(0);
const OTHER: PlayerId = PlayerId(1);

#[rstest]
#[case::pass(Event::PriorityPassed { player: OTHER }, None)]
#[case::step(Event::StepBegan { step: Step::BeginCombat }, None)]
#[case::untap(Event::PermanentUntapped { object: ObjectId(7) }, None)]
#[case::damage_wears_off(Event::DamageCleared, None)]
#[case::went_back(Event::WentBack { player: OTHER, step: Step::Main1, active: OTHER }, None)]
#[case::draw(Event::CardDrawn { player: SEAT, object: ObjectId(7) }, Some(GoBackRefusal::CardDrawn))]
#[case::shuffle(Event::LibraryShuffled { player: OTHER, order: Vec::new() }, Some(GoBackRefusal::LibraryShuffled))]
#[case::life(Event::LifeGained { player: SEAT, amount: 1 }, Some(GoBackRefusal::Changed))]
#[case::attack(Event::AttackersDeclared { player: SEAT, attackers: vec![(ObjectId(7), AttackTarget::Player(OTHER))] }, Some(GoBackRefusal::Declared))]
#[case::nothing_to_attack_with(Event::AttackersDeclared { player: SEAT, attackers: Vec::new() }, None)]
#[case::take_back(Event::TookBack { player: OTHER, kind: TakeBackKind::Land, name: "Forest".into() }, Some(GoBackRefusal::Acted))]
#[case::tapped(Event::PermanentTapped { object: ObjectId(7) }, Some(GoBackRefusal::Changed))]
fn each_event_since_the_pass_allows_or_names_its_refusal(
    #[case] event: Event,
    #[case] expected: Option<GoBackRefusal>,
) {
    assert_eq!(refusal_in(&[event], &[], SEAT), expected);
}

fn answered(player: PlayerId, pass: bool) -> PassPoint {
    PassPoint {
        player,
        pass,
        from: 3,
        rng: 0,
        turn: 1,
        step: Step::Main1,
        active: SEAT,
    }
}

#[rstest]
#[case::passes_only(vec![answered(OTHER, true), answered(SEAT, true)], None)]
#[case::own_answer(vec![answered(SEAT, false)], Some(GoBackRefusal::Chose))]
#[case::other_answer(vec![answered(OTHER, true), answered(OTHER, false)], Some(GoBackRefusal::OtherPlayerActed))]
fn an_answer_other_than_a_pass_since_stops_going_back(
    #[case] after: Vec<PassPoint>,
    #[case] expected: Option<GoBackRefusal>,
) {
    assert_eq!(refusal_in(&[], &after, SEAT), expected);
}
