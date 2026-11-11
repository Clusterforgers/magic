//! Goal: a rule that makes a player skip their next step or combat phase (`Duration::UntilSkipped`) skips exactly the next
//! one of that player's turns and is then used up, at tables of two to four seats: the next untap step leaves a tapped
//! permanent tapped and the one after untaps it, the next draw step draws nothing and the one after draws, another
//! player's next combat phase is skipped and their combat after it is not, and "this turn" skips no later turn's combat.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::collections::BTreeSet;
use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_engine::choose::{Target, TargetChoice};
use mtg_engine::decision::{Answer, DecisionKind};
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::free_creature;
use mtg_testkit::trigger_cards::artifact_with;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

/// An artifact with one activated ability that costs `{T}` and targets a player when `aims`.
fn rig(name: &str, effects: &str, aims: bool) -> Arc<CardDefinition> {
    let targets = if aims { r#"[{"what":"Player"}]"# } else { "[]" };
    let ability: Ability = serde_json::from_str(&format!(
        r#"{{"Activated":{{"cost":["Tap"],"effects":{effects},"targets":{targets}}}}}"#
    ))
    .unwrap();
    artifact_with(name, vec![ability])
}

/// The rule that `who` skips `action` for `duration`.
fn skip(who: &str, action: &str, duration: &str) -> String {
    format!(
        r#"[{{"Rule":{{"rule":{{"Cant":{{"who":{who},"action":{action}}}}},"duration":"{duration}"}}}}]"#
    )
}

/// A table of `seats` whose libraries hold ten cards each, with `mine` on seat 0's battlefield.
fn table(seats: usize, mine: Vec<Arc<CardDefinition>>) -> Result<Driver, ScriptError> {
    let mut scenario =
        Scenario::new(seats).battlefield(0, mine.into_iter().map(ready).collect::<Vec<_>>());
    for seat in 0..seats {
        scenario = scenario.library(seat, (0..10).map(|_| free_creature("Card", 1, 1)));
    }
    Driver::from_scenario(scenario)
}

/// Activates `name`, aims at `aim` when asked, and answers until the stack is empty.
fn run(driver: &mut Driver, name: &str, aim: Option<Target>) -> Result<(), ScriptError> {
    driver.begin_activation(name)?;
    for _ in 0..40 {
        let decision = driver.decision()?.clone();
        if matches!(decision.kind, DecisionKind::Priority(_))
            && driver.state().stack().is_empty()
            && driver.state().pending_triggers().is_empty()
        {
            return Ok(());
        }
        let answer = match (&decision.kind, aim) {
            (DecisionKind::ChooseTargets(_), Some(aim)) => {
                Answer::ChooseTargets(TargetChoice::Targets(vec![aim]))
            }
            _ => Driver::passive_answer(&decision),
        };
        driver.answer(answer)?;
    }
    Err(ScriptError::Stuck("a finished resolution".to_owned()))
}

/// Answers passively until turn `turn` reaches `step`, and gives the steps each turn began on the way.
fn advance(
    driver: &mut Driver,
    turn: u32,
    step: Step,
    seen: &mut BTreeSet<(u32, Step)>,
) -> Result<(), ScriptError> {
    for _ in 0..2000 {
        let now = driver.state().turn();
        seen.insert((now.number(), now.step()));
        if now.number() > turn || (now.number() == turn && now.step() >= step) {
            return Ok(());
        }
        let decision = driver.decision()?.clone();
        driver.answer(Driver::passive_answer(&decision))?;
    }
    Err(ScriptError::Stuck("the step".to_owned()))
}

/// Whether the battlefield permanent `name` is tapped.
fn tapped(driver: &Driver, name: &str) -> bool {
    let id = driver.id_of(name).unwrap();
    driver
        .state()
        .object(id)
        .and_then(|o| o.permanent())
        .is_some_and(|p| p.tapped())
}

#[rstest]
fn your_next_untap_step_is_skipped_once(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let rule = skip(r#""You""#, r#"{"SkipStep":"Untap"}"#, "UntilSkipped");
    let mut driver = table(seats, vec![rig("Halt", &rule, false)])?;
    run(&mut driver, "Halt", None)?;
    let round = u32::try_from(seats).unwrap();
    let mut seen = BTreeSet::new();
    advance(&mut driver, 1 + round, Step::Upkeep, &mut seen)?;
    assert!(tapped(&driver, "Halt"), "the next untap step is skipped");
    advance(&mut driver, 1 + 2 * round, Step::Upkeep, &mut seen)?;
    assert!(!tapped(&driver, "Halt"), "the one after untaps");
    Ok(())
}

#[rstest]
fn a_target_players_next_draw_step_is_skipped_once(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let rule = skip(r#"{"Target":0}"#, r#"{"SkipStep":"Draw"}"#, "UntilSkipped");
    let mut driver = table(seats, vec![rig("Tire", &rule, true)])?;
    let other = seats - 1;
    let aim = driver.target_player(other);
    run(&mut driver, "Tire", Some(aim))?;
    let hand = |driver: &Driver| {
        driver
            .state()
            .zone(Zone::Hand(mtg_core::ids::PlayerId::from_index(other)))
            .len()
    };
    let turn = 1 + u32::try_from(other).unwrap();
    let round = u32::try_from(seats).unwrap();
    let mut seen = BTreeSet::new();
    advance(&mut driver, turn, Step::Main1, &mut seen)?;
    let after_skip = hand(&driver);
    advance(&mut driver, turn + round, Step::Main1, &mut seen)?;
    assert!(!seen.contains(&(turn, Step::Draw)));
    assert!(seen.contains(&(turn + round, Step::Draw)));
    assert_eq!(hand(&driver), after_skip + 1);
    Ok(())
}

#[rstest]
fn another_players_next_combat_phase_is_skipped_once(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let rule = skip(r#"{"Target":0}"#, r#""SkipCombat""#, "UntilSkipped");
    let mut driver = table(seats, vec![rig("Calm", &rule, true)])?;
    let other = seats - 1;
    let aim = driver.target_player(other);
    run(&mut driver, "Calm", Some(aim))?;
    let turn = 1 + u32::try_from(other).unwrap();
    let round = u32::try_from(seats).unwrap();
    let mut seen = BTreeSet::new();
    advance(&mut driver, turn + round, Step::End, &mut seen)?;
    let combat = |t: u32| {
        seen.iter()
            .any(|(n, s)| *n == t && matches!(s, Step::BeginCombat | Step::DeclareAttackers))
    };
    assert!(combat(1), "my own combat stays");
    assert!(!combat(turn), "their next combat is skipped");
    assert!(combat(turn + round), "the one after stays");
    Ok(())
}

#[rstest]
fn a_combat_skipped_this_turn_leaves_later_turns_alone(
    #[values(2, 3, 4)] seats: usize,
    #[values(true, false)] this_turn: bool,
) -> Result<(), ScriptError> {
    let duration = if this_turn {
        "UntilSkippedThisTurn"
    } else {
        "UntilSkipped"
    };
    let rule = skip(r#"{"Target":0}"#, r#""SkipCombat""#, duration);
    let mut driver = table(seats, vec![rig("Hush", &rule, true)])?;
    let other = seats - 1;
    let aim = driver.target_player(other);
    run(&mut driver, "Hush", Some(aim))?;
    let turn = 1 + u32::try_from(other).unwrap();
    let mut seen = BTreeSet::new();
    advance(&mut driver, turn, Step::End, &mut seen)?;
    let combat = seen
        .iter()
        .any(|(n, s)| *n == turn && matches!(s, Step::BeginCombat | Step::DeclareAttackers));
    assert_eq!(
        combat, this_turn,
        "a rule for this turn ends before their turn"
    );
    Ok(())
}
