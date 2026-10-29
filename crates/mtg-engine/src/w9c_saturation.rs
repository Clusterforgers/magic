//! Goal: numbers that legitimately grow without bound (a power doubled turn after turn, counters, a life total) stop at
//! the largest value they hold instead of overflowing: a pump past it, counters put past it and life lost or gained by
//! more than a life total holds.

use mtg_cards::value::CounterKind;
use mtg_core::ids::PlayerId;
use rstest::rstest;
use std::collections::BTreeMap;

use crate::characteristics::characteristics;
use crate::effects::Modification;
use crate::events::Event;
use crate::fixtures::{creature, state};
use crate::fold::apply_event;
use crate::zones::Zone;

#[rstest]
#[case::up(i32::MAX, i32::MAX)]
#[case::down(i32::MIN, i32::MIN)]
fn a_pump_past_the_largest_power_stops_there(#[case] by: i32, #[case] expected: i32) {
    let game = state(
        vec![vec![], vec![]],
        vec![vec![creature("Bear", 2, 2)], vec![]],
    );
    let bear = game.zone(Zone::Battlefield)[0];
    let mut chars = characteristics(&game, bear).unwrap();
    for _ in 0..2 {
        Modification::ModifyPowerToughness {
            power: by,
            toughness: by,
        }
        .apply(&mut chars);
    }
    assert_eq!(chars.power, Some(expected));
    assert_eq!(chars.toughness, chars.power);
}

#[test]
fn counters_put_past_the_largest_count_stop_there() {
    let mut held = BTreeMap::from([(CounterKind::PlusOnePlusOne, u32::MAX - 1)]);
    crate::counters::change(&mut held, &CounterKind::PlusOnePlusOne, i32::MAX).unwrap();
    assert_eq!(held[&CounterKind::PlusOnePlusOne], u32::MAX);
}

#[rstest]
#[case::lost(false, 20 - i32::MAX)]
#[case::gained(true, i32::MAX)]
fn life_changed_by_more_than_a_total_holds_goes_as_far_as_it_can(
    #[case] gained: bool,
    #[case] expected: i32,
) {
    let mut game = state(vec![vec![], vec![]], vec![vec![], vec![]]);
    let player = PlayerId(1);
    let event = if gained {
        Event::LifeGained {
            player,
            amount: u32::MAX,
        }
    } else {
        Event::LifeLost {
            player,
            amount: u32::MAX,
        }
    };
    apply_event(&mut game, &event).unwrap();
    assert_eq!(game.player(player).unwrap().life(), expected);
}
