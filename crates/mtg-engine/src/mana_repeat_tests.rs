//! Goal: a mana ability that neither taps nor sacrifices its source and costs only the cards it exiles (Cadaverous Bloom) is used again within one payment while the hand has a card for each use, a source that taps is still used once, and affordability, automatic payment, checking and completion agree on how many uses a payment has.

use std::collections::BTreeMap;

use mtg_core::ids::ObjectId;
use mtg_core::mana::{ManaCost, ManaType};
use rstest::rstest;

use crate::error::PaymentError;
use crate::mana::{
    ManaOption, ManaPool, ManaSource, Payment, Take, auto_payment, can_afford, check_payment,
    complete_payment,
};
use crate::spend::Spend;

const LIFE: i32 = 20;

/// The first object of the lands, past the Blooms.
const LAND: u64 = 20;

fn cost(text: &str) -> ManaCost {
    ManaCost::parse(text).unwrap()
}

fn mana(letter: char) -> ManaType {
    match letter {
        'B' | 's' => ManaType::Black,
        'G' | 'f' => ManaType::Green,
        'R' | 'm' => ManaType::Red,
        'W' | 'p' => ManaType::White,
        'U' | 'i' => ManaType::Blue,
        other => panic!("not a mana letter: {other}"),
    }
}

fn source(object: u64, ability: usize) -> ManaSource {
    ManaSource {
        object: ObjectId(object),
        ability,
    }
}

/// The sources of a table: each Bloom is a permanent whose abilities add two mana of the letters of its entry and exile
/// a card of `hand`; `lands` has one permanent for each letter, which taps for one mana.
fn table(blooms: &[(u64, &str)], hand: &[u64], lands: &str) -> Vec<ManaOption> {
    let mut sources = Vec::new();
    for (object, colours) in blooms {
        for (ability, letter) in colours.chars().enumerate() {
            sources.push(ManaOption {
                count: 2,
                extra: true,
                tapless: true,
                repeat: true,
                takes: vec![Take {
                    sacrifice: false,
                    exile: true,
                    count: 1,
                    from: hand.iter().copied().map(ObjectId).collect(),
                }],
                ..ManaOption::new(source(*object, ability), mana(letter))
            });
        }
    }
    for (at, letter) in (LAND..).zip(lands.chars()) {
        sources.push(ManaOption::new(source(at, 0), mana(letter)));
    }
    sources
}

fn pay(taps: &[(u64, usize)]) -> Payment {
    Payment {
        taps: taps
            .iter()
            .map(|(object, ability)| source(*object, *ability))
            .collect(),
        ..Payment::default()
    }
}

/// How many times each permanent of `payment` is used.
fn uses(payment: &Payment) -> Vec<(u64, usize)> {
    let mut counts: BTreeMap<u64, usize> = BTreeMap::new();
    for tap in &payment.taps {
        *counts.entry(tap.object.0).or_default() += 1;
    }
    counts.into_iter().collect()
}

#[rstest]
#[case::one_use("{B}{B}", &[(1, "BG")], &[10], "", Some(&[(1, 1)][..]))]
#[case::one_list_twice("{B}{B}{B}{B}", &[(1, "BG")], &[10, 11], "", Some(&[(1, 2)][..]))]
#[case::both_lists("{B}{B}{G}{G}", &[(1, "BG")], &[10, 11], "", Some(&[(1, 2)][..]))]
#[case::a_card_for_each_list("{B}{B}{G}{G}", &[(1, "BG")], &[10], "", None)]
#[case::no_card("{B}{B}", &[(1, "BG")], &[], "", None)]
#[case::three_uses("{B}{B}{B}{B}{B}{B}", &[(1, "B")], &[10, 11, 12], "", Some(&[(1, 3)][..]))]
#[case::a_card_too_many("{B}{B}{B}{B}", &[(1, "B")], &[10, 11, 12], "", Some(&[(1, 2)][..]))]
#[case::the_third_use_has_no_card("{B}{B}{B}{B}{B}{B}", &[(1, "B")], &[10, 11], "", None)]
#[case::two_enchantments_share_the_hand("{B}{B}{G}{G}", &[(1, "B"), (2, "G")], &[10, 11], "", Some(&[(1, 1), (2, 1)][..]))]
#[case::two_enchantments_one_card("{B}{B}{G}{G}", &[(1, "B"), (2, "G")], &[10], "", None)]
#[case::three_uses_of_two("{B}{B}{B}{B}{G}{G}", &[(1, "B"), (2, "G")], &[10, 11, 12], "", Some(&[(1, 2), (2, 1)][..]))]
#[case::three_uses_of_two_with_two_cards("{B}{B}{B}{B}{G}{G}", &[(1, "B"), (2, "G")], &[10, 11], "", None)]
#[case::a_land_beside_two_uses("{B}{B}{B}{G}{G}", &[(1, "BG")], &[10, 11], "sf", Some(&[(1, 2), (LAND, 1)][..]))]
#[case::lands_before_cards("{B}{G}", &[(1, "BG")], &[10, 11], "sf", Some(&[(LAND, 1), (LAND + 1, 1)][..]))]
#[case::a_land_is_one_mana("{B}{B}{B}", &[(1, "B")], &[10], "s", Some(&[(1, 1), (LAND, 1)][..]))]
#[case::a_land_is_not_used_twice("{B}{B}{B}", &[], &[10], "s", None)]
#[case::a_colour_it_cannot_make("{R}{R}", &[(1, "BG")], &[10], "", None)]
fn automatic_payment_uses_a_source_as_often_as_it_needs(
    #[case] cost_text: &str,
    #[case] blooms: &[(u64, &str)],
    #[case] hand: &[u64],
    #[case] lands: &str,
    #[case] expected: Option<&[(u64, usize)]>,
) {
    let (cost, pool, sources) = (
        cost(cost_text),
        ManaPool::default(),
        table(blooms, hand, lands),
    );
    let found = auto_payment(&cost, &pool, &sources, LIFE, &Spend::other());
    assert_eq!(
        can_afford(&cost, &pool, &sources, LIFE, &Spend::other()),
        expected.is_some()
    );
    match (found, expected) {
        (None, None) => {}
        (Some(payment), Some(expected)) => {
            assert_eq!(uses(&payment), expected);
            assert_eq!(
                check_payment(&cost, &pool, &sources, &[], LIFE, &Spend::other(), &payment)
                    .map(|_| ()),
                Ok(()),
                "the payment it makes is valid"
            );
        }
        (found, expected) => panic!("{found:?} against {expected:?}"),
    }
}

#[rstest]
#[case::two_lists("{B}{B}{G}{G}", &[10, 11], &[(1, 0), (1, 1)], Ok(()))]
#[case::one_list_twice("{B}{B}{B}{B}", &[10, 11], &[(1, 0), (1, 0)], Ok(()))]
#[case::the_other_order("{B}{B}{G}{G}", &[10, 11], &[(1, 1), (1, 0)], Ok(()))]
#[case::beside_a_land("{B}{B}{B}", &[10], &[(1, 0), (LAND, 0)], Ok(()))]
#[case::one_use("{B}{B}", &[10], &[(1, 0)], Ok(()))]
#[case::one_card_two_lists("{B}{B}{G}{G}", &[10], &[(1, 0), (1, 1)], Err(PaymentError::ObjectTappedTwice(ObjectId(1))))]
#[case::one_card_one_list_twice("{B}{B}{B}{B}", &[10], &[(1, 0), (1, 0)], Err(PaymentError::DuplicateSource(source(1, 0))))]
#[case::a_third_use("{B}{B}{B}{B}{B}{B}", &[10, 11], &[(1, 0), (1, 0), (1, 0)], Err(PaymentError::DuplicateSource(source(1, 0))))]
#[case::more_than_the_cost_takes("{B}{B}", &[10, 11], &[(1, 0), (1, 0)], Err(PaymentError::WrongTotal { needed: 2, got: 4 }))]
#[case::a_land_twice("{B}{B}", &[10], &[(LAND, 0), (LAND, 0)], Err(PaymentError::DuplicateSource(source(LAND, 0))))]
#[case::a_land_and_a_use_of_the_land("{B}{B}{B}", &[10], &[(1, 0), (LAND, 0), (LAND, 0)], Err(PaymentError::DuplicateSource(source(LAND, 0))))]
#[case::a_copy_nobody_offered("{B}{B}", &[10], &[(1 + (1 << 40), 0)], Err(PaymentError::UnknownSource(source(1 + (1 << 40), 0))))]
fn a_checked_payment_names_a_source_once_for_each_use(
    #[case] cost_text: &str,
    #[case] hand: &[u64],
    #[case] taps: &[(u64, usize)],
    #[case] verdict: Result<(), PaymentError>,
) {
    let (cost, pool) = (cost(cost_text), ManaPool::default());
    let sources = table(&[(1, "BG")], hand, "s");
    let checked = check_payment(
        &cost,
        &pool,
        &sources,
        &[],
        LIFE,
        &Spend::other(),
        &pay(taps),
    );
    match verdict {
        Ok(()) => {
            let spent = checked.unwrap();
            let steps: Vec<ManaSource> = spent.steps.iter().map(|step| step.source).collect();
            assert_eq!(steps.len(), taps.len(), "one step for each use");
            assert!(
                steps.iter().all(|tap| tap.object.0 < 1 << 40),
                "the steps name the permanent"
            );
        }
        Err(error) => assert_eq!(checked.map(|_| ()), Err(error)),
    }
}

#[rstest]
#[case::nothing_chosen("{B}{B}{G}{G}", &[10, 11], &[], Ok(&[(1, 2)][..]))]
#[case::the_first_list("{B}{B}{G}{G}", &[10, 11], &[(1, 0)], Ok(&[(1, 2)][..]))]
#[case::the_second_list("{B}{B}{G}{G}", &[10, 11], &[(1, 1)], Ok(&[(1, 2)][..]))]
#[case::the_same_list_again("{B}{B}{B}{B}", &[10, 11], &[(1, 0)], Ok(&[(1, 2)][..]))]
#[case::already_whole("{B}{B}{B}{B}", &[10, 11], &[(1, 0), (1, 0)], Ok(&[(1, 2)][..]))]
#[case::a_land_chosen("{B}{B}{B}", &[10], &[(LAND, 0)], Ok(&[(1, 1), (LAND, 1)][..]))]
#[case::no_card_for_the_rest("{B}{B}{G}{G}", &[10], &[(1, 0)], Err(PaymentError::CannotComplete))]
#[case::too_much_chosen("{B}{B}", &[10, 11], &[(1, 0), (1, 0)], Err(PaymentError::Wasted))]
fn a_partial_payment_is_finished_with_more_uses(
    #[case] cost_text: &str,
    #[case] hand: &[u64],
    #[case] partial: &[(u64, usize)],
    #[case] expected: Result<&[(u64, usize)], PaymentError>,
) {
    let (cost, pool) = (cost(cost_text), ManaPool::default());
    let sources = table(&[(1, "BG")], hand, "s");
    let done = complete_payment(
        &cost,
        &pool,
        &sources,
        &[],
        LIFE,
        &Spend::other(),
        &pay(partial),
    );
    match expected {
        Ok(expected) => {
            let whole = done.unwrap();
            assert_eq!(uses(&whole), expected);
            assert_eq!(
                check_payment(&cost, &pool, &sources, &[], LIFE, &Spend::other(), &whole)
                    .map(|_| ()),
                Ok(())
            );
            for tap in &partial
                .iter()
                .map(|(o, a)| source(*o, *a))
                .collect::<Vec<_>>()
            {
                let kept = |p: &Payment| p.taps.iter().filter(|t| *t == tap).count();
                assert!(kept(&whole) >= kept(&pay(partial)), "the chosen uses stay");
            }
        }
        Err(error) => assert_eq!(done.map(|_| ()), Err(error)),
    }
}
