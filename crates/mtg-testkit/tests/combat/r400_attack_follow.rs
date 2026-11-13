//! Goal: attack requirements that follow another attacker play as printed at tables of two, three and four seats: Ekundu
//! Cyclops must join an attack of another creature its controller controls while it can attack (never when it is tapped),
//! a creature with a magnet counter under Magnetic Web brings every other creature with one into the attack (a creature
//! without one stays free), and each creature with a magnet counter on the defending side must block a magnetised
//! attacker; the log replays to the state it was played to.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_cards::value::CounterKind;
use mtg_engine::decision::DecisionKind;
use mtg_engine::replay;
use mtg_engine::turn::Step;
use mtg_testkit::cards::free_creature;
use mtg_testkit::corpus::card;
use mtg_testkit::scenario::{tapped, with_counters};
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

fn r400_magnet() -> CounterKind {
    CounterKind::Other("magnet".to_owned())
}

fn r400_replays(driver: &Driver) {
    let state = replay(driver.engine().setup(), driver.engine().events()).unwrap();
    assert_eq!(state.state_hash(), driver.state().state_hash());
}

/// Seat 0 holds Ekundu Cyclops (tapped when `cyclops_tapped`) and a Wolf, and declares `attackers` at seat 1.
#[rstest]
#[case::the_wolf_alone(&["Wolf"], false, false)]
#[case::both(&["Wolf", "Ekundu Cyclops"], false, true)]
#[case::the_cyclops_alone(&["Ekundu Cyclops"], false, true)]
#[case::nobody(&[], false, true)]
#[case::the_wolf_beside_a_tapped_cyclops(&["Wolf"], true, true)]
fn r400_ekundu_joins_an_attack(
    #[values(2, 3, 4)] seats: usize,
    #[case] attackers: &[&str],
    #[case] cyclops_tapped: bool,
    #[case] legal: bool,
) -> Result<(), ScriptError> {
    let cyclops = card("Ekundu Cyclops").unwrap();
    let cyclops = if cyclops_tapped {
        tapped(cyclops)
    } else {
        ready(cyclops)
    };
    let scenario = Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .battlefield(0, [cyclops, ready(free_creature("Wolf", 2, 2))]);
    let mut driver = Driver::from_scenario(scenario)?;
    driver.pass_until(Step::DeclareAttackers)?;
    let declared: Vec<(&str, usize)> = attackers.iter().map(|name| (*name, 1)).collect();
    assert_eq!(driver.attack(&declared).is_ok(), legal, "{attackers:?}");
    if legal {
        r400_replays(&driver);
    }
    Ok(())
}

/// Seat 0 holds Magnetic Web, two creatures with magnet counters (an Ox and a Yak) and a Wolf without one, and declares
/// `attackers` at seat 1.
#[rstest]
#[case::one_magnet(&["Ox"], false)]
#[case::both_magnets(&["Ox", "Yak"], true)]
#[case::the_free_wolf(&["Wolf"], true)]
#[case::the_wolf_and_one_magnet(&["Wolf", "Yak"], false)]
fn r400_magnets_attack_together(
    #[values(2, 3, 4)] seats: usize,
    #[case] attackers: &[&str],
    #[case] legal: bool,
) -> Result<(), ScriptError> {
    let scenario = Scenario::new(seats).at(1, 0, Step::Main1).battlefield(
        0,
        [
            ready(card("Magnetic Web").unwrap()),
            with_counters(ready(free_creature("Ox", 2, 2)), r400_magnet(), 1),
            with_counters(ready(free_creature("Yak", 2, 2)), r400_magnet(), 1),
            ready(free_creature("Wolf", 2, 2)),
        ],
    );
    let mut driver = Driver::from_scenario(scenario)?;
    driver.pass_until(Step::DeclareAttackers)?;
    let declared: Vec<(&str, usize)> = attackers.iter().map(|name| (*name, 1)).collect();
    assert_eq!(driver.attack(&declared).is_ok(), legal, "{attackers:?}");
    Ok(())
}

/// Seat 0's Ox with a magnet counter attacks seat 1, which holds a Gnu with a magnet counter and a Wolf without one; the
/// web's trigger makes the Gnu block the Ox (`blocks` is refused when it leaves the Gnu out), and leaves the Wolf free.
#[rstest]
#[case::no_block(&[], false)]
#[case::the_wolf_only(&[("Wolf", "Ox")], false)]
#[case::the_gnu(&[("Gnu", "Ox")], true)]
#[case::both(&[("Gnu", "Ox"), ("Wolf", "Ox")], true)]
fn r400_magnets_block_a_magnet(
    #[values(2, 3, 4)] seats: usize,
    #[case] blocks: &[(&str, &str)],
    #[case] legal: bool,
) -> Result<(), ScriptError> {
    let scenario = Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .battlefield(
            0,
            [
                ready(card("Magnetic Web").unwrap()),
                with_counters(ready(free_creature("Ox", 2, 2)), r400_magnet(), 1),
            ],
        )
        .battlefield(
            1,
            [
                with_counters(ready(free_creature("Gnu", 2, 2)), r400_magnet(), 1),
                ready(free_creature("Wolf", 2, 2)),
            ],
        );
    let mut driver = Driver::from_scenario(scenario)?;
    driver.pass_until(Step::DeclareAttackers)?;
    driver.attack(&[("Ox", 1)])?;
    for _ in 0..40 {
        if matches!(driver.decision()?.kind, DecisionKind::DeclareBlockers(_)) {
            break;
        }
        driver.answer_passively()?;
    }
    assert_eq!(driver.block(blocks).is_ok(), legal, "{blocks:?}");
    if legal {
        r400_replays(&driver);
    }
    Ok(())
}
