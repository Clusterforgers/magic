//! Goal: "Equipped creature gets +3/+1 and must be blocked by an Eldrazi if able." plays as printed at tables of two,
//! three and four seats: the equipped attacker has +3/+1, a defending player with an Eldrazi able to block it must block
//! it with one (no block or a block by a non-Eldrazi alone is refused, a block by the Eldrazi is accepted), a defender
//! without an Eldrazi blocks freely, the automatic declaration meets the requirement, and the log replays.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_engine::decision::DecisionKind;
use mtg_engine::replay;
use mtg_engine::turn::Step;
use mtg_testkit::cards::free_creature;
use mtg_testkit::corpus::card;
use mtg_testkit::scenario::attached_to;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

/// A 1/1 creature with the subtype.
fn r5502_of_kind(name: &str, subtype: &str) -> Arc<CardDefinition> {
    Arc::new((*free_creature(name, 1, 1)).clone().with_subtype(subtype))
}

/// Seat 0's Ox wears Slayer's Cleaver and attacks seat 1, which holds a Wolf and a creature of `kind` named Spawn.
fn r5502_driver(seats: usize, kind: &str) -> Result<Driver, ScriptError> {
    let scenario = Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .battlefield(
            0,
            [
                ready(free_creature("Ox", 2, 2)),
                attached_to(ready(card("Slayer's Cleaver").unwrap()), 0, 0),
            ],
        )
        .battlefield(
            1,
            [
                ready(free_creature("Wolf", 2, 2)),
                ready(r5502_of_kind("Spawn", kind)),
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
    Ok(driver)
}

#[rstest]
#[case::eldrazi_none("Eldrazi", &[], false)]
#[case::eldrazi_wolf_only("Eldrazi", &[("Wolf", "Ox")], false)]
#[case::eldrazi_blocks("Eldrazi", &[("Spawn", "Ox")], true)]
#[case::eldrazi_and_wolf("Eldrazi", &[("Spawn", "Ox"), ("Wolf", "Ox")], true)]
#[case::no_eldrazi_none("Goblin", &[], true)]
#[case::no_eldrazi_wolf("Goblin", &[("Wolf", "Ox")], true)]
fn r5502_the_equipped_creature_is_blocked_by_an_eldrazi(
    #[values(2, 3, 4)] seats: usize,
    #[case] kind: &str,
    #[case] blocks: &[(&str, &str)],
    #[case] legal: bool,
) -> Result<(), ScriptError> {
    let mut driver = r5502_driver(seats, kind)?;
    let ox = driver.id_of("Ox")?;
    let power = mtg_engine::characteristics::characteristics(driver.state(), ox)
        .unwrap()
        .power;
    assert_eq!(power, Some(5), "the Cleaver gives +3/+1");
    let DecisionKind::DeclareBlockers(options) = driver.decision()?.kind.clone() else {
        panic!("blockers are declared");
    };
    assert!(options.accepts(&options.legal_blocks()), "{kind}");
    assert_eq!(driver.block(blocks).is_ok(), legal, "{kind}: {blocks:?}");
    if legal {
        let state = replay(driver.engine().setup(), driver.engine().events()).unwrap();
        assert_eq!(state.state_hash(), driver.state().state_hash());
    }
    Ok(())
}
