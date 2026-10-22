//! Landwalk (CR 702.14): the creature can't be blocked as long as the defending player controls a land the keyword names
//! (Islandwalk: an Island). The land is a selector, so a basic land type, a nonbasic land and a snow land are one rule.

use mtg_cards::keyword::Keyword;
use mtg_core::card::CardType;
use mtg_core::ids::ObjectId;

use super::KeywordRules;
use crate::characteristics::controller_of;
use crate::object::GameObject;
use crate::resolve::Scope;
use crate::state::GameState;
use crate::zones::Zone;

/// The rules of landwalk.
#[derive(Debug)]
pub(super) struct Landwalk;

impl KeywordRules for Landwalk {
    /// The blocker's controller is the defending player: only they may block (CR 506.1).
    fn restricts_blocking(
        &self,
        state: &GameState,
        keyword: &Keyword,
        attacker: ObjectId,
        blocker: ObjectId,
    ) -> bool {
        let Keyword::Landwalk(land) = keyword else {
            return false;
        };
        let (Some(defending), Some(attacking), Some(source)) = (
            controller_of(state, blocker),
            controller_of(state, attacker),
            state.object(attacker).map(GameObject::reference),
        ) else {
            return false;
        };
        let scope = Scope::announcing(state, attacking, source);
        state.zone(Zone::Battlefield).iter().any(|id| {
            controller_of(state, *id) == Some(defending)
                && state.object(*id).is_some_and(|object| {
                    crate::characteristics::characteristics(state, *id)
                        .is_some_and(|chars| chars.types.contains(CardType::Land))
                        && scope.selects(land, object.reference()).unwrap_or(false)
                })
        })
    }
}
