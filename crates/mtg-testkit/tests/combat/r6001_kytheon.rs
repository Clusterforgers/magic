//! Goal: Kytheon transforms at end of combat only if it and at least two other creatures attacked this combat, at two to
//! four seats: with one other attacker it stays, with two it is exiled and returns as Gideon, Battle-Forged under its
//! owner's control, and an attacker that died in combat still counts (the Kytheon ruling); the log replays to the state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_engine::decision::DecisionKind;
use mtg_engine::replay;
use mtg_engine::turn::Step;
use mtg_testkit::cards::free_creature;
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

use crate::common::pid;

const KYTHEON: &str = "Kytheon, Hero of Akros";
const GIDEON: &str = "Gideon, Battle-Forged";

#[rstest]
#[case::one_other_stays(1, false, false)]
#[case::two_others_transform(2, false, true)]
#[case::three_others_transform(3, false, true)]
#[case::a_dead_attacker_counts(2, true, true)]
fn r6001_kytheon_transforms_after_attacking_with_two_others(
    #[values(2, 3, 4)] seats: usize,
    #[case] others: usize,
    #[case] one_dies: bool,
    #[case] transforms: bool,
) -> Result<(), ScriptError> {
    let mut mine = vec![ready(card(KYTHEON).unwrap())];
    mine.extend((0..others).map(|n| ready(free_creature(&format!("Ally {n}"), 1, 1))));
    let scenario = Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .battlefield(0, mine)
        .battlefield(1, [ready(free_creature("Ogre", 3, 3))]);
    let setup = scenario.clone().setup().unwrap();
    let mut driver = Driver::from_scenario(scenario)?;
    driver.pass_until(Step::DeclareAttackers)?;
    let mut attackers = vec![(KYTHEON.to_owned(), 1)];
    attackers.extend((0..others).map(|n| (format!("Ally {n}"), 1)));
    let named: Vec<(&str, usize)> = attackers
        .iter()
        .map(|(name, seat)| (name.as_str(), *seat))
        .collect();
    driver.attack(&named)?;
    for _ in 0..40 {
        if matches!(driver.decision()?.kind, DecisionKind::DeclareBlockers(_)) {
            break;
        }
        driver.answer_passively()?;
    }
    if one_dies {
        // The 3/3 Ogre blocks and kills a 1/1 ally before end of combat.
        driver.block(&[("Ogre", "Ally 0")])?;
    } else {
        driver.block(&[])?;
    }
    driver.pass_until(Step::Main2)?;
    for _ in 0..40 {
        if driver.state().stack().is_empty() && driver.state().pending_triggers().is_empty() {
            break;
        }
        driver.answer_passively()?;
    }
    if one_dies {
        assert!(
            driver.id_of("Ally 0").is_err(),
            "the blocked ally died in combat"
        );
    }
    assert_eq!(driver.id_of(GIDEON).is_ok(), transforms);
    assert_eq!(driver.id_of(KYTHEON).is_ok(), !transforms);
    if transforms {
        let gideon = driver.id_of(GIDEON)?;
        assert_eq!(driver.state().object(gideon).unwrap().controller(), pid(0));
    }
    let replayed = replay(&setup, driver.engine().events()).unwrap();
    assert_eq!(replayed.state_hash(), driver.state().state_hash());
    Ok(())
}
