//! Goal: the simulator reads the round each fight ended in and the boss phases it reached (only the boss side's), and
//! tallies them into a median, the share that reached each phase and the share that ended in each round.

use mtg_campaign::sim::{FightLength, Lengths};
use mtg_core::ids::{ObjectId, PlayerId};
use mtg_engine::events::Event;
use rstest::rstest;

/// A phase of seat `seat` beginning.
fn phase(seat: u64) -> Event {
    Event::PhaseBegan {
        player: PlayerId(seat),
        emblem: ObjectId(100 + seat),
    }
}

#[rstest]
#[case::no_phase(vec![], 2, 0)]
#[case::boss_phase_one(vec![phase(2)], 2, 1)]
#[case::boss_phase_two(vec![phase(2), phase(2)], 2, 2)]
#[case::a_hero_phase_is_not_the_boss_s(vec![phase(0), phase(1)], 2, 0)]
#[case::four_heroes(vec![phase(3), phase(4)], 4, 1)]
fn a_fight_counts_the_boss_side_s_phases(
    #[case] events: Vec<Event>,
    #[case] party: usize,
    #[case] phases: u8,
) {
    assert_eq!(
        FightLength::of(12, &events, party),
        FightLength { round: 12, phases }
    );
}

/// Lengths of fights given as (round, phases).
fn lengths(fights: &[(u32, u8)]) -> Lengths {
    let mut lengths = Lengths::default();
    for (round, phases) in fights {
        lengths.add(FightLength {
            round: *round,
            phases: *phases,
        });
    }
    lengths
}

#[rstest]
#[case::none(&[], None, 0, 0, "median=- p1=0% p2=0% ended=-")]
#[case::one(&[(9, 1)], Some(9), 100, 0, "median=9 p1=100% p2=0% ended=9:100%")]
#[case::spread(
    &[(7, 0), (9, 1), (11, 1), (15, 2)],
    Some(9),
    75,
    25,
    "median=9 p1=75% p2=25% ended=7:25%,9:25%,11:25%,15:25%"
)]
#[case::thirds(
    &[(14, 1), (15, 2), (10, 1)],
    Some(14),
    100,
    33,
    "median=14 p1=100% p2=33% ended=10:33%,14:33%,15:33%"
)]
fn lengths_tally_into_one_line(
    #[case] fights: &[(u32, u8)],
    #[case] median: Option<u32>,
    #[case] p1: u32,
    #[case] p2: u32,
    #[case] fields: &str,
) {
    let lengths = lengths(fights);
    assert_eq!(lengths.len(), fights.len());
    assert_eq!(lengths.median(), median);
    assert_eq!(lengths.reached(1), p1);
    assert_eq!(lengths.reached(2), p2);
    assert_eq!(lengths.fields(), fields);
}

#[test]
fn merged_lengths_hold_both_series() {
    let mut all = lengths(&[(8, 0), (9, 1)]);
    all.merge(&lengths(&[(14, 2)]));
    assert_eq!(all, lengths(&[(8, 0), (9, 1), (14, 2)]));
    assert_eq!(all.by_round().len(), 7);
    assert_eq!(all.by_round().iter().filter(|(_, s)| *s > 0).count(), 3);
}
