//! g11b: damage that can't be prevented ("~ deals 5 damage to target creature. The damage can't be prevented."), and
//! the same damage after "Damage can't be prevented this turn.", which says it already.

use super::Writer;
use crate::effect::Effect;
use crate::modify::{Duration, GameRule};

impl Writer<'_> {
    /// "<the damage>. The damage can't be prevented" for damage that can't be prevented; none for any other effect.
    pub(super) fn g11b_unpreventable(&mut self, effect: &Effect) -> Option<Option<String>> {
        let Effect::DealDamage {
            unpreventable: true,
            ..
        } = effect
        else {
            return Some(None);
        };
        let mut plain = effect.clone();
        if let Effect::DealDamage { unpreventable, .. } = &mut plain {
            *unpreventable = false;
        }
        let dealt = self.effect(&plain)?;
        Some(Some(format!("{dealt}. The damage can't be prevented")))
    }
}

/// The effects with "the damage can't be prevented" dropped from the damage after "Damage can't be prevented this turn.";
/// none when nothing is dropped. Damage at the top level and in the branches of an `If` counts.
pub(super) fn implied(effects: &[Effect]) -> Option<Vec<Effect>> {
    fn clear(effect: &mut Effect) -> bool {
        match effect {
            Effect::DealDamage { unpreventable, .. } => std::mem::take(unpreventable),
            Effect::If {
                then, otherwise, ..
            } => then
                .iter_mut()
                .chain(otherwise.iter_mut())
                .fold(false, |cleared, effect| clear(effect) | cleared),
            _ => false,
        }
    }
    let rule = effects.iter().position(|effect| {
        matches!(
            effect,
            Effect::Rule {
                rule: GameRule::DamageCantBePrevented {
                    source,
                    combat: None,
                },
                duration: Duration::UntilEndOfTurn,
            } if source.0.is_empty()
        )
    })?;
    let mut implied = effects.to_vec();
    let cleared = implied[rule + 1..]
        .iter_mut()
        .fold(false, |cleared, effect| clear(effect) | cleared);
    cleared.then_some(implied)
}
