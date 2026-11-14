//! Goal: "As long as equipped creature is attacking, it has first strike and must be blocked by a Dalek if able." plays
//! as printed at two, three and four seats: before it attacks the equipped creature has +3/+0 and no first strike; once
//! attacking it has first strike, a defender with a Dalek able to block must block it with one (no block or a non-Dalek
//! alone is refused, a Dalek is accepted), a defender without a Dalek blocks freely, and the log replays.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::keyword::Keyword;
use mtg_engine::decision::DecisionKind;
use mtg_engine::keywords::has_keyword;
use mtg_engine::replay;
use mtg_engine::turn::Step;
use mtg_testkit::cards::free_creature;
use mtg_testkit::corpus::card;
use mtg_testkit::scenario::attached_to;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

const BAT: &str = "Ace's Baseball Bat";

/// A 1/1 creature with the subtype.
fn r6001_of_kind(name: &str, subtype: &str) -> Arc<CardDefinition> {
    Arc::new((*free_creature(name, 1, 1)).clone().with_subtype(subtype))
}

#[rstest]
#[case::dalek_none("Dalek", &[], false)]
#[case::dalek_wolf_only("Dalek", &[("Wolf", "Ox")], false)]
#[case::dalek_blocks("Dalek", &[("Spawn", "Ox")], true)]
#[case::dalek_and_wolf("Dalek", &[("Spawn", "Ox"), ("Wolf", "Ox")], true)]
#[case::no_dalek_none("Goblin", &[], true)]
#[case::no_dalek_wolf("Goblin", &[("Wolf", "Ox")], true)]
fn r6001_the_attacking_bearer_has_first_strike_and_is_blocked_by_a_dalek(
    #[values(2, 3, 4)] seats: usize,
    #[case] kind: &str,
    #[case] blocks: &[(&str, &str)],
    #[case] legal: bool,
) -> Result<(), ScriptError> {
    let scenario = Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .battlefield(
            0,
            [
                ready(free_creature("Ox", 2, 2)),
                attached_to(ready(card(BAT).unwrap()), 0, 0),
            ],
        )
        .battlefield(
            1,
            [
                ready(free_creature("Wolf", 2, 2)),
                ready(r6001_of_kind("Spawn", kind)),
            ],
        );
    let mut driver = Driver::from_scenario(scenario)?;
    let ox = driver.id_of("Ox")?;
    let power = |driver: &Driver| {
        mtg_engine::characteristics::characteristics(driver.state(), ox)
            .unwrap()
            .power
    };
    assert_eq!(power(&driver), Some(5), "the Bat gives +3/+0");
    assert!(
        !has_keyword(driver.state(), ox, &Keyword::FirstStrike),
        "no first strike before it attacks"
    );
    driver.pass_until(Step::DeclareAttackers)?;
    driver.attack(&[("Ox", 1)])?;
    for _ in 0..40 {
        if matches!(driver.decision()?.kind, DecisionKind::DeclareBlockers(_)) {
            break;
        }
        driver.answer_passively()?;
    }
    assert!(
        has_keyword(driver.state(), ox, &Keyword::FirstStrike),
        "first strike while attacking"
    );
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
