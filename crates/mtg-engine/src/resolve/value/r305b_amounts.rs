//! The values r30-5b adds: the players who have lost the game, the targets of the spell being cast, a spell being cast
//! that targets a player (read from what it announced), and a player who attacked you during their last turn.

use mtg_core::ids::PlayerId;

use super::{CastTargets, ResolveError, Scope};
use crate::choose::Target;
use crate::object::ObjectRef;

impl Scope<'_> {
    /// Whether one of the players `who` names attacked "you" during their latest finished turn
    /// (`Condition::AttackedYouLastTurn`).
    pub(super) fn r305b_attacked_you(
        &self,
        who: &mtg_cards::refs::Ref,
    ) -> Result<bool, ResolveError> {
        Ok(self
            .players(who)?
            .into_iter()
            .any(|player| self.state.r305b_attacked_last_turn(player, self.controller)))
    }

    /// How many players of the game have lost it (`Amount::PlayersLost`).
    pub(super) fn r305b_players_lost(&self) -> i64 {
        let seats = self.state.players.len();
        let lost = (0..seats)
            .map(mtg_core::ids::PlayerId::from_index)
            .filter(|player| !self.state.is_alive(*player))
            .count();
        i64::try_from(lost).unwrap_or(i64::MAX)
    }

    /// How many targets the spell being cast has, each target of each specification counted (`Amount::CastTargets`):
    /// before its targets are announced, one for each specification it may announce.
    pub(super) fn r305b_cast_targets(&self) -> Result<i64, ResolveError> {
        let Some(CastTargets { choices, judging }) =
            self.cast_targets.filter(|_| self.casting.is_some())
        else {
            return Err(ResolveError::Invalid("no spell is being cast".into()));
        };
        let count = match judging {
            Some(specs) => specs.len(),
            None => choices.targets.iter().flatten().count(),
        };
        Ok(i64::try_from(count).unwrap_or(i64::MAX))
    }

    /// Whether the spell `reference` names, while it is being cast, targets one of `players` (`Pred::TargetsPlayer`): what
    /// it announced, or before its targets are announced whether one of its specifications may target one of them.
    /// `None` when it is not the spell being cast.
    pub(super) fn r305b_casting_targets_player(
        &self,
        reference: ObjectRef,
        players: &[PlayerId],
    ) -> Option<bool> {
        let (cast, caster) = self.casting.filter(|(cast, _)| *cast == reference.id)?;
        let CastTargets { choices, judging } = self.cast_targets?;
        let found = match judging {
            Some(specs) => specs.iter().any(|spec| {
                crate::choose::legal_targets(self.state, spec, cast, caster)
                    .into_iter()
                    .any(|target| matches!(target, Target::Player(player) if players.contains(&player)))
            }),
            None => choices
                .targets
                .iter()
                .flatten()
                .any(|target| matches!(target, Target::Player(player) if players.contains(player))),
        };
        Some(found)
    }
}
