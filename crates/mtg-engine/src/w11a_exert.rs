//! Exert (CR 701.43): a creature with `Keyword::Exert` may be exerted as it is declared an attacker, and it doesn't untap
//! during the next untap step of the player who exerted it.

use std::hash::{Hash, Hasher};

use mtg_cards::keyword::Keyword;
use mtg_core::ids::{ObjectId, PlayerId};

use crate::characteristics::controller_of;
use crate::combat::AttackTarget;
use crate::decision::AttackOptions;
use crate::error::{EngineError, IllegalAnswer};
use crate::events::Event;
use crate::fold::invalid;
use crate::object::GameObject;
use crate::state::{GameState, Tally};
use crate::zones::Zone;

/// The players whose next untap step an exerted permanent skips; an empty record adds nothing to the hash of the game.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct W11aExerted(pub(crate) Vec<PlayerId>);

impl Hash for W11aExerted {
    fn hash<H: Hasher>(&self, state: &mut H) {
        if !self.0.is_empty() {
            self.0.hash(state);
        }
    }
}

impl W11aExerted {
    /// The players who exerted the permanent and whose next untap step has not come yet.
    pub fn players(&self) -> &[PlayerId] {
        &self.0
    }
}

/// Whether `object` may be exerted as it attacks: it has exert, and for "If this creature hasn't been exerted this
/// turn" it hasn't been.
pub(crate) fn w11a_may_exert(state: &GameState, object: ObjectId) -> bool {
    let Some(reference) = state.object(object).map(GameObject::reference) else {
        return false;
    };
    let exerted = state.object_tallied(reference, None, Tally::W11aExerted) > 0;
    crate::keywords::keywords_of(state, object)
        .into_iter()
        .any(|keyword| matches!(keyword, Keyword::Exert { once_per_turn } if !*once_per_turn || !exerted))
}

/// Checks the creatures a declaration exerts: each is declared, once, and was offered as one that may be exerted.
pub(crate) fn w11a_check_exert(
    options: &AttackOptions,
    attackers: &[(ObjectId, AttackTarget)],
    exert: &[ObjectId],
) -> Result<(), IllegalAnswer> {
    for (i, object) in exert.iter().enumerate() {
        if exert[..i].contains(object) {
            return Err(IllegalAnswer::Duplicate("exerted creature"));
        }
        if !attackers.iter().any(|(attacker, _)| attacker == object) {
            return Err(IllegalAnswer::NotOffered(
                "exert of a creature not attacking",
            ));
        }
        if !options
            .attackers
            .iter()
            .any(|option| option.object == *object && option.exert)
        {
            return Err(IllegalAnswer::NotOffered("exert"));
        }
    }
    Ok(())
}

/// The `Exerted` events of a declaration, in the order the declaration names the creatures.
pub(crate) fn w11a_exert_events(player: PlayerId, exert: &[ObjectId]) -> Vec<Event> {
    exert
        .iter()
        .map(|object| Event::Exerted {
            object: *object,
            player,
        })
        .collect()
}

/// Whether the untap step under way skips `object`: a player who exerted it controls it now, and the untap step is
/// theirs (only the active player's permanents untap).
pub(crate) fn w11a_held(state: &GameState, object: ObjectId) -> bool {
    let controller = controller_of(state, object);
    state
        .object(object)
        .and_then(GameObject::permanent)
        .is_some_and(|permanent| {
            controller.is_some_and(|controller| permanent.exerted.0.contains(&controller))
        })
}

/// The `ExertEnded` events of the untap step of `active`: each exert of `active` ends in it, whoever controls the
/// permanent and whether it was tapped or not.
pub(crate) fn w11a_spent(state: &GameState, active: PlayerId) -> Vec<Event> {
    state
        .zone(Zone::Battlefield)
        .iter()
        .filter(|id| {
            state
                .object(**id)
                .and_then(GameObject::permanent)
                .is_some_and(|permanent| permanent.exerted.0.contains(&active))
        })
        .map(|id| Event::ExertEnded {
            object: *id,
            player: active,
        })
        .collect()
}

/// Folds `Exerted`: the permanent remembers the player and counts as exerted this turn.
pub(crate) fn w11a_fold_exerted(
    state: &mut GameState,
    object: ObjectId,
    player: PlayerId,
) -> Result<(), EngineError> {
    let reference = state
        .object(object)
        .filter(|found| found.zone() == Zone::Battlefield)
        .map(GameObject::reference)
        .ok_or_else(|| invalid(format!("{object:?} is not a permanent to exert")))?;
    let permanent = state
        .objects
        .get_mut(&object)
        .and_then(|found| found.permanent.as_mut())
        .ok_or_else(|| invalid(format!("{object:?} is not a permanent")))?;
    if !permanent.exerted.0.contains(&player) {
        permanent.exerted.0.push(player);
    }
    state.tally_object(reference, Some(player), Tally::W11aExerted);
    Ok(())
}

/// Folds `ExertEnded`: the permanent forgets the exert of `player`.
pub(crate) fn w11a_fold_ended(
    state: &mut GameState,
    object: ObjectId,
    player: PlayerId,
) -> Result<(), EngineError> {
    let permanent = state
        .objects
        .get_mut(&object)
        .and_then(|found| found.permanent.as_mut())
        .filter(|permanent| permanent.exerted.0.contains(&player))
        .ok_or_else(|| invalid(format!("{object:?} was not exerted by {player:?}")))?;
    permanent.exerted.0.retain(|exerter| *exerter != player);
    Ok(())
}
