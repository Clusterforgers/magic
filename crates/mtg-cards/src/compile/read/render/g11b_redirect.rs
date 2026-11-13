//! g11b: shields that send all of the damage elsewhere this turn: "All damage that would be dealt to ~ this turn is dealt
//! to target creature instead.", "The next time damage would be dealt to ~ this turn, that damage is dealt to any target
//! instead." and, with a source chosen before it, "The next time a source of your choice would deal damage to you this
//! turn, that damage is dealt to target creature you control instead."

use super::r9w_shields::chosen_source;
use super::{Effect, Ref, Writer};
use crate::modify::Duration;
use crate::refs::Pred;
use crate::trigger::{ReplaceBy, ReplaceWhen};

impl Writer<'_> {
    /// A redirection shield for the turn, with the choice of its source before it, as one sentence, and the number of
    /// effects it covers.
    pub(super) fn g11b_redirect(&mut self, effects: &[Effect]) -> Option<(Vec<String>, usize)> {
        match effects {
            [
                Effect::Choose {
                    who: Ref::You,
                    from,
                    count,
                    bind,
                    rest_as: None,
                    differ: None,
                },
                Effect::Shield {
                    when:
                        ReplaceWhen::Damage {
                            to: Some(dealt),
                            source,
                            combat: None,
                        },
                    by: ReplaceBy::Redirect { to, next: true },
                    duration: Duration::UntilEndOfTurn,
                },
                ..,
            ] if count.is_one() && source.0 == [Pred::Is(Ref::Bound(bind.clone()))] => {
                let chosen = chosen_source(from)?;
                let dealt = self.r9w_one(dealt)?;
                let to = self.r9w_one(to)?;
                Some((
                    vec![format!(
                        "the next time {chosen} would deal damage to {dealt} this turn, that damage is dealt to {to} instead"
                    )],
                    2,
                ))
            }
            [
                Effect::Shield {
                    when:
                        ReplaceWhen::Damage {
                            to: Some(dealt),
                            source,
                            combat: None,
                        },
                    by: ReplaceBy::Redirect { to, next },
                    duration: Duration::UntilEndOfTurn,
                },
                ..,
            ] if source.0.is_empty() => {
                let dealt = self.r9w_one(dealt)?;
                let to = self.r9w_one(to)?;
                let text = if *next {
                    format!(
                        "the next time damage would be dealt to {dealt} this turn, that damage is dealt to {to} instead"
                    )
                } else {
                    format!(
                        "all damage that would be dealt to {dealt} this turn is dealt to {to} instead"
                    )
                };
                Some((vec![text], 1))
            }
            _ => None,
        }
    }
}
