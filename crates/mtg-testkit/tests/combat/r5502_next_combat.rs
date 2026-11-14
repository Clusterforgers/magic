//! Goal: "target creature an opponent controls attacks during its controller's next combat phase if able" plays as
//! printed at tables of two, three and four seats: Trench Behemoth's landfall trigger makes the targeted creature a
//! required attacker in its controller's next combat (an empty attack is refused, attacking with it is accepted), the
//! requirement is gone by that player's following turn, and the log replays.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_engine::choose::TargetChoice;
use mtg_engine::decision::{Answer, DecisionKind};
use mtg_engine::replay;
use mtg_engine::turn::Step;
use mtg_testkit::cards::{forest, free_creature};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

/// The creatures the attack options of the deciding player require.
fn r5502_required(driver: &Driver) -> Result<usize, ScriptError> {
    let DecisionKind::DeclareAttackers(options) = &driver.decision()?.kind else {
        panic!("attackers are declared");
    };
    Ok(options.required.len())
}

#[rstest]
fn r5502_trench_behemoth_sends_a_creature_into_its_controllers_next_combat(
    #[values(2, 3, 4)] seats: usize,
    #[values(false, true)] attacks: bool,
) -> Result<(), ScriptError> {
    let scenario = Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .battlefield(0, [ready(card("Trench Behemoth").unwrap())])
        .battlefield(1, [ready(free_creature("Brute", 2, 2))])
        .hand(0, [forest()]);
    let mut driver = Driver::from_scenario(scenario)?;
    driver.play_land("Forest")?;
    let brute = driver.target_named("Brute")?;
    driver.answer(Answer::ChooseTargets(TargetChoice::Targets(vec![brute])))?;
    driver.idle_until(2, Step::DeclareAttackers)?;
    assert_eq!(driver.decision()?.player.index(), 1);
    assert_eq!(r5502_required(&driver)?, 1);
    if attacks {
        driver.attack(&[("Brute", 0)])?;
    } else {
        assert!(driver.attack(&[]).is_err(), "the Brute must attack");
        driver.attack(&[("Brute", 0)])?;
    }
    let state = replay(driver.engine().setup(), driver.engine().events()).unwrap();
    assert_eq!(state.state_hash(), driver.state().state_hash());
    let next = u32::try_from(seats).unwrap() + 2;
    driver.idle_until(next, Step::DeclareAttackers)?;
    assert_eq!(driver.decision()?.player.index(), 1);
    assert_eq!(
        r5502_required(&driver)?,
        0,
        "the requirement ended with that combat"
    );
    Ok(())
}
