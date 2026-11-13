//! Goal: "each opponent must attack (you or a planeswalker you control) with at least one creature each combat if able"
//! plays as printed at tables of two, three and four seats: Seeker of Slaanesh makes each opponent attack someone with
//! at least one creature, Trove of Temptation makes them attack its controller, neither binds its own controller or a
//! player with no creature able to attack, the automatic declaration meets the requirement, and the log replays.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_engine::decision::DecisionKind;
use mtg_engine::replay;
use mtg_engine::turn::Step;
use mtg_testkit::cards::free_creature;
use mtg_testkit::corpus::card;
use mtg_testkit::scenario::tapped;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

fn r5502_replays(driver: &Driver) {
    let state = replay(driver.engine().setup(), driver.engine().events()).unwrap();
    assert_eq!(state.state_hash(), driver.state().state_hash());
}

/// Seat `holder` controls `card`; seat 0 is active with a Wolf and an Ox (both tapped when `tapped_out`) and declares
/// `attackers`, each at a seat (a seat past the table is skipped).
#[rstest]
#[case::seeker_nobody("Seeker of Slaanesh", 1, false, &[], false)]
#[case::seeker_at_its_controller("Seeker of Slaanesh", 1, false, &[("Wolf", 1)], true)]
#[case::seeker_elsewhere("Seeker of Slaanesh", 1, false, &[("Wolf", 2)], true)]
#[case::seeker_of_your_own("Seeker of Slaanesh", 0, false, &[], true)]
#[case::seeker_none_able("Seeker of Slaanesh", 1, true, &[], true)]
#[case::trove_nobody("Trove of Temptation", 1, false, &[], false)]
#[case::trove_at_its_controller("Trove of Temptation", 1, false, &[("Ox", 1)], true)]
#[case::trove_elsewhere_only("Trove of Temptation", 1, false, &[("Wolf", 2)], false)]
#[case::trove_both("Trove of Temptation", 1, false, &[("Wolf", 2), ("Ox", 1)], true)]
#[case::trove_of_your_own("Trove of Temptation", 0, false, &[], true)]
#[case::trove_none_able("Trove of Temptation", 1, true, &[], true)]
fn r5502_an_opponent_attacks_with_at_least_one(
    #[values(2, 3, 4)] seats: usize,
    #[case] name: &str,
    #[case] holder: usize,
    #[case] tapped_out: bool,
    #[case] attackers: &[(&str, usize)],
    #[case] legal: bool,
) -> Result<(), ScriptError> {
    if attackers.iter().any(|(_, seat)| *seat >= seats) {
        return Ok(());
    }
    let creature = |name: &str| {
        let body = free_creature(name, 2, 2);
        if tapped_out {
            tapped(body)
        } else {
            ready(body)
        }
    };
    let mut mine = vec![creature("Wolf"), creature("Ox")];
    let mut scenario = Scenario::new(seats).at(1, 0, Step::Main1);
    if holder == 0 {
        mine.push(ready(card(name).unwrap()));
    } else {
        scenario = scenario.battlefield(holder, [ready(card(name).unwrap())]);
    }
    let scenario = scenario.battlefield(0, mine);
    let mut driver = Driver::from_scenario(scenario)?;
    driver.pass_until(Step::DeclareAttackers)?;
    if tapped_out {
        // No creature can attack, so no attack is asked of seat 0 this turn (CR 508.1d binds nothing).
        assert!(driver.state().turn().number() > 1, "{name}");
        return Ok(());
    }
    let declared = driver.attack(attackers);
    assert_eq!(
        declared.is_ok(),
        legal,
        "{name}: {attackers:?} {declared:?}"
    );
    if legal {
        r5502_replays(&driver);
    }
    Ok(())
}

/// The declaration an automatic answer gives meets the requirement: one creature attacks, at Trove's controller.
#[rstest]
fn r5502_the_automatic_attack_meets_the_requirement(
    #[values(2, 3, 4)] seats: usize,
    #[values("Seeker of Slaanesh", "Trove of Temptation")] name: &str,
) -> Result<(), ScriptError> {
    let holder = seats - 1;
    let scenario = Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .battlefield(holder, [ready(card(name).unwrap())])
        .battlefield(0, [ready(free_creature("Wolf", 2, 2))]);
    let mut driver = Driver::from_scenario(scenario)?;
    driver.pass_until(Step::DeclareAttackers)?;
    let DecisionKind::DeclareAttackers(options) = driver.decision()?.kind.clone() else {
        panic!("attackers are declared");
    };
    assert_eq!(
        options
            .at_least_one
            .as_ref()
            .map_or(0, |groups| groups.len()),
        1,
        "{name}"
    );
    let attack = options.legal_attack();
    assert_eq!(attack.len(), 1, "{name}");
    assert!(options.accepts(&attack), "{name}");
    if name == "Trove of Temptation" {
        assert_eq!(
            attack[0].1,
            mtg_engine::combat::AttackTarget::Player(mtg_core::ids::PlayerId(
                u64::try_from(holder).unwrap()
            ))
        );
    }
    Ok(())
}
