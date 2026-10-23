//! Exiling a whole graveyard: "exile target player's graveyard", "exile each opponent's graveyard", "exile your graveyard".

use super::Writer;
use crate::effect::Effect;
use crate::refs::{Pred, Ref, Targetable, ZoneKind};

impl Writer<'_> {
    /// An exile of every card in one graveyard or in each of an opponent's.
    pub(super) fn graveyard_exile(&mut self, effect: &Effect) -> Option<String> {
        let Effect::Exile {
            what: Ref::Each(selector),
            bind: None,
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
