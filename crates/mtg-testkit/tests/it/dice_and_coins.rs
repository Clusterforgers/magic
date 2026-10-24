//! Goal: rolls and flips play whole through Boompile, Goblin Archaeologist, Mutalith Vortex Beast and Earth-Cult Elemental at two, three and four seats: every outcome is forced through the seed of the game's random stream, the row of the table or the list for the flip that the outcome names is done and no other, each roll and flip is a public event that moves the stream on by one, one flip for each opponent is for that opponent, and the game replays from its log to the same state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_engine::decision::DecisionKind;
use mtg_engine::rng::GameRng;
use mtg_engine::zones::Zone;
use mtg_engine::{Event, replay};
use mtg_testkit::cards::{creature, forest, free_creature, island, mountain, swamp};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

use crate::common::pid;

/// Passes priority and answers every question passively until nothing is on the stack and priority is asked again.
fn resolve(driver: &mut Driver) -> Result<(), ScriptError> {
    for _ in 0..200 {
        let idle = matches!(driver.decision()?.kind, DecisionKind::Priority(_))
            && driver.state().stack().is_empty();
        if idle {
            return Ok(());
        }
        driver.answer_passively()?;
    }
    Err(ScriptError::Stuck("the stack did not empty".to_owned()))
}

/// The first seed whose stream, from the substream `index` on, wins and loses the flips as `wins` says.
fn coin_seed(index: u64, wins: &[bool]) -> u64 {
    (0u64..100_000)
        .find(|seed| {
            wins.iter()
                .zip(index..)
                .all(|(win, at)| (GameRng::substream(*seed, at).below(2) == 0) == *win)
        })
        .unwrap()
}

/// The first seed whose roll of a die of `sides` faces from the substream `index` is `face`.
fn die_seed(index: u64, sides: u64, face: u64) -> u64 {
    (0u64..100_000)
        .find(|seed| GameRng::substream(*seed, index).below(sides) + 1 == face)
        .unwrap()
}

/// Where the stream of a game of this many seats stands before anything is rolled.
fn stream_at(seats: usize) -> u64 {
    Driver::from_scenario(Scenario::new(seats))
        .unwrap()
        .state()
        .rng_index()
}

fn flips(driver: &Driver) -> Vec<bool> {
    driver
        .engine()
        .events()
        .iter()
        .filter_map(|event| match event {
            Event::CoinFlipped { won, .. } => Some(*won),
            _ => None,
        })
        .collect()
}

fn rolls(driver: &Driver) -> Vec<(u32, u32)> {
    driver
        .engine()
        .events()
        .iter()
        .filter_map(|event| match event {
            Event::DieRolled { sides, result, .. } => Some((*sides, *result)),
            _ => None,
        })
        .collect()
}

/// How many permanents the seat controls whose name starts with `prefix`.
fn permanents(driver: &Driver, seat: usize, prefix: &str) -> usize {
    driver
        .state()
        .zone(Zone::Battlefield)
        .iter()
        .filter_map(|id| driver.state().object(*id))
        .filter(|object| object.controller() == pid(seat) && object.card().name.starts_with(prefix))
        .count()
}

fn life(driver: &Driver, seat: usize) -> i32 {
    driver.state().player(pid(seat)).unwrap().life()
}

fn hand(driver: &Driver, seat: usize) -> usize {
    driver.state().zone(Zone::Hand(pid(seat))).len()
}

/// The game replays from its log to the state it ended in, and the stream stood still for nothing it did not use.
fn replays(driver: &Driver, used: u64, start: u64) {
    let replayed = replay(driver.engine().setup(), driver.engine().events()).unwrap();
    assert_eq!(replayed.state_hash(), driver.state().state_hash());
    assert_eq!(
        driver.state().rng_index(),
        start + used,
        "the stream moved on once for each roll and flip"
    );
}

/// Boompile: {T}: flip a coin; a won flip destroys every nonland permanent, itself and the creatures of every seat, and a
/// lost flip does nothing.
#[rstest]
fn boompile_destroys_all_nonland_permanents_when_the_flip_is_won(
    #[values(2, 3, 4)] seats: usize,
    #[values(true, false)] win: bool,
) -> Result<(), ScriptError> {
    let start = stream_at(seats);
    let mut scenario = Scenario::new(seats)
        .seed(coin_seed(start, &[win]))
        .battlefield(
            0,
            [
                ready(card("Boompile").unwrap()),
                ready(free_creature("Bear 0", 2, 2)),
                ready(forest()),
            ],
        );
    for seat in 1..seats {
        scenario = scenario.battlefield(
            seat,
            [
                ready(free_creature(&format!("Bear {seat}"), 2, 2)),
                ready(swamp()),
            ],
        );
    }
    let mut driver = Driver::from_scenario(scenario)?;
    driver.begin_activation("Boompile")?;
    resolve(&mut driver)?;
    assert_eq!(flips(&driver), [win]);
    let kept = usize::from(!win);
    assert_eq!(permanents(&driver, 0, "Bear"), kept);
    assert_eq!(permanents(&driver, 0, "Boompile"), kept);
    for seat in 1..seats {
        assert_eq!(permanents(&driver, seat, "Bear"), kept, "seat {seat}");
        assert_eq!(permanents(&driver, seat, "Swamp"), 1, "lands stay");
    }
    replays(&driver, 1, start);
    Ok(())
}

/// Goblin Archaeologist: a won flip destroys the target artifact and untaps it, a lost flip sacrifices it.
#[rstest]
fn goblin_archaeologist_destroys_the_artifact_and_untaps_or_is_sacrificed(
    #[values(2, 3, 4)] seats: usize,
    #[values(true, false)] win: bool,
) -> Result<(), ScriptError> {
    let start = stream_at(seats);
    let mut driver = Driver::from_scenario(
        Scenario::new(seats)
            .seed(coin_seed(start, &[win]))
            .battlefield(
                0,
                [
                    ready(card("Goblin Archaeologist").unwrap()),
                    ready(mountain()),
                ],
            )
            .battlefield(1, [ready(card("Boompile").unwrap())]),
    )?;
    driver.begin_activation("Goblin Archaeologist")?;
    let artifact = driver.target_named("Boompile")?;
    driver.choose_targets(&[artifact])?;
    driver.pay_tapping(&["Mountain"], 0)?;
    resolve(&mut driver)?;
    assert_eq!(flips(&driver), [win]);
    assert_eq!(
        permanents(&driver, 1, "Boompile"),
        usize::from(!win),
        "the artifact"
    );
    assert_eq!(
        permanents(&driver, 0, "Goblin Archaeologist"),
        usize::from(win),
        "the creature"
    );
    if win {
        let id = driver.id_of("Goblin Archaeologist")?;
        assert!(
            !driver
                .state()
                .object(id)
                .unwrap()
                .permanent()
                .unwrap()
                .tapped(),
            "untapped"
        );
    }
    replays(&driver, 1, start);
    Ok(())
}

/// Mutalith Vortex Beast flips a coin for each opponent: a won flip draws the controller a card and a lost flip deals 3
/// damage to the opponent that flip is for.
#[rstest]
#[case::two_seats_won(2, &[true])]
#[case::two_seats_lost(2, &[false])]
#[case::three_seats_won_then_lost(3, &[true, false])]
#[case::three_seats_lost_then_won(3, &[false, true])]
#[case::three_seats_both_lost(3, &[false, false])]
#[case::four_seats_mixed(4, &[false, true, false])]
#[case::four_seats_all_won(4, &[true, true, true])]
fn mutalith_vortex_beast_flips_once_for_each_opponent(
    #[case] seats: usize,
    #[case] wins: &[bool],
) -> Result<(), ScriptError> {
    let start = stream_at(seats);
    let mut driver = Driver::from_scenario(
        Scenario::new(seats)
            .seed(coin_seed(start, wins))
            .hand(0, [card("Mutalith Vortex Beast").unwrap()])
            .battlefield(
                0,
                [
                    ready(island()),
                    ready(mountain()),
                    ready(forest()),
                    ready(forest()),
                    ready(forest()),
                    ready(forest()),
                ],
            ),
    )?;
    let (hand_before, lives): (usize, Vec<i32>) = (
        hand(&driver, 0),
        (0..seats).map(|s| life(&driver, s)).collect(),
    );
    driver.cast("Mutalith Vortex Beast")?;
    resolve(&mut driver)?;
    assert_eq!(flips(&driver), wins);
    let won = wins.iter().filter(|win| **win).count();
    assert_eq!(
        hand(&driver, 0),
        hand_before - 1 + won,
        "a card for each flip won, the Beast cast"
    );
    assert_eq!(life(&driver, 0), lives[0]);
    for (opponent, win) in (1..seats).zip(wins) {
        let expected = lives[opponent] - if *win { 0 } else { 3 };
        assert_eq!(
            life(&driver, opponent),
            expected,
            "the flip for seat {opponent}"
        );
    }
    replays(&driver, u64::try_from(wins.len()).unwrap(), start);
    Ok(())
}

/// Earth-Cult Elemental rolls a d20 when it enters: 1 to 9 every player sacrifices a permanent, 10 to 19 each opponent does,
/// 20 each opponent sacrifices two.
#[rstest]
#[case::the_lowest_of_the_first_row(1, 1, 1)]
#[case::the_highest_of_the_first_row(9, 1, 1)]
#[case::the_lowest_of_the_second_row(10, 0, 1)]
#[case::the_highest_of_the_second_row(19, 0, 1)]
#[case::the_last_row(20, 0, 2)]
fn earth_cult_elemental_reads_the_row_of_its_roll(
    #[values(2, 3, 4)] seats: usize,
    #[case] face: u64,
    #[case] mine_lost: usize,
    #[case] theirs_lost: usize,
) -> Result<(), ScriptError> {
    let start = stream_at(seats);
    let mut scenario = Scenario::new(seats)
        .seed(die_seed(start, 20, face))
        .hand(0, [card("Earth-Cult Elemental").unwrap()])
        .battlefield(
            0,
            (0..6)
                .map(|_| ready(mountain()))
                .chain([ready(creature("Idol", 0, &[], 1, 1))]),
        );
    for seat in 1..seats {
        scenario = scenario.battlefield(
            seat,
            (0..3).map(|n| ready(free_creature(&format!("Pawn {seat}-{n}"), 1, 1))),
        );
    }
    let mut driver = Driver::from_scenario(scenario)?;
    driver.cast("Earth-Cult Elemental")?;
    resolve(&mut driver)?;
    assert_eq!(rolls(&driver), [(20, u32::try_from(face).unwrap())]);
    // Seat 0 holds the Elemental and six lands whose names start with "Mountain"; one of the seven is sacrificed in the
    // first row.
    let held = permanents(&driver, 0, "Mountain")
        + permanents(&driver, 0, "Idol")
        + permanents(&driver, 0, "Earth-Cult");
    assert_eq!(held, 8 - mine_lost, "seat 0");
    for seat in 1..seats {
        assert_eq!(
            permanents(&driver, seat, "Pawn"),
            3 - theirs_lost,
            "seat {seat}"
        );
    }
    replays(&driver, 1, start);
    Ok(())
}
