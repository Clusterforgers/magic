//! A choice between two steps on how the creature of an event compares with the card: "put a +1/+1 counter on that
//! creature if its power is less than ~'s power. Otherwise, put a +1/+1 counter on ~."

use super::{Effect, Writer, stat_name};
use crate::refs::Ref;
use crate::value::{Amount, Cmp, Condition};

/// "less than" for the comparison of the event's creature with the card.
fn relation(op: Cmp) -> Option<&'static str> {
    Some(match op {
        Cmp::Eq => "equal to",
        Cmp::Lt => "less than",
        Cmp::Le => "less than or equal to",
        Cmp::Gt => "greater than",
        Cmp::Ge => "greater than or equal to",
        Cmp::Ne => return None,
    })
}

/// "its power is less than ~'s power": a characteristic of the creature of the event against the same one of the card.
fn comparison(condition: &Condition) -> Option<String> {
    let Condition::Compare {
        left: Amount::Stat {
            of: Ref::Triggering,
            stat,
        },
        op,
        right: Amount::Stat {
            of: Ref::This,
            stat: other,
        },
    } = condition
    else {
        return None;
    };
    if stat != other {
        return None;
    }
    let name = stat_name(*stat)?;
    Some(format!("its {name} is {} ~'s {name}", relation(*op)?))
}

impl Writer<'_> {
    /// The two sentences of a step on a comparison and the step it takes otherwise, and the number of effects they cover.
    pub(super) fn either_way(&mut self, effects: &[Effect]) -> Option<(String, usize)> {
        let [
            Effect::If {
                condition,
                then,
                otherwise,
            },
            ..,
        ] = effects
        else {
            return None;
        };
        let ([chosen], [instead]) = (then.as_slice(), otherwise.as_slice()) else {
            return None;
        };
        let when = comparison(condition)?;
        let (first, second) = (self.effect(chosen)?, self.effect(instead)?);
        // The creature the condition is about is named again, not "it", where the sentence goes on to the comparison.
        let first = match first.strip_suffix(" on it") {
            Some(step) => format!("{step} on that creature"),
            None => first,
        };
        Some((format!("{first} if {when}. Otherwise, {second}"), 1))
    }
}
