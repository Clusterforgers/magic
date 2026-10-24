//! A move the card says another way when a land entered is a basic land type: "you may return target nonland permanent
//! card from your graveyard to your hand. If that land is a Plains, you may return that nonland permanent card to the
//! battlefield instead.", and an effect that another replaces when a condition holds: "Draw a card. If you control eight
//! or more lands, instead draw two cards."

use super::{
    Destination, Effect, Wording, Writer, conditional, exiled_kind, kicked_copies, spec_text,
    written,
};
use crate::refs::{Pred, Ref, TargetSpec};
use crate::value::Condition;

/// The basic land types a land that entered can be.
const LAND_TYPES: [&str; 5] = ["Plains", "Island", "Swamp", "Mountain", "Forest"];

/// The one card a `May` moves to `to`: "you may return target ... to your hand".
fn may_move(effects: &[Effect], to: &Destination) -> bool {
    matches!(
        effects,
        [Effect::May { who: Ref::You, effects }]
            if matches!(effects.as_slice(), [Effect::Move { what: Ref::Target(0), to: found }] if found == to)
    )
}

impl Writer<'_> {
    /// The sentence of a move to the hand that a basic land type changes to a move to the battlefield, and the
    /// number of effects they cover.
    pub(super) fn move_instead(&mut self, effects: &[Effect]) -> Option<(String, usize)> {
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
        let [Pred::Subtype(land)] = selector.0.as_slice() else {
            return None;
        };
        let battlefield = Destination::Battlefield {
            tapped: false,
            under: None,
            transformed: false,
            counters: Vec::new(),
        };
        if !LAND_TYPES.contains(&land.0.as_str())
            || !may_move(then, &battlefield)
            || !may_move(otherwise, &Destination::Hand)
        {
            return None;
        }
        let kind = spec_text(self.targets.first()?)?;
        let kind = kind
            .strip_prefix("target ")?
            .strip_suffix(" from your graveyard")?
            .to_owned();
        let first = self.effect(otherwise.first()?)?;
        Some((
            format!(
                "{first}. If that land is a {}, you may return that {kind} to the battlefield instead",
                land.0
            ),
            1,
        ))
    }
}

impl Writer<'_> {
    /// The exile of a target and the sentence that follows when the object was of a kind: "exile target card from a
    /// graveyard" and "if it was a creature card, put a +1/+1 counter on ~", and the number of effects they cover.
    pub(super) fn exile_kind(&mut self, effects: &[Effect]) -> Option<(Vec<String>, usize)> {
        let (plain, kind, then) = exiled_kind::is_exile_kind(effects)?;
        let Some(Effect::If { condition, .. }) = effects.get(1) else {
            return None;
        };
        let first = self.effect(&plain)?;
        let condition_text = exiled_kind::condition_text(kind)?;
        let second = self.consequence(condition, then)?;
        Some((vec![first, format!("{condition_text}, {second}")], 2))
    }
}

/// The one sentence the effects of a branch are written as.
fn one_sentence(targets: &[TargetSpec], effects: &[Effect], wording: Wording) -> Option<String> {
    let mut texts = written(targets, effects, 0, wording)?;
    (texts.len() == 1).then(|| texts.remove(0))
}

/// The two sentences of an `If` with a branch for each, "S1" for the `otherwise` and "if C, instead S2" for the `then`,
/// when the spell has no target, and the number of effects they cover.
pub(super) fn pair(
    targets: &[TargetSpec],
    effects: &[Effect],
    wording: Wording,
) -> Option<(Vec<String>, usize)> {
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
    if !targets.is_empty() || then.is_empty() || otherwise.is_empty() {
        return None;
    }
    // The other two readings of an `If` with both branches have sentences of their own.
    if crate::compile::read::day_night::effect_text(&effects[0]).is_some()
        || kicked_copies::is_kicked_copy(&effects[0]).is_some()
    {
        return None;
    }
    let texts = conditional::condition_texts(condition);
    let text = texts.get(wording.condition).or(texts.first())?;
    let first = one_sentence(targets, otherwise, wording)?;
    let second = one_sentence(targets, then, wording)?;
    Some((vec![first, format!("if {text}, instead {second}")], 1))
}
