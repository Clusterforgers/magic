//! Goal: a shield over the creatures that enter ("this turn, each creature you control enters with an additional +1/+1
//! counter on it") is judged as each creature enters, at tables of two to four seats: a creature cast or returned under
//! its maker's control after the shield began gets the counter, one already there and one entering under another
//! player's control do not; and a creature spell named by `SpellEntersWith` enters with its counters while the next one
//! does not.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_cards::value::CounterKind;
use mtg_engine::choose::{Target, TargetChoice};
use mtg_engine::decision::{Answer, DecisionKind};
use mtg_testkit::cards::free_creature;
use mtg_testkit::trigger_cards::artifact_with;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

/// An artifact with one activated ability that costs `{T}`: `effects` and `targets` as the card DSL writes them.
fn rig(name: &str, effects: &str, targets: &str) -> Arc<CardDefinition> {
    let ability: Ability = serde_json::from_str(&format!(
        r#"{{"Activated":{{"cost":["Tap"],"effects":{effects},"targets":{targets}}}}}"#
    ))
    .unwrap();
    artifact_with(name, vec![ability])
}

/// Activates `name`, aims at `aim` when asked, and passes until the stack is empty.
fn run(driver: &mut Driver, name: &str, aim: Option<Target>) -> Result<(), ScriptError> {
    driver.begin_activation(name)?;
    settle(driver, aim)
}

/// Answers until the stack is empty, aiming at `aim` when a target is asked.
fn settle(driver: &mut Driver, aim: Option<Target>) -> Result<(), ScriptError> {
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

const THIS_TURN: &str = r#"[{"Shield":{"when":{"Enters":{"what":[{"Type":"Creature"},{"ControlledBy":"You"}]}},"by":{"WithCounters":{"counter":"+1/+1","count":1}},"duration":"UntilEndOfTurn"}}]"#;
const TWO_THIS_TURN: &str = r#"[{"Shield":{"when":{"Enters":{"what":[{"Type":"Creature"},{"ControlledBy":"You"}]}},"by":{"WithCounters":{"counter":"+1/+1","count":2}},"duration":"UntilEndOfTurn"}}]"#;
const RETURN: &str = r#"[{"Move":{"what":{"Target":0},"to":{"Battlefield":{}}}}]"#;
const CARD_IN_GRAVEYARD: &str = r#"[{"what":{"Object":[{"Type":"Creature"},{"In":"Graveyard"}]}}]"#;

/// How the creature that is looked at came to the battlefield.
#[derive(Debug, Clone, Copy)]
enum Arrival {
    /// Yours before the shield began.
    Before,
    /// Cast by you after the shield began.
    Cast,
    /// Returned from your graveyard under your control after the shield began.
    Returned,
    /// Returned from another player's graveyard under that player's control.
    Theirs,
}

/// The +1/+1 counters on the battlefield permanent `name`.
fn counters(driver: &Driver, name: &str) -> u32 {
    let id = driver.id_of(name).unwrap();
    driver
        .state()
        .object(id)
        .and_then(|o| o.permanent())
        .map_or(0, |p| p.counters_of(&CounterKind::PlusOnePlusOne))
}

#[rstest]
fn a_shield_over_entering_creatures_you_control_counts_those_that_enter_later(
    #[values(2, 3, 4)] seats: usize,
    #[values(Arrival::Before, Arrival::Cast, Arrival::Returned, Arrival::Theirs)] arrival: Arrival,
    #[values(1, 2)] each: u32,
) -> Result<(), ScriptError> {
    let other = seats - 1;
    let shield = if each == 1 { THIS_TURN } else { TWO_THIS_TURN };
    let mut mine = vec![
        ready(rig("Brew", shield, "[]")),
        ready(rig("Raise", RETURN, CARD_IN_GRAVEYARD)),
    ];
    if matches!(arrival, Arrival::Before) {
        mine.push(ready(free_creature("Newt", 1, 1)));
    }
    let scenario = Scenario::new(seats).battlefield(0, mine);
    let scenario = match arrival {
        Arrival::Before => scenario,
        Arrival::Cast => scenario.hand(0, vec![free_creature("Newt", 1, 1)]),
        Arrival::Returned => scenario.graveyard(0, vec![free_creature("Newt", 1, 1)]),
        Arrival::Theirs => scenario.graveyard(other, vec![free_creature("Newt", 1, 1)]),
    };
    let mut driver = Driver::from_scenario(scenario)?;
    run(&mut driver, "Brew", None)?;
    match arrival {
        Arrival::Before => {}
        Arrival::Cast => {
            driver.cast("Newt")?;
            settle(&mut driver, None)?;
        }
        Arrival::Returned | Arrival::Theirs => {
            let newt = driver
                .state()
                .objects()
                .find(|o| o.card().name == "Newt")
                .map(|o| Target::Object(o.reference()))
                .unwrap();
            run(&mut driver, "Raise", Some(newt))?;
        }
    }
    let gets = if matches!(arrival, Arrival::Cast | Arrival::Returned) {
        each
    } else {
        0
    };
    assert_eq!(counters(&driver, "Newt"), gets, "{arrival:?}");
    Ok(())
}

const NEXT_CREATURE: &str = r#"[{"DelayedTrigger":{"when":{"SpellCast":{"what":[{"Type":"Creature"}],"by":"You"}},"this_turn":true,"effects":[{"SpellEntersWith":{"spell":"Triggering","count":COUNT}}]}}]"#;

/// The `SpellEntersWith` step after a cast: the spell that set off "when you next cast a creature spell this turn" enters
/// with the counters, the creature cast after it does not, and with a standing "whenever you cast a creature spell" each
/// does.
#[rstest]
fn the_spell_that_was_cast_enters_with_the_counters(
    #[values(2, 3, 4)] seats: usize,
    #[values(1, 3)] count: u32,
    #[values(false, true)] standing: bool,
) -> Result<(), ScriptError> {
    let effects = NEXT_CREATURE.replace("COUNT", &count.to_string());
    let mut mine = Vec::new();
    if standing {
        let ability: Ability = serde_json::from_str(&format!(
            r#"{{"Triggered":{{"when":{{"SpellCast":{{"what":[{{"Type":"Creature"}}],"by":"You"}}}},"effects":[{{"SpellEntersWith":{{"spell":"Triggering","count":{count}}}}}]}}}}"#
        ))
        .unwrap();
        mine.push(ready(artifact_with("Den", vec![ability])));
    } else {
        mine.push(ready(rig("Howl", &effects, "[]")));
    }
    let scenario = Scenario::new(seats).battlefield(0, mine).hand(
        0,
        vec![free_creature("First", 1, 1), free_creature("Second", 1, 1)],
    );
    let mut driver = Driver::from_scenario(scenario)?;
    if !standing {
        run(&mut driver, "Howl", None)?;
    }
    for name in ["First", "Second"] {
        driver.cast(name)?;
        settle(&mut driver, None)?;
    }
    let second = if standing { count } else { 0 };
    assert_eq!(
        (counters(&driver, "First"), counters(&driver, "Second")),
        (count, second)
    );
    Ok(())
}
