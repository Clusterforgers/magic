//! Exiling a whole graveyard: "exile target player's graveyard", "exile each opponent's graveyard", "exile your graveyard".

use super::Writer;
use crate::compile::read::effects::EXILED_CARD;
use crate::effect::Effect;
use crate::refs::{Bounds, Pred, Ref, Targetable, ZoneKind};

impl Writer<'_> {
    /// A player's choice of a card of their graveyard and its exile, as one sentence, and the number of effects it
    /// covers: "target player exiles a card from their graveyard".
    pub(super) fn exile_chosen_from_graveyard(
        &mut self,
        effects: &[Effect],
    ) -> Option<(Vec<String>, usize)> {
        let [
            Effect::Choose {
                who: who @ Ref::Target(0),
                from: Ref::Each(selector),
                count,
                bind,
                rest_as: None,
            },
            Effect::Exile {
                what: Ref::Bound(exiled),
                bind: None,
                duration: None,
            },
            ..,
        ] = effects
        else {
            return None;
        };
        let theirs = [Pred::In(ZoneKind::Graveyard), Pred::OwnedBy(who.clone())];
        (selector.0 == theirs
            && *count == Bounds::default()
            && bind == EXILED_CARD
            && exiled == bind
            && self.single(who)
            && self.target_is(|what| matches!(what, Targetable::Player | Targetable::Opponent)))
        .then_some(())?;
        Some((
            vec![format!(
                "{} exiles a card from their graveyard",
                self.name_target()?
            )],
            2,
        ))
    }

    /// An exile of every card in one graveyard or in each of an opponent's.
    pub(super) fn graveyard_exile(&mut self, effect: &Effect) -> Option<String> {
        let Effect::Exile {
            what: Ref::Each(selector),
            bind: _,
            duration: None,
        } = effect
        else {
            return None;
        };
        if selector.0 == [Pred::In(ZoneKind::Graveyard)] {
            return Some("exile all graveyards".to_owned());
        }
        let [Pred::In(ZoneKind::Graveyard), Pred::OwnedBy(owner)] = selector.0.as_slice() else {
            return None;
        };
        let whose = match owner {
            Ref::You => "your".to_owned(),
            Ref::Opponents => "each opponent's".to_owned(),
            Ref::Target(0)
                if self.single(owner)
                    && self.target_is(|what| {
                        matches!(what, Targetable::Player | Targetable::Opponent)
                    }) =>
            {
                format!("{}'s", self.name_target()?)
            }
            _ => return None,
        };
        Some(format!("exile {whose} graveyard"))
    }
}
