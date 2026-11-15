//! Goal: a blocker removed from combat leaves unblocked only the attackers no other creature blocked this combat, and a
//! creature an effect has block an attacker of its controller's choice blocks it (CR 506.4, 509.1h), at two to four
//! seats: Balduvian Warlord moves a blocker to another attacker (the attacker it left is unblocked when it blocked
//! alone and stays blocked when another creature blocked it too, even after that one left; back on the same attacker
//! it is blocked again), False Orders may leave the removed creature out of combat, and every log replays.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_engine::choose::CardsChoice;
use mtg_engine::decision::{Answer, DecisionKind, PayChoice};
use mtg_engine::turn::Step;
use mtg_engine::{PresetPermanent, replay};
use mtg_testkit::cards::{free_creature, mountain};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

use crate::common::pid;

/// Creatures with no abilities, each a 1/1.
fn creatures(names: &[&str]) -> Vec<PresetPermanent> {
    names
        .iter()
        .map(|name| ready(free_creature(name, 1, 1)))
        .collect()
}

/// Seat 0 attacks seat 1 with Ape and Bat (holding False Orders and a Mountain when `orders`); seat 1 holds `theirs`
/// and declares `blocks`.
fn blocked(
    seats: usize,
    theirs: Vec<PresetPermanent>,
    orders: bool,
    blocks: &[(&str, &str)],
) -> Result<Driver, ScriptError> {
    let mut attackers = creatures(&["Ape", "Bat"]);
    let mut scenario = Scenario::new(seats).at(2, 0, Step::DeclareAttackers);
    if orders {
        attackers.push(ready(mountain()));
        scenario = scenario.hand(0, [card("False Orders").unwrap()]);
    }
    let mut driver =
        Driver::from_scenario(scenario.battlefield(0, attackers).battlefield(1, theirs))?;
    driver.attack(&[("Ape", 1), ("Bat", 1)])?;
    for _ in 0..40 {
        if matches!(driver.decision()?.kind, DecisionKind::DeclareBlockers(_)) {
            break;
        }
        driver.answer_passively()?;
    }
    driver.block(blocks)?;
    Ok(driver)
}

/// Whether the attacker named is blocked, and by how many creatures.
fn blocked_by(driver: &Driver, name: &str) -> (bool, usize) {
    let id = driver.id_of(name).unwrap();
    let attack = driver
        .state()
        .combat()
        .attackers()
        .iter()
        .find(|attack| attack.object() == id)
        .unwrap();
    (attack.blocked(), attack.blockers().len())
}

/// Lets the stack resolve, choosing the attacker named `to` when an attacker is asked for (none when `to` is `None`).
fn resolve(driver: &mut Driver, to: Option<&str>) -> Result<(), ScriptError> {
    for _ in 0..100 {
        let decision = driver.decision()?.clone();
        match decision.kind {
            DecisionKind::ChooseCards(_) => {
                let picked = to
                    .map(|name| driver.id_of(name).unwrap())
                    .into_iter()
                    .collect();
                driver.answer(Answer::ChooseCards(CardsChoice(picked)))?;
            }
            DecisionKind::PayMana(_) => {
                driver.answer(Answer::PayMana(PayChoice::Auto))?;
            }
            _ if driver.state().stack().is_empty() && driver.state().resolving().is_none() => {
                return Ok(());
            }
            _ => {
                driver.pass()?;
            }
        }
    }
    Err(ScriptError::Stuck("the stack".into()))
}

/// The log replays to the same state.
fn replays(driver: &Driver) {
    let state = replay(driver.engine().setup(), driver.engine().events()).unwrap();
    assert_eq!(state.state_hash(), driver.state().state_hash());
}

/// Gives seat 1 priority in the declare blockers step.
fn to_defender(driver: &mut Driver) -> Result<(), ScriptError> {
    for _ in 0..10 {
        let decision = driver.decision()?;
        if decision.player == pid(1) && matches!(decision.kind, DecisionKind::Priority(_)) {
            return Ok(());
        }
        driver.pass()?;
    }
    Err(ScriptError::Stuck("the defender's priority".into()))
}

// (blocks, the Wall moves to, Ape after, Bat after)
#[rstest]
#[case::alone_to_the_other(&[("Wall", "Ape")], "Bat", (false, 0), (true, 1))]
#[case::alone_back_to_the_same(&[("Wall", "Ape")], "Ape", (true, 1), (false, 0))]
#[case::beside_another(&[("Wall", "Ape"), ("Guard", "Ape")], "Bat", (true, 1), (true, 1))]
#[case::onto_a_blocked_one(&[("Wall", "Ape"), ("Guard", "Bat")], "Bat", (false, 0), (true, 2))]
fn r6400_balduvian_warlord_moves_a_blocker(
    #[values(2, 3, 4)] seats: usize,
    #[case] blocks: &[(&str, &str)],
    #[case] to: &str,
    #[case] ape: (bool, usize),
    #[case] bat: (bool, usize),
) -> Result<(), ScriptError> {
    let mut theirs = creatures(&["Wall", "Guard"]);
    theirs.push(ready(card("Balduvian Warlord").unwrap()));
    let mut driver = blocked(seats, theirs, false, blocks)?;
    to_defender(&mut driver)?;
    driver.begin_ability("Balduvian Warlord", 0)?;
    let wall = driver.target_named("Wall")?;
    driver.choose_targets(&[wall])?;
    resolve(&mut driver, Some(to))?;
    assert_eq!(blocked_by(&driver, "Ape"), ape, "Ape");
    assert_eq!(blocked_by(&driver, "Bat"), bat, "Bat");
    replays(&driver);
    Ok(())
}

#[rstest]
#[case::left_out(None, (false, 0))]
#[case::blocks_again(Some("Bat"), (true, 1))]
fn r6400_false_orders_may_leave_it_out(
    #[values(2, 3, 4)] seats: usize,
    #[case] to: Option<&str>,
    #[case] bat: (bool, usize),
) -> Result<(), ScriptError> {
    let mut driver = blocked(seats, creatures(&["Wall"]), true, &[("Wall", "Ape")])?;
    for _ in 0..10 {
        let decision = driver.decision()?;
        if decision.player == pid(0) && matches!(decision.kind, DecisionKind::Priority(_)) {
            break;
        }
        driver.pass()?;
    }
    driver.begin_cast("False Orders")?;
    let wall = driver.target_named("Wall")?;
    driver.choose_targets(&[wall])?;
    resolve(&mut driver, to)?;
    assert_eq!(blocked_by(&driver, "Ape"), (false, 0), "Ape is unblocked");
    assert_eq!(blocked_by(&driver, "Bat"), bat, "Bat");
    replays(&driver);
    Ok(())
}
