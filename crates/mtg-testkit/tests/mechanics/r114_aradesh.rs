//! Goal: Aradesh, the Founder at 2 to 4 seats: enlist is chosen as the attack is declared (CR 702.154a), so "whenever a
//! creature you control attacks, if it enlisted a creature this combat" sees it at once: the creature that enlisted gains
//! double strike and, with power 4 or greater, draws its controller a card; one that enlisted nothing gets neither, and
//! the log replays to the live state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_cards::keyword::Keyword;
use mtg_engine::replay;
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{free_creature, keyworded, plains};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

use crate::common::pid;
use crate::macro_keywords::{macro_keyword, r114_play_enlisting};

#[rstest]
fn r114_a_creature_that_enlisted_gains_double_strike(
    #[values(2, 3, 4)] seats: usize,
    #[values(false, true)] enlist: bool,
) -> Result<(), ScriptError> {
    let mut scenario = Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .battlefield(
            0,
            vec![
                ready(card("Aradesh, the Founder").unwrap()),
                ready(keyworded("Brute", 4, 4, &[macro_keyword("enlist", 0)])),
                ready(free_creature("Helper", 3, 3)),
            ],
        )
        .library(0, [plains(), plains()]);
    for seat in 1..seats {
        scenario = scenario.battlefield(
            seat,
            vec![ready(free_creature(&format!("Wall {seat}"), 0, 9))],
        );
    }
    let mut driver = Driver::from_scenario(scenario)?;
    let enlisting: &[(&str, &str)] = if enlist { &[("Brute", "Helper")] } else { &[] };
    r114_play_enlisting(
        &mut driver,
        &["Brute"],
        enlisting,
        &|_, _| None,
        Step::EndCombat,
    )?;
    let brute = driver.id_of("Brute")?;
    let double = mtg_engine::keywords::keywords_of(driver.state(), brute)
        .into_iter()
        .any(|keyword| *keyword == Keyword::DoubleStrike);
    assert_eq!(double, enlist);
    assert_eq!(
        driver.state().zone(Zone::Hand(pid(0))).len(),
        usize::from(enlist)
    );
    let helper = driver.id_of("Helper")?;
    let tapped = driver
        .state()
        .object(helper)
        .unwrap()
        .permanent()
        .unwrap()
        .tapped();
    assert_eq!(tapped, enlist);
    let state = replay(driver.engine().setup(), driver.engine().events()).unwrap();
    assert_eq!(state.state_hash(), driver.state().state_hash());
    Ok(())
}
