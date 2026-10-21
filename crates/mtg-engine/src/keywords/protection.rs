//! Protection (CR 702.16): a permanent or player with protection from a quality can't be damaged, enchanted,
//! equipped, fortified, blocked or targeted by a source with that quality (DEBT). The quality is a selector over the
//! source, so a colour, a card type, a player's objects and everything are all the same rule.

use mtg_cards::keyword::Keyword;

use super::{Holder, KeywordRules, Source};
use crate::characteristics::controller_of_object;
use crate::resolve::Scope;
use crate::state::GameState;
use crate::zones::Zone;

/// The rules of protection.
#[derive(Debug)]
pub(super) struct Protection;

impl KeywordRules for Protection {
    /// The quality is judged for the holder's controller (so "your opponents" is the holder's opponents), against the
    /// source as it was if it has gone (last-known information, CR 608.2h). Protection from everything has the empty
    /// selector and needs no look at the source.
    fn protects_from(
        &self,
        state: &GameState,
        keyword: &Keyword,
        holder: Holder,
        source: Source<'_>,
    ) -> bool {
        let Keyword::Protection(quality) = keyword else {
            return false;
        };
        let (controller, anchor) = match holder {
            Holder::Object(id) => {
                // A static ability works on the battlefield only (CR 113.6).
                let Some(object) = state
                    .object(id)
                    .filter(|object| object.zone() == Zone::Battlefield)
                else {
                    return false;
                };
                (controller_of_object(state, object), object.reference())
            }
            Holder::Player(player) => (player, source.object),
        };
        if quality.is_empty() {
            return true;
        }
        let mut scope = Scope::announcing(state, controller, anchor);
        scope.snapshots = source.snapshots.to_vec();
        scope.selects(quality, source.object).unwrap_or(false)
    }
}
