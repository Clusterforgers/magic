//! Goal: Mana Short plays at tables of two to four seats as its rules say: every land the target player controls is tapped and that player loses all unspent mana, while another player keeps the mana in their pool.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{forest, island, swamp};
use mtg_testkit::corpus::corpus;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

use crate::common::pid;

/// Seat 0 holds Mana Short, three Islands and a Swamp; the last seat controls two Forests.
fn rmt1_table(seats: usize) -> Scenario {
    let short = corpus().unwrap().card("Mana Short").unwrap();
    Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .battlefield(
            0,
            [
                ready(island()),
                ready(island()),
                ready(island()),
                ready(swamp()),
            ],
        )
        .hand(0, [short])
        .battlefield(seats - 1, [ready(forest()), ready(forest())])
}

fn rmt1_untapped_lands(driver: &Driver, seat: usize) -> usize {
    let state = driver.state();
    state
        .zone(Zone::Battlefield)
        .iter()
        .filter_map(|id| state.object(*id))
        .filter(|object| {
            object.controller() == pid(seat) && object.permanent().is_some_and(|p| !p.tapped())
        })
        .count()
}

#[rstest]
fn rmt1_the_target_player_taps_out_and_loses_unspent_mana(
    #[values(2, 3, 4)] seats: usize,
    #[values(true, false)] at_caster: bool,
) -> Result<(), ScriptError> {
    let target = if at_caster { 0 } else { seats - 1 };
    let mut driver = Driver::from_scenario(rmt1_table(seats))?;
    driver.begin_cast("Mana Short")?;
    let player = driver.target_player(target);
    driver.choose_targets(&[player])?;
    driver.pay_tapping(&["Island", "Island", "Island"], 0)?;
    // The caster floats black mana with the spell on the stack.
    driver.tap_for_mana("Swamp")?;
    assert_eq!(driver.state().player(pid(0)).unwrap().mana().total(), 1);
    for _ in 0..40 {
        if driver.state().stack().is_empty() && driver.state().resolving().is_none() {
            break;
        }
        driver.answer_passively()?;
    }
    assert_eq!(rmt1_untapped_lands(&driver, target), 0);
    assert_eq!(
        driver.state().player(pid(target)).unwrap().mana().total(),
        0
    );
    let kept = u32::from(!at_caster);
    assert_eq!(driver.state().player(pid(0)).unwrap().mana().total(), kept);
    Ok(())
}
