//! A target chosen in a sentence of its own and a group compared with it: "Choose target creature you control. Destroy
//! all creatures with power less than that creature's power."

use super::super::selectors::type_name;
use super::{
    Amount, Effect, Form, Pred, Ref, Selector, Targetable, Writer, phrase_text, stat_name,
};
use crate::value::Cmp;

/// "less than", "greater than": how the group's characteristic compares with the target's.
fn relation(op: Cmp) -> Option<&'static str> {
    Some(match op {
        Cmp::Lt => "less than",
        Cmp::Gt => "greater than",
        _ => return None,
    })
}

impl Writer<'_> {
    /// The two sentences that choose the target and destroy the group that compares with it, and the number of
    /// effects they cover.
    pub(super) fn chosen_then_group(&mut self, effects: &[Effect]) -> Option<(String, usize)> {
        let [
            Effect::Destroy {
                what: Ref::Each(Selector(preds)),
                regenerate: true,
            },
            ..,
        ] = effects
        else {
            return None;
        };
        let [spec] = self.targets else {
            return None;
        };
        let Targetable::Object(Selector(chosen)) = &spec.what else {
            return None;
        };
        let noun = chosen.iter().find_map(|pred| match pred {
            Pred::Type(card_type) => Some(type_name(*card_type)),
            _ => None,
        })?;
        let (at, stat, op) = preds.iter().enumerate().find_map(|(at, pred)| match pred {
            Pred::Compare {
                stat,
                op,
                to:
                    Amount::Stat {
                        of: Ref::Target(0),
                        stat: same,
                    },
            } if stat == same => Some((at, *stat, *op)),
            _ => None,
        })?;
        let mut group = preds.clone();
        group.remove(at);
        let (group, name) = (phrase_text(&Selector(group), Form::All)?, stat_name(stat)?);
        let target = self.name_target()?;
        Some((
            format!(
                "choose {target}. Destroy all {group} with {name} {} that {noun}'s {name}",
                relation(op)?
            ),
            1,
        ))
    }
}
