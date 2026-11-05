//! Goal: a cost with `{S}` is paid only when that many of the mana spent come from snow sources (floating or tapped), whichever of the payment functions asks, and the snow mana spent is counted.

use mtg_cards::spend::{ManaLasts, SpendOn};
use mtg_core::ids::ObjectId;
use mtg_core::mana::{ManaCost, ManaType};
use rstest::rstest;

use crate::error::PaymentError;
use crate::mana::{
    ManaOption, ManaPool, ManaSource, Payment, auto_payment, can_afford, check_payment,
    complete_payment,
};
use crate::mana_snow::spent_snow;
use crate::spend::Spend;

const LIFE: i32 = 20;

fn cost(text: &str) -> ManaCost {
    ManaCost::parse(text).unwrap()
}

/// A land at `id` that taps for `mana`; a snow land when `snow`.
fn land(id: u64, mana: ManaType, snow: bool) -> ManaOption {
    let mut option = ManaOption::new(
        ManaSource {
            object: ObjectId(id),
            ability: 0,
        },
        mana,
    );
    if snow {
        option.only = vec![SpendOn::Snow];
    }
    option
}

fn pool(snow: &[ManaType], plain: &[ManaType]) -> ManaPool {
    let mut pool = ManaPool::default();
    for mana in plain {
        pool.add(*mana);
    }
    for mana in snow {
        pool.add_restricted(*mana, vec![SpendOn::Snow], ManaLasts::Step);
    }
    pool
}

use ManaType::{Colorless, Green, Red};

#[rstest]
#[case::snow_land_pays("{S}", &[land(1, Green, true)], &[], true)]
#[case::plain_land_cannot("{S}", &[land(1, Green, false)], &[], false)]
#[case::one_of_two_is_snow("{S}", &[land(1, Green, false), land(2, Red, true)], &[], true)]
#[case::snow_and_generic("{1}{S}", &[land(1, Green, false), land(2, Red, true)], &[], true)]
#[case::two_symbols_need_two_snow("{S}{S}", &[land(1, Green, false), land(2, Red, true)], &[], false)]
#[case::two_symbols_two_snow("{S}{S}", &[land(1, Green, true), land(2, Red, true)], &[], true)]
#[case::color_and_snow_apart("{G}{S}", &[land(1, Green, true), land(2, Red, true)], &[], true)]
#[case::snow_land_is_the_color("{G}{S}", &[land(1, Green, true), land(2, Red, false)], &[], false)]
#[case::floating_snow("{S}", &[], &[(Red, true)], true)]
#[case::floating_plain_mana("{S}", &[], &[(Red, false)], false)]
#[case::floating_and_tapped("{S}{R}", &[land(1, Red, false)], &[(Green, true)], true)]
fn a_snow_cost_needs_snow_mana(
    #[case] text: &str,
    #[case] lands: &[ManaOption],
    #[case] floating: &[(ManaType, bool)],
    #[case] payable: bool,
) {
    let snow: Vec<ManaType> = floating.iter().filter(|f| f.1).map(|f| f.0).collect();
    let plain: Vec<ManaType> = floating.iter().filter(|f| !f.1).map(|f| f.0).collect();
    let pool = pool(&snow, &plain);
    let cost = cost(text);
    let spend = Spend::other();
    assert_eq!(can_afford(&cost, &pool, lands, LIFE, &spend), payable);
    let payment = auto_payment(&cost, &pool, lands, LIFE, &spend);
    assert_eq!(payment.is_some(), payable);
    if let Some(payment) = payment {
        let spent = check_payment(&cost, &pool, lands, &[], LIFE, &spend, &payment).unwrap();
        assert!(spent_snow(&spent) >= cost.snow());
        let done = complete_payment(&cost, &pool, lands, &[], LIFE, &spend, &Payment::default());
        assert!(done.is_ok());
    }
}

#[test]
fn a_payment_without_enough_snow_is_refused() {
    let lands = [land(1, Green, false), land(2, Red, true)];
    let by_hand = Payment {
        taps: vec![lands[0].source],
        ..Payment::default()
    };
    assert_eq!(
        check_payment(
            &cost("{S}"),
            &ManaPool::default(),
            &lands,
            &[],
            LIFE,
            &Spend::other(),
            &by_hand
        ),
        Err(PaymentError::MissingSnow)
    );
}

#[test]
fn completing_a_partial_payment_adds_the_snow_mana() {
    let lands = [land(1, Green, false), land(2, Red, true)];
    let partial = Payment {
        taps: vec![lands[0].source],
        ..Payment::default()
    };
    let done = complete_payment(
        &cost("{1}{S}"),
        &ManaPool::default(),
        &lands,
        &[],
        LIFE,
        &Spend::other(),
        &partial,
    )
    .unwrap();
    assert_eq!(done.taps.len(), 2);
    assert!(done.taps.contains(&lands[1].source));
}

#[test]
fn colorless_snow_is_still_snow() {
    let lands = [land(1, Colorless, true)];
    assert!(can_afford(
        &cost("{S}"),
        &ManaPool::default(),
        &lands,
        LIFE,
        &Spend::other()
    ));
}
