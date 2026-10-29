//! The pass `w20c_owners` reads: "each player gains control of all creatures they own".

use super::Writer;
use crate::compile::read::selectors::{Form, phrase_text};
use crate::compile::read::w20c_owners::W20C_PLAYER;
use crate::effect::Effect;
use crate::modify::{Duration, Modification};
use crate::refs::{Pred, Ref, Selector};

impl Writer<'_> {
    /// "each player gains control of all creatures they own".
    pub(super) fn w20c_owners(effect: &Effect) -> Option<String> {
        let Effect::RepeatEach {
            over: Ref::EachPlayer,
            bind,
            effects,
        } = effect
        else {
            return None;
        };
        let [
            Effect::Continuous {
                affected: Ref::Each(group),
                mods,
                duration: Duration::Forever,
            },
        ] = effects.as_slice()
        else {
            return None;
        };
        let player = Ref::Bound(bind.clone());
        (bind == W20C_PLAYER && *mods == [Modification::SetController(player.clone())])
            .then_some(())?;
        let (owned, rest) = group.0.split_last()?;
        (*owned == Pred::OwnedBy(player)).then_some(())?;
        let rest = Selector(rest.to_vec());
        Some(format!(
            "each player gains control of all {} they own",
            phrase_text(&rest, Form::All)?
        ))
    }
}
