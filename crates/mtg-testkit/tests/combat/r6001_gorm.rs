//! Goal: "Gorm must be blocked if able, and Gorm must be blocked by two or more creatures if able." plays as printed at
//! two, three and four seats: with two or more creatures able to block, no block or a single blocker is refused and two
//! are accepted; with one, that creature must block alone; the automatic declaration meets the requirement, and the log
//! replays.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_engine::decision::DecisionKind;
use mtg_engine::replay;
use mtg_engine::turn::Step;
use mtg_testkit::cards::free_creature;
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

const GORM: &str = "Gorm the Great";

#[rstest]
#[case::two_able_none(2, &[], false)]
#[case::two_able_one(2, &[("Wolf 0", GORM)], false)]
#[case::two_able_two(2, &[("Wolf 0", GORM), ("Wolf 1", GORM)], true)]
#[case::three_able_two(3, &[("Wolf 1", GORM), ("Wolf 2", GORM)], true)]
#[case::three_able_three(3, &[("Wolf 0", GORM), ("Wolf 1", GORM), ("Wolf 2", GORM)], true)]
#[case::one_able_none(1, &[], false)]
#[case::one_able_one(1, &[("Wolf 0", GORM)], true)]
fn r6001_gorm_is_blocked_by_two_or_more_if_able(
    #[values(2, 3, 4)] seats: usize,
    #[case] wolves: usize,
    #[case] blocks: &[(&str, &str)],
    #[case] legal: bool,
) -> Result<(), ScriptError> {
    let scenario = Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .battlefield(0, [ready(card(GORM).unwrap())])
        .battlefield(
            1,
            (0..wolves).map(|n| ready(free_creature(&format!("Wolf {n}"), 1, 1))),
        );
    let mut driver = Driver::from_scenario(scenario)?;
    driver.pass_until(Step::DeclareAttackers)?;
    driver.attack(&[(GORM, 1)])?;
    for _ in 0..40 {
        if matches!(driver.decision()?.kind, DecisionKind::DeclareBlockers(_)) {
            break;
        }
        driver.answer_passively()?;
    }
    let DecisionKind::DeclareBlockers(options) = driver.decision()?.kind.clone() else {
        panic!("blockers are declared");
    };
    let automatic = options.legal_blocks();
    assert!(options.accepts(&automatic));
    assert_eq!(
        automatic.len(),
        wolves.min(2),
        "the automatic declaration blocks with as many as it must"
    );
    assert_eq!(driver.block(blocks).is_ok(), legal, "{blocks:?}");
    if legal {
        let state = replay(driver.engine().setup(), driver.engine().events()).unwrap();
        assert_eq!(state.state_hash(), driver.state().state_hash());
    }
    Ok(())
}
