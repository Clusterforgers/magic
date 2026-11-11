//! rmt2: a choice between destroying all of one kind or all of another, the second of which can't be regenerated
//! ("Destroy all lands or all creatures. Creatures destroyed this way can't be regenerated"); a target player who can't
//! play lands this turn; all damage a target spell would deal this turn dealt to its controller instead; and a target
//! spell that can't be countered.

use super::Writer;
use crate::effect::Effect;
use crate::modify::{Duration, GameRule, Prohibited};
use crate::refs::{Pred, Ref, Targetable};
use crate::trigger::{ReplaceBy, ReplaceWhen};

/// The plural name of a selector of one card type ("lands").
fn kind(what: &Ref) -> Option<String> {
    let Ref::Each(selector) = what else {
        return None;
    };
    let [Pred::Type(kind)] = selector.0.as_slice() else {
        return None;
    };
    Some(format!("{kind:?}s").to_lowercase())
}

impl Writer<'_> {
    /// "destroy all lands or all creatures. Creatures destroyed this way can't be regenerated", and the one effect it
    /// takes.
    pub(super) fn rmt2_destroy_either(effects: &[Effect]) -> Option<(String, usize)> {
        let [Effect::Either { first, second }, ..] = effects else {
            return None;
        };
        let (
            [
                Effect::Destroy {
                    what: one,
                    regenerate: true,
                    bind: None,
                },
            ],
            [
                Effect::Destroy {
                    what: other,
                    regenerate: false,
                    bind: None,
                },
            ],
        ) = (first.as_slice(), second.as_slice())
        else {
            return None;
        };
        let (one, other) = (kind(one)?, kind(other)?);
        let mut capital = other.clone();
        if let Some(first) = capital.get_mut(..1) {
            first.make_ascii_uppercase();
        }
        Some((
            format!(
                "destroy all {one} or all {other}. {capital} destroyed this way can't be regenerated"
            ),
            1,
        ))
    }

    /// "target player can't play lands this turn".
    pub(super) fn rmt2_no_lands(&mut self, effect: &Effect) -> Option<String> {
        let Effect::Rule {
            rule:
                GameRule::Cant {
                    who: Ref::Target(0),
                    action: Prohibited::PlayLand,
                },
            duration: Duration::UntilEndOfTurn,
        } = effect
        else {
            return None;
        };
        if !self.target_is(|what| matches!(what, Targetable::Player | Targetable::Opponent))
            || !self.single(&Ref::Target(0))
        {
            return None;
        }
        let player = self.name_target()?;
        Some(format!("{player} can't play lands this turn"))
    }

    /// "all damage that would be dealt this turn by target sorcery spell is dealt to that spell's controller instead".
    pub(super) fn rmt2_reverberation(&mut self, effect: &Effect) -> Option<String> {
        let Effect::Shield {
            when:
                ReplaceWhen::Damage {
                    to: None,
                    source,
                    combat: None,
                },
            by:
                ReplaceBy::Redirect {
                    to: Ref::ControllerOf(controlled),
                    next: false,
                },
            duration: Duration::UntilEndOfTurn,
        } = effect
        else {
            return None;
        };
        if source.0.as_slice() != [Pred::Is(Ref::Target(0))]
            || **controlled != Ref::Target(0)
            || !self.target_is(|what| {
                matches!(what, Targetable::Object(selector) if selector.0.first() == Some(&Pred::Spell))
            })
        {
            return None;
        }
        let spell = self.name_target()?;
        Some(format!(
            "all damage that would be dealt this turn by {spell} is dealt to that spell's controller instead"
        ))
    }

    /// "target spell can't be countered".
    pub(super) fn rmt2_target_spell_uncounterable(&mut self, effect: &Effect) -> Option<String> {
        let Effect::Rule {
            rule: GameRule::CantBeCountered(Ref::Target(0)),
            duration: Duration::Forever,
        } = effect
        else {
            return None;
        };
        if !self.target_is(|what| {
            matches!(what, Targetable::Object(selector) if selector.0.as_slice() == [Pred::Spell])
        }) || !self.single(&Ref::Target(0))
        {
            return None;
        }
        let spell = self.name_target()?;
        Some(format!("{spell} can't be countered"))
    }
}
