//! An exile whose card comes back at a later step: "Exile another target creature. Return that card to the battlefield
//! under its owner's control at the beginning of the next end step."

use super::{Destination, Effect, Ref, Writer};
use crate::trigger::{Step, TriggerPattern};

impl Writer<'_> {
    /// The exile of a target, bound as `exiled`, and the delayed trigger that returns it at the next end step, as two
    /// sentences, and the number of effects they cover.
    pub(super) fn exile_and_return_later(
        &mut self,
        effects: &[Effect],
    ) -> Option<(Vec<String>, usize)> {
        let [
            Effect::Exile {
                what: what @ Ref::Target(_),
                bind: Some(exiled),
                duration: None,
            },
            Effect::DelayedTrigger {
                when:
                    TriggerPattern::StepBegins {
                        step: Step::End,
                        whose: Ref::EachPlayer,
                    },
                once: true,
                effects: later,
            },
            ..,
        ] = effects
        else {
            return None;
        };
        let [
            Effect::Move {
                what: Ref::Bound(moved),
                to:
                    Destination::Battlefield {
                        tapped,
                        under,
                        transformed: false,
                        counters,
                    },
            },
        ] = later.as_slice()
        else {
            return None;
        };
        if moved != exiled || !counters.is_empty() {
            return None;
        }
        let control = match under {
            None => "its owner's",
            Some(Ref::You) => "your",
            Some(_) => return None,
        };
        let tapped = if *tapped { " tapped" } else { "" };
        // Any number of targets come back as "those cards" under "their owner's" control.
        let (cards, control) = if self.single(what) || self.at_most_one(what) {
            ("that card", control.to_owned())
        } else {
            ("those cards", control.replace("its", "their"))
        };
        Some((
            vec![
                format!("exile {}", self.object(what)?),
                format!(
                    "return {cards} to the battlefield{tapped} under {control} control at the beginning of the next end step"
                ),
            ],
            2,
        ))
    }
}
