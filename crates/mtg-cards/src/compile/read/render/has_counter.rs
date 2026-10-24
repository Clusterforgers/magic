//! A choice between two steps on whether the creature of an event has a counter: "draw a card if that creature has a
//! +1/+1 counter on it. If it doesn't, put a +1/+1 counter on it."

use super::{Amount, Condition, Effect, Pred, Ref, Writer, counters};

impl Writer<'_> {
    /// The two sentences of a step on a counter of the creature of the event and the step taken otherwise, and the
    /// number of effects they cover.
    pub(super) fn counter_or_not(&mut self, effects: &[Effect]) -> Option<(String, usize)> {
        let [
            Effect::If {
                condition:
                    Condition::Matches {
                        subject: Ref::Triggering,
                        selector,
                    },
                then,
                otherwise,
            },
            ..,
        ] = effects
        else {
            return None;
        };
        let [Pred::HasCounter(kind)] = selector.0.as_slice() else {
            return None;
        };
        let ([chosen], [instead]) = (then.as_slice(), otherwise.as_slice()) else {
            return None;
        };
        let counter = counters::text(kind, &Amount::one())?;
        let (first, second) = (self.effect(chosen)?, self.effect(instead)?);
        Some((
            format!("{first} if that creature has {counter} on it. If it doesn't, {second}"),
            1,
        ))
    }
}
