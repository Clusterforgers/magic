//! The changes of control `w6e_control` reads: "Gain control of up to one target creature an opponent controls until end
//! of turn. Untap that creature. It gains trample and haste until end of turn.", "gain control of enchanted creature",
//! "gain control of all artifacts", and "exchange control of two target creatures" or of two targets of their own.

use super::Writer;
use crate::compile::read::selectors::{Form, granted_text, phrase_text, spec_text};
use crate::effect::Effect;
use crate::modify::{Duration, Modification};
use crate::refs::{Pred, Ref, TargetSpec, Targetable};
use crate::value::Amount;
use mtg_core::card::CardType;

impl Writer<'_> {
    /// What a change of control names that `control` does not: up to one target, the enchanted permanent, all of a
    /// group.
    pub(super) fn w6e_taken(&mut self, affected: &Ref) -> Option<String> {
        match affected {
            Ref::Target(0)
                if self.at_most_one(affected)
                    && self.target_is(|what| matches!(what, Targetable::Object(_))) =>
            {
                self.name_target()
            }
            Ref::Attached => Some("enchanted creature".to_owned()),
            Ref::Each(selector) => Some(format!("all {}", phrase_text(selector, Form::All)?)),
            _ => None,
        }
    }

    /// The control of the taken permanents for the turn, their untap and the keywords they gain: one sentence group that no
    /// joint splits, and the number of effects it covers.
    pub(super) fn w6e_stolen(&mut self, effects: &[Effect]) -> Option<(String, usize)> {
        let [
            Effect::Continuous {
                affected,
                mods: take,
                duration: Duration::UntilEndOfTurn,
            },
            Effect::Untap { what: untapped },
            Effect::Continuous {
                affected: gaining,
                mods: gain,
                duration: Duration::UntilEndOfTurn,
            },
            ..,
        ] = effects
        else {
            return None;
        };
        (affected == untapped && affected == gaining).then_some(())?;
        let [Modification::SetController(Ref::You)] = take.as_slice() else {
            return None;
        };
        let noun = match affected {
            Ref::Attached => "creature",
            Ref::Target(0) => self.w6e_noun()?,
            _ => return None,
        };
        let keywords = granted_text(gain)?;
        let taken = self.w6e_taken(affected)?;
        Some((
            format!(
                "gain control of {taken} until end of turn. Untap that {noun}. It gains {keywords} until end of turn"
            ),
            3,
        ))
    }

    /// "creature" for a target creature, "permanent" for a target that names no kind.
    pub(super) fn w6e_noun(&self) -> Option<&'static str> {
        if self.target_is(|what| {
            matches!(what, Targetable::Object(selector)
                if selector.0.contains(&Pred::Type(CardType::Creature)))
        }) {
            Some("creature")
        } else if self
            .target_is(|what| matches!(what, Targetable::Object(selector) if selector.0.is_empty()))
        {
            Some("permanent")
        } else {
            None
        }
    }

    /// "exchange control of two target creatures", "exchange control of target land you control and target land an
    /// opponent controls".
    pub(super) fn w6e_exchanged(&mut self, effect: &Effect) -> Option<String> {
        let Effect::ExchangeControl {
            what,
            duration: Duration::Forever,
        } = effect
        else {
            return None;
        };
        let [Ref::Target(0)] = what.as_slice() else {
            return None;
        };
        let spec = self.targets.first()?;
        let pair = Some(Amount::Fixed(2));
        (spec.bounds.min == Amount::Fixed(2) && spec.bounds.max == pair).then_some(())?;
        self.named = true;
        Some(format!("exchange control of {}", spec_text(spec)?))
    }
}

/// "exchange control of target land you control and target land an opponent controls": the one sentence of an exchange
/// of two targets of their own, for good.
pub(super) fn w6e_exchange_pair(targets: &[TargetSpec], effects: &[Effect]) -> Option<String> {
    let [
        Effect::ExchangeControl {
            what,
            duration: Duration::Forever,
        },
    ] = effects
    else {
        return None;
    };
    let ([Ref::Target(0), Ref::Target(1)], [first, second]) = (what.as_slice(), targets) else {
        return None;
    };
    (first.bounds.is_one() && second.bounds.is_one()).then_some(())?;
    Some(format!(
        "exchange control of {} and {}",
        spec_text(first)?,
        spec_text(second)?
    ))
}

/// The other wordings of a change of control: an Aura that enchants a permanent or an artifact names its host so, and a
/// target for each opponent is "those creatures" that "gain" the keywords.
pub(super) fn w6e_wordings(texts: &[String]) -> Vec<String> {
    let mut more = Vec::new();
    for text in texts
        .iter()
        .filter(|text| text.contains("control of enchanted creature"))
    {
        for noun in ["permanent", "artifact"] {
            more.push(
                text.replace("enchanted creature", &format!("enchanted {noun}"))
                    .replace("Untap that creature", &format!("Untap that {noun}")),
            );
        }
    }
    more
}

/// The steps of a target for each opponent in the plural: "Untap those creatures. They gain haste".
pub(super) fn w6e_plural(text: &str) -> String {
    text.replace(
        "Untap that creature. It gains",
        "Untap those creatures. They gain",
    )
}
