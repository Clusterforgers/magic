//! Goal: counters put past the largest count the engine holds stop there, and the invariant checker's own tally stops
//! with them: a doubler that grows counters without bound keeps the invariants, and counters taken off after are
//! taken from the saturated count; at two to four seats.
use crate::common;

use common::pid;
use mtg_cards::value::CounterKind;
use mtg_engine::choose::Target;
use mtg_engine::counters::CountersChanged;
use mtg_engine::events::Event;
use mtg_engine::replay;
use mtg_engine::setup::PresetPermanent;
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::creature;
use mtg_testkit::{InvariantChecker, Scenario};
use rstest::rstest;

#[rstest]
#[case::one_past(&[1, 1], u32::MAX)]
#[case::far_past(&[i32::MAX, i32::MAX, i32::MAX], u32::MAX)]
#[case::past_then_off(&[i32::MAX, -3], u32::MAX - 3)]
#[case::up_to_it(&[1], u32::MAX)]
#[case::below_it(&[-10, 4], u32::MAX - 7)]
fn fix13_saturated_counters_keep_the_invariants(
    #[values(2, 3, 4)] seats: usize,
    #[case] deltas: &[i32],
    #[case] held: u32,
) {
    let mut permanent = PresetPermanent::new(creature("Grizzly", 1, &[], 1, 1));
    permanent.counters = vec![(CounterKind::PlusOnePlusOne, u32::MAX - 1)];
    let setup = Scenario::new(seats)
        .at(3, 1, Step::Main1)
        .battlefield(0, [permanent])
        .setup()
        .unwrap();
    let start = replay(&setup, &[]).unwrap();
    let id = start.zone(Zone::Battlefield)[0];
    let named = start.object(id).unwrap().reference();
    let log: Vec<Event> = deltas
        .iter()
        .map(|&delta| {
            Event::CountersChanged(CountersChanged {
                on: Target::Object(named),
                counter: CounterKind::PlusOnePlusOne,
                delta,
            })
        })
        .collect();
    let live = replay(&setup, &log).unwrap();
    let permanent = live.object(id).unwrap().permanent().unwrap();
    assert_eq!(permanent.counters_of(&CounterKind::PlusOnePlusOne), held);
    let mut checker = InvariantChecker::new(&setup).unwrap();
    assert_eq!(checker.observe(&live, &log), Ok(()));
    assert_eq!(live.object(id).unwrap().owner(), pid(0));
}
