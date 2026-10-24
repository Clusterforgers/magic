//! An exile the player may undo at once, or else at the next end step with counters: "Exile another target creature you
//! control. You may return that card to the battlefield under its owner's control. If you don't, at the beginning of
//! the next end step, return that card to the battlefield under its owner's control with a vigilance counter and a
//! lifelink counter on it."

use super::{Destination, Effect, Ref, Writer};
use crate::compile::read::counters;
use crate::trigger::{Step, TriggerPattern};
use crate::value::{Amount, Condition};

impl Writer<'_> {
    /// The exile of a target, the optional return at once and the delayed return with counters, as three sentences, and
    /// the number of effects they cover.
    pub(super) fn exile_and_maybe_return(
        &mut self,
        effects: &[Effect],
    ) -> Option<(Vec<String>, usize)> {
        let [
            Effect::Exile {
                what: what @ Ref::Target(_),
                bind: Some(exiled),
                duration: None,
            },
            Effect::May {
                who: Ref::You,
                effects: now,
            },
            Effect::If {
                condition: Condition::Not(missed),
                then,
                otherwise,
            },
            ..,
        ] = effects
        else {
            return None;
        };
        (otherwise.is_empty() && **missed == Condition::Performed).then_some(())?;
        returned(now, exiled, false)?;
        let [
            Effect::DelayedTrigger {
                when:
                    TriggerPattern::StepBegins {
                        step: Step::End,
                        whose: Ref::EachPlayer,
                    },
                once: true,
                effects: later,
            },
        ] = then.as_slice()
        else {
            return None;
        };
        let with = returned(later, exiled, true)?;
        Some((
            vec![
                format!("exile {}", self.object(what)?),
                "you may return that card to the battlefield under its owner's control".to_owned(),
                format!(
                    "if you don't, at the beginning of the next end step, return that card to the battlefield under its owner's control with {with} on it"
                ),
            ],
            3,
        ))
    }
}

/// The move of the exiled card to the battlefield under its owner's control, and the counters it enters with as
/// "a vigilance counter and a lifelink counter" (none when `counted` is false).
fn returned(effects: &[Effect], exiled: &str, counted: bool) -> Option<String> {
    let [
        Effect::Move {
            what: Ref::Bound(moved),
            to:
                Destination::Battlefield {
                    tapped: false,
                    under: None,
                    transformed: false,
                    counters: entering,
                },
        },
    ] = effects
    else {
        return None;
    };
    (moved == exiled && entering.is_empty() != counted).then_some(())?;
    let texts: Option<Vec<String>> = entering
        .iter()
        .map(|(kind, number)| counters::text(kind, &Amount::Fixed(i32::try_from(*number).ok()?)))
        .collect();
    Some(texts?.join(" and "))
}
