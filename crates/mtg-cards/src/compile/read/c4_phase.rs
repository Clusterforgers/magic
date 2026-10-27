//! The phase of the turn in a condition: "it's the first combat phase of the turn" (`Condition::FirstCombat`), "... of
//! your turn" (also on your turn), "it's your main phase" and "it's your combat phase" (on your turn, in that phase).

use super::tokens::Reader;
use crate::trigger::Step;
use crate::value::Condition;

/// A main phase: the precombat or the postcombat one.
fn main_phase() -> Condition {
    Condition::Any(vec![
        Condition::Step(Step::Main1),
        Condition::Step(Step::Main2),
    ])
}

/// On your turn, in the phase.
fn yours(phase: Condition) -> Condition {
    Condition::All(vec![Condition::YourTurn, phase])
}

/// "it's the first combat phase of the turn", "it's the first combat phase of your turn", "it's your main phase" and
/// "it's your combat phase", after "if".
pub fn read(r: &mut Reader<'_>) -> Option<Condition> {
    r.need("it's")?;
    if r.words("the first combat phase of") {
        return if r.words("your turn") {
            Some(yours(Condition::FirstCombat))
        } else {
            r.words("the turn").then_some(Condition::FirstCombat)
        };
    }
    r.need("your")?;
    if r.words("main phase") {
        Some(yours(main_phase()))
    } else {
        r.words("combat phase").then(|| yours(Condition::InCombat))
    }
}

/// The condition `read` reads, written after "if".
pub fn text(condition: &Condition) -> Option<String> {
    let phase = match condition {
        Condition::FirstCombat => {
            return Some("it's the first combat phase of the turn".to_owned());
        }
        Condition::All(held) => match held.as_slice() {
            [Condition::YourTurn, phase] => phase,
            _ => return None,
        },
        _ => return None,
    };
    let words = match phase {
        Condition::FirstCombat => "the first combat phase of your turn",
        Condition::InCombat => "your combat phase",
        other if *other == main_phase() => "your main phase",
        _ => return None,
    };
    Some(format!("it's {words}"))
}
