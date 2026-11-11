//! Entering under an opponent's control: "~ enters under the control of an opponent of your choice." is
//! `ReplaceBy::UnderChosenOpponent` of the card itself; the writer's side is `UNDER_OPPONENT` in the statics.

use super::tokens::Reader;
use crate::ability::Ability;
use crate::refs::{Selector, ZoneKind};
use crate::trigger::{ReplaceBy, ReplaceWhen};

/// The words of `ReplaceBy::UnderChosenOpponent` of the card itself.
pub(super) const UNDER_OPPONENT: &str = "~ enters under the control of an opponent of your choice";

/// "~ enters under the control of an opponent of your choice.": the card enters under the opponent its controller chooses.
pub fn enters_under_opponent(r: &mut Reader<'_>) -> Option<Ability> {
    r.source().then_some(())?;
    r.need("enters under the control of an opponent of your choice")?;
    Some(Ability::Replacement {
        when: ReplaceWhen::Enters {
            what: Selector::this(),
        },
        by: ReplaceBy::UnderChosenOpponent,
        condition: None,
        from: ZoneKind::Battlefield,
    })
}
