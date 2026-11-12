//! "only if you've been attacked this step": whether a player was attacked in the combat phase in progress.

use mtg_cards::refs::Ref;
use mtg_core::ids::PlayerId;

use super::{ResolveError, Scope};
use crate::combat::{Attacker, defending_player};

impl Scope<'_> {
    /// Whether a player `who` names was attacked this combat, directly or through a planeswalker or battle (CR 506.2):
    /// the attackers in combat and those that left it, each by the player it attacked (`Condition::AttackedThisCombat`).
    pub(crate) fn r285_attacked_this_combat(&self, who: &Ref) -> Result<bool, ResolveError> {
        let combat = self.state.combat();
        let attacked: Vec<PlayerId> = combat
            .attackers()
            .iter()
            .map(Attacker::target)
            .chain(combat.left.iter().map(|(_, target)| *target))
            .filter_map(|target| defending_player(self.state, target))
            .collect();
        Ok(self
            .players(who)?
            .iter()
            .any(|player| attacked.contains(player)))
    }
}
