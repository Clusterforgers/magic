//! Goal: the compiled readings of Wretched Confluence, Sublime Epiphany and Kimoyo Beads play at tables of two, three and four seats: "Choose three. You may choose the same mode more than once." asks for exactly three modes that may repeat, each copy of a mode with targets of its own; "Choose one or more" asks for at least one of the legal modes and refuses none; "choose one that hasn't been chosen" leaves out the modes the ability chose in any earlier turn until its source becomes a new object, and every log replays to the live state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_engine::choose::{ModeChoice, ModeOptions, Target};
use mtg_engine::decision::{Answer, DecisionKind, PayChoice};
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{bear, island, keyworded, swamp};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

use crate::common::casting::lands;
use crate::common::pid;
use crate::fight::{life, replays, settle};

const C11D_BIG: &str = "Big";

/// Seat 0 holds `spell` with `count` of `land` ready; the last seat has a 4/4 and a Bear; every seat has a library.
fn c11d_table(
    seats: usize,
    spell: &str,
    land: fn() -> std::sync::Arc<mtg_cards::CardDefinition>,
    count: usize,
) -> Scenario {
    let mut scenario = Scenario::new(seats).hand(0, vec![card(spell).unwrap()]);
    for seat in 0..seats {
        scenario = scenario.library(seat, (0..12).map(|_| bear()));
    }
    scenario = scenario.battlefield(0, lands(count, land));
    scenario.battlefield(
        seats - 1,
        [ready(keyworded(C11D_BIG, 4, 4, &[])), ready(bear())],
    )
}

/// The pending mode decision.
fn c11d_options(driver: &Driver) -> ModeOptions {
    match &driver.decision().unwrap().kind {
        DecisionKind::ChooseMode(options) => options.clone(),
        other => panic!("not a mode decision: {other:?}"),
    }
}

fn c11d_hand(driver: &Driver, seat: usize) -> usize {
    driver.state().zone(Zone::Hand(pid(seat))).len()
}

fn c11d_on_battlefield(driver: &Driver, name: &str) -> bool {
    driver.id_of(name).is_ok_and(|id| {
        driver
            .state()
            .object(id)
            .is_some_and(|object| object.zone() == Zone::Battlefield)
    })
}

/// A target of Wretched Confluence's modes: seat 0 or the 4/4 of the last seat.
#[derive(Clone, Copy, Debug)]
enum C11dAim {
    Me,
    Big,
}

/// Wretched Confluence with these modes, the targets asked in printed order of the modes (CR 700.2): what it did to seat 0's hand and life and to the 4/4.
#[rstest]
#[case::one_mode_three_times(vec![0, 0, 0], vec![C11dAim::Me; 3], 3, -3, true)]
#[case::shrink_twice_and_draw(vec![1, 1, 0], vec![C11dAim::Me, C11dAim::Big, C11dAim::Big], 1, -1, false)]
#[case::draw_twice_and_shrink(vec![0, 0, 1], vec![C11dAim::Me, C11dAim::Me, C11dAim::Big], 2, -2, true)]
fn c11d_three_modes_may_repeat(
    #[values(2, 3, 4)] seats: usize,
    #[case] modes: Vec<usize>,
    #[case] aims: Vec<C11dAim>,
    #[case] drawn: usize,
    #[case] life_change: i32,
    #[case] big_lives: bool,
) -> Result<(), ScriptError> {
    let game = || c11d_table(seats, "Wretched Confluence", swamp, 5);
    let setup = game().setup()?;
    let mut driver = Driver::from_scenario(game())?;
    let (hand, before) = (c11d_hand(&driver, 0), life(&driver, 0));
    driver.begin_cast("Wretched Confluence")?;
    let options = c11d_options(&driver);
    assert_eq!((options.min, options.max, options.repeat), (3, 3, true));
    let two = Answer::ChooseMode(ModeChoice::Modes(vec![0, 0]));
    assert!(driver.answer(two).is_err(), "two modes are too few");
    driver.answer(Answer::ChooseMode(ModeChoice::Modes(modes)))?;
    for aim in aims {
        let target: Target = match aim {
            C11dAim::Me => driver.target_player(0),
            C11dAim::Big => driver.target_named(C11D_BIG)?,
        };
        driver.choose_targets(&[target])?;
    }
    driver.answer(Answer::PayMana(PayChoice::Auto))?;
    settle(&mut driver)?;
    assert_eq!(c11d_hand(&driver, 0), hand - 1 + drawn, "cards drawn");
    assert_eq!(life(&driver, 0), before + life_change, "life");
    assert_eq!(c11d_on_battlefield(&driver, C11D_BIG), big_lives, "the 4/4");
    replays(&driver, &setup);
    Ok(())
}

/// Sublime Epiphany with these modes (no spell or ability to counter, no creature of its own to copy): the Bear of the last seat returns, seat 0 draws.
#[rstest]
#[case::draw_only(vec![4], false, 1)]
#[case::bounce_only(vec![2], true, 0)]
#[case::bounce_and_draw(vec![2, 4], true, 1)]
fn c11d_one_or_more_modes(
    #[values(2, 3, 4)] seats: usize,
    #[case] modes: Vec<usize>,
    #[case] bounced: bool,
    #[case] drawn: usize,
) -> Result<(), ScriptError> {
    let game = || c11d_table(seats, "Sublime Epiphany", island, 6);
    let setup = game().setup()?;
    let mut driver = Driver::from_scenario(game())?;
    let hand = c11d_hand(&driver, 0);
    driver.begin_cast("Sublime Epiphany")?;
    let options = c11d_options(&driver);
    let offered: Vec<usize> = options.modes.iter().map(|mode| mode.index).collect();
    assert_eq!(
        offered,
        vec![2, 4],
        "nothing to counter and no creature to copy"
    );
    assert_eq!((options.min, options.max, options.repeat), (1, 2, false));
    let none = Answer::ChooseMode(ModeChoice::Modes(Vec::new()));
    assert!(driver.answer(none).is_err(), "at least one mode");
    let twice = Answer::ChooseMode(ModeChoice::Modes(vec![4, 4]));
    assert!(driver.answer(twice).is_err(), "no mode twice");
    driver.answer(Answer::ChooseMode(ModeChoice::Modes(modes.clone())))?;
    for mode in &modes {
        let target = if *mode == 2 {
            driver.target_named("Grizzly Bears")?
        } else {
            driver.target_player(0)
        };
        driver.choose_targets(&[target])?;
    }
    driver.answer(Answer::PayMana(PayChoice::Auto))?;
    settle(&mut driver)?;
    assert_eq!(c11d_hand(&driver, 0), hand - 1 + drawn, "cards drawn");
    let bears = driver.state().zone(Zone::Hand(pid(seats - 1))).len();
    assert_eq!(bears, usize::from(bounced), "the Bear in its owner's hand");
    replays(&driver, &setup);
    Ok(())
}

/// Seat 0 has Kimoyo Beads in the first main phase of turn 1; every seat has a library.
fn c11d_beads(seats: usize) -> Scenario {
    let mut scenario = Scenario::new(seats)
        .seed(5)
        .at(1, 0, Step::Main1)
        .battlefield_named(0, ["Kimoyo Beads"]);
    for seat in 0..seats {
        scenario = scenario.library_named(seat, ["Grizzly Bears"; 12]);
    }
    scenario
}

/// Plays to the end step of seat 0's `round`-th turn, where the Beads ask for a mode, answers `mode` and returns what
/// was offered.
fn c11d_end_step(
    driver: &mut Driver,
    seats: usize,
    round: usize,
    mode: usize,
) -> Result<Vec<usize>, ScriptError> {
    let turn = u32::try_from(1 + round * seats).unwrap();
    driver.idle_until(turn, Step::End)?;
    let offered = c11d_options(driver)
        .modes
        .iter()
        .map(|option| option.index)
        .collect();
    driver.answer(Answer::ChooseMode(ModeChoice::Modes(vec![mode])))?;
    settle(driver)?;
    Ok(offered)
}

#[rstest]
fn c11d_a_mode_chosen_in_an_earlier_turn_is_not_offered_again(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let setup = c11d_beads(seats).setup()?;
    let mut driver = Driver::from_scenario(c11d_beads(seats))?;
    assert_eq!(c11d_end_step(&mut driver, seats, 0, 1)?, vec![0, 1, 2]);
    assert_eq!(
        c11d_end_step(&mut driver, seats, 1, 0)?,
        vec![0, 2],
        "the Soldiers were chosen"
    );
    let before = life(&driver, 0);
    assert_eq!(
        c11d_end_step(&mut driver, seats, 2, 2)?,
        vec![2],
        "only the life is left"
    );
    assert_eq!(life(&driver, 0), before + 3);
    // The Beads were exiled and returned: a new object that has chosen nothing.
    assert_eq!(c11d_end_step(&mut driver, seats, 3, 0)?, vec![0, 1, 2]);
    replays(&driver, &setup);
    Ok(())
}
