//! Facts of the whole game the r55-22 cards read: the cards of each name each player cycled ("if you've cycled a card
//! named ~ four or more times this game") and every damage dealt to a player with its source as it was ("each opponent
//! dealt combat damage this game by a creature named ~", "target opponent previously dealt damage by it").

use mtg_cards::refs::{Ref, Selector};
use mtg_core::ids::{ObjectId, PlayerId};

use crate::resolve::{ResolveError, Scope};
use crate::state::GameState;

/// Keeps how many untapped lands each player controls as a turn begins, before its untap step.
pub(crate) fn r5522_note_turn_began(state: &mut GameState) {
    let mut counts: std::collections::BTreeMap<PlayerId, u32> = std::collections::BTreeMap::new();
    for id in state.zone(crate::zones::Zone::Battlefield) {
        let Some(chars) = crate::characteristics::characteristics(state, *id) else {
            continue;
        };
        let untapped = state
            .object(*id)
            .and_then(crate::object::GameObject::permanent)
            .is_some_and(|permanent| !permanent.tapped());
        if untapped
            && chars.types.contains(mtg_core::card::CardType::Land)
            && let Some(controller) = crate::characteristics::controller_of(state, *id)
        {
            *counts.entry(controller).or_insert(0) += 1;
        }
    }
    state.r5522_untapped_lands = counts;
}

/// Keeps the creatures the active player could declare as attackers now, as attackers are declared.
pub(crate) fn r5522_note_could_attack(state: &mut GameState) {
    let options = crate::combat::attack_options(state, state.turn.active);
    let could: Vec<crate::object::ObjectRef> = options
        .attackers
        .iter()
        .filter_map(|option| state.object(option.object))
        .map(crate::object::GameObject::reference)
        .collect();
    state.tallies.r5522_could_attack.extend(could);
}

/// Counts a spell cast from anywhere other than its caster's hand, read before it moves to the stack.
pub(crate) fn r5522_note_played(state: &mut GameState, player: PlayerId, object: ObjectId) {
    if state
        .object(object)
        .is_some_and(|card| card.zone() != crate::zones::Zone::Hand(player))
    {
        state.tally(player, crate::state::Tally::R5522PlayedNotFromHand);
    }
}

/// Turns an object that moved face down as its cause says, counting a permanent that entered the battlefield face down.
pub(crate) fn r5522_put_face_down(
    state: &mut GameState,
    object: ObjectId,
    how: crate::object::FaceDown,
    to: crate::zones::Zone,
) {
    let Some(moved) = state.objects.get_mut(&object) else {
        return;
    };
    let was_face_down = moved.face_down.is_some();
    moved.face_down = Some(how);
    let controller = moved.controller;
    if to == crate::zones::Zone::Battlefield && !was_face_down {
        state.tally(controller, crate::state::Tally::R5522EnteredFaceDown);
    }
}

/// Counts a card cycled for its name and the player who cycled it.
pub(crate) fn r5522_note_cycled(state: &mut GameState, player: PlayerId, object: ObjectId) {
    let Some(name) = state
        .source_object(object)
        .map(|card| card.card().name.clone())
    else {
        return;
    };
    *state.r5522_cycled_names.entry((player, name)).or_insert(0) += 1;
}

/// Keeps a damage dealt to a player with its source as it is now.
pub(crate) fn r5522_note_hit(
    state: &mut GameState,
    source: ObjectId,
    player: PlayerId,
    combat: bool,
) {
    if let Some(shot) = crate::snapshot::snapshot(state, source) {
        state.r5522_hits.push((player, combat, shot));
    }
}

impl Scope<'_> {
    /// Whether "you" were dealt damage this game (combat damage with `combat`) by a source `by` selects as it was then.
    pub(crate) fn r5522_damaged_this_game(
        &self,
        by: &Selector,
        combat: bool,
    ) -> Result<bool, ResolveError> {
        let you = self.players(&Ref::You)?;
        for (player, in_combat, shot) in &self.state.r5522_hits {
            if !you.contains(player) || (combat && !in_combat) {
                continue;
            }
            let mut scope = self.clone();
            scope.as_was = true;
            scope.snapshots.insert(0, shot);
            if scope.selects(by, shot.reference())? {
                return Ok(true);
            }
        }
        Ok(false)
    }

    /// The cards with the name of the object with the ability that the players `who` names cycled this game.
    pub(crate) fn r5522_cycled_same_name(&self, who: &Ref) -> Result<i64, ResolveError> {
        let name = self
            .state
            .source_object(self.source.id)
            .map(|source| source.card().name.clone())
            .unwrap_or_default();
        let total: u32 = self
            .players(who)?
            .into_iter()
            .map(|player| {
                self.state
                    .r5522_cycled_names
                    .get(&(player, name.clone()))
                    .copied()
                    .unwrap_or(0)
            })
            .sum();
        Ok(i64::from(total))
    }
}
