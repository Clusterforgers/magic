//! Goal: pack tactics ("if you attacked with creatures with total power N or greater this combat") reads into the
//! total power its controller attacked with this combat, at every number and after any trigger, is written back as the
//! same text, and a near miss (this turn, a total below, an opponent) is left to the table.
use super::*;
use crate::refs::Ref;
use crate::value::{Amount, Cmp, Condition, TurnTally};

#[rstest]
#[case::draws(
    "Whenever ~ attacks, if you attacked with creatures with total power 6 or greater this combat, draw a card.",
    6
)]
#[case::counter(
    "Whenever ~ attacks, if you attacked with creatures with total power 8 or greater this combat, put a +1/+1 counter on ~.",
    8
)]
fn r115_pack_tactics_reads_the_power_attacked_with(#[case] text: &str, #[case] power: i32) {
    let Ability::Triggered { intervening_if, .. } = golden(text, ClauseKind::Triggered, false)
    else {
        panic!("{text}: no trigger");
    };
    assert_eq!(
        intervening_if,
        Some(Condition::Compare {
            left: Amount::Tallied {
                who: Ref::You,
                what: TurnTally::R115AttackPower,
            },
            op: Cmp::Ge,
            right: Amount::Fixed(power),
        })
    );
}

#[rstest]
#[case::this_turn(
    "Whenever ~ attacks, if you attacked with creatures with total power 6 or greater this turn, draw a card."
)]
#[case::or_less(
    "Whenever ~ attacks, if you attacked with creatures with total power 6 or less this combat, draw a card."
)]
#[case::opponent(
    "Whenever ~ attacks, if an opponent attacked with creatures with total power 6 or greater this combat, draw a card."
)]
fn r115_a_near_miss_of_pack_tactics_is_not_read(#[case] text: &str) {
    assert_eq!(clause(text, ClauseKind::Triggered, false), None, "{text}");
}
