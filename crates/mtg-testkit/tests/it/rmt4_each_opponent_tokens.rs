//! Goal: "for each opponent, create a token that's tapped and attacking that player" (Stampede Surfer, Endless Foot Assault) makes one tapped token for each opponent, each attacking a different opponent, at tables of two, three and four seats; every game replays to the same state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::collections::BTreeSet;

use mtg_core::ids::PlayerId;
use mtg_engine::object::PermanentState;
use mtg_engine::turn::Step;
use mtg_testkit::cards::free_creature;
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

use crate::c4_control::{replays, settle};
use crate::common::name_of;

/// Seat 0 has the card and a Bear ready to attack; the game is in seat 0's first main phase.
fn table(seats: usize, name: &str) -> Scenario {
    Scenario::new(seats).at(1, 0, Step::Main1).battlefield(
        0,
        [
            ready(card(name).unwrap()),
            ready(free_creature("Bear", 2, 2)),
        ],
    )
}

/// The players each token named `token` attacks, after the attack trigger resolved.
fn attacked_by(driver: &Driver, token: &str) -> Vec<PlayerId> {
    let state = driver.state();
    state
        .combat()
        .attackers()
        .iter()
        .filter(|a| name_of(state, a.object()) == token)
        .map(|a| {
            let tapped = state
                .object(a.object())
                .and_then(|o| o.permanent())
                .is_some_and(PermanentState::tapped);
            assert!(tapped, "the token enters tapped");
            a.defender(state).unwrap()
        })
        .collect()
}

#[rstest]
#[case::stampede_surfer("Stampede Surfer", "Stampede Surfer", "Boar")]
#[case::endless_foot_assault("Endless Foot Assault", "Bear", "Ninja")]
fn one_token_attacks_each_opponent(
    #[case] name: &str,
    #[case] attacker: &str,
    #[case] token: &str,
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(table(seats, name))?;
    driver.pass_until(Step::DeclareAttackers)?;
    driver.attack(&[(attacker, 1)])?;
    settle(&mut driver, None)?;
    let attacked = attacked_by(&driver, token);
    assert_eq!(attacked.len(), seats - 1, "one token for each opponent");
    let distinct: BTreeSet<usize> = attacked.iter().map(|p| p.index()).collect();
    assert_eq!(distinct, (1..seats).collect::<BTreeSet<_>>());
    replays(&driver);
    Ok(())
}
