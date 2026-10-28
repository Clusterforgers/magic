//! An opponent who exiles cards from the top of their library until one is found, and the permission to cast it this turn:
//! "Target opponent exiles cards from the top of their library until they exile a nonland card. Until end of turn, you may
//! cast that card without paying its mana cost."

use super::{Effect, Ref, Targetable, Writer};
use crate::effect::{EXILED, FOUND};
use crate::modify::{CastPermission, Duration, GameRule};
use crate::refs::{Selector, ZoneKind};

impl Writer<'_> {
    /// The two sentences when the player who exiles is the one target, an opponent; the number of effects they take.
    pub(super) fn opponent_exiles_for_the_turn(
        &mut self,
        effects: &[Effect],
    ) -> Option<(String, usize)> {
        let [
            Effect::ExileUntil {
                who: Ref::Target(0),
                filter,
                bind,
                exiled_as,
            },
            Effect::Rule {
                rule: GameRule::MayCast(permission),
                duration: Duration::UntilEndOfTurn,
            },
            ..,
        ] = effects
        else {
            return None;
        };
        let allowed = CastPermission {
            who: Ref::You,
            what: Selector::default(),
            from: Some(ZoneKind::Exile),
            free: true,
            flash: false,
            only: Some(Ref::Bound(FOUND.to_owned())),
            play: false,
            once: false,
        };
        (bind == FOUND && exiled_as == EXILED && *permission == allowed).then_some(())?;
        (self.single(&Ref::Target(0))
            && self.target_is(|what| matches!(what, Targetable::Opponent)))
        .then_some(())?;
        let kind = super::library::filter_text(filter, false)?;
        let target = self.name_target()?;
        Some((
            format!(
                "{target} exiles cards from the top of their library until they exile {} {kind}. Until end of turn, you may cast that card without paying its mana cost",
                super::article(&kind)
            ),
            2,
        ))
    }
}
