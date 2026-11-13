//! Goal: the decision-quality suite builds every position with a decision for seat 0, has 60 cases or more over every
//! theme, judges a passive brain as failing its interaction cases, and the levels keep their floor scores (Normal and
//! Hard here; Very Hard is scored by `examples/bot_suite.rs`).
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_core::ids::PlayerId;
use mtg_engine::decision::{Answer, DecisionKind};
use mtg_engine::{Decision, Engine};
use mtg_testkit::brain::suite::{Theme, cases, run, score};
use mtg_testkit::brain::{Level, SeatBrain};
use mtg_testkit::driver::Driver;
use rstest::rstest;

/// A brain that never acts: it passes, attacks with nothing and blocks nothing.
struct Passive;

impl SeatBrain for Passive {
    fn choose(&mut self, decision: &Decision, _table: Option<&Engine>) -> Answer {
        Driver::passive_answer(decision)
    }
}

#[test]
fn every_case_builds_with_a_decision_for_seat_0() {
    let all = cases();
    assert!(all.len() >= 60, "{} cases", all.len());
    for case in &all {
        let table = (case.build)().unwrap_or_else(|e| panic!("{}: {e}", case.name));
        let decision = table.engine.decision().expect(case.name);
        assert_eq!(decision.player, PlayerId::from_index(0), "{}", case.name);
    }
    for theme in [
        Theme::Counter,
        Theme::Hold,
        Theme::Removal,
        Theme::Protect,
        Theme::Attack,
        Theme::Block,
        Theme::Wipe,
        Theme::Develop,
        Theme::Trick,
    ] {
        assert!(all.iter().any(|case| case.theme == theme), "{theme:?}");
    }
}

#[test]
fn a_brain_that_never_acts_fails_the_cases_that_need_action() {
    let mut failed = 0;
    for case in cases() {
        let verdict = run(&case, &mut Passive).unwrap();
        let table = (case.build)().unwrap();
        let acting_needed = matches!(
            table.engine.decision().map(|d| &d.kind),
            Some(DecisionKind::Priority(_))
        ) && case.name.starts_with("counter_the_bomb");
        if acting_needed {
            assert!(!verdict.passed, "{}", case.name);
        }
        if !verdict.passed {
            failed += 1;
        }
    }
    assert!(failed >= 25, "a passive brain failed only {failed}");
}

#[rstest]
#[case::normal(Level::Normal, 0.9)]
#[case::hard(Level::Hard, 0.9)]
fn each_level_keeps_its_floor(#[case] level: Level, #[case] floor: f64) {
    let verdicts = score(level, 7, false);
    let total = verdicts.len();
    let mut failed = Vec::new();
    for verdict in verdicts {
        let verdict = verdict.unwrap();
        if !verdict.passed {
            failed.push(verdict.case.name);
        }
    }
    #[allow(clippy::cast_precision_loss)]
    let share = (total - failed.len()) as f64 / total as f64;
    assert!(share >= floor, "{level:?} {share:.2}: failed {failed:?}");
}
