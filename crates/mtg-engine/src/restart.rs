//! Restarting the game (CR 727, Karn Liberated): `GameRestarted` puts every card back in its owner's library, starts the
//! players over and opens the game again with the restarting player first; the cards kept in exile enter the
//! battlefield under that player's control once the opening is over.

use std::collections::BTreeMap;
use std::sync::Arc;

use mtg_core::ids::{ObjectId, PlayerId};

use crate::events::{Event, ZoneCause};
use crate::object::{GameObject, ObjectKind};
use crate::player::MulliganStatus;
use crate::state::{GameState, Stage, Tallies};
use crate::turn::{Step, TurnState};
use crate::zones::{Placement, Zone, Zones};

/// A restart under way: who restarted the game and the cards that enter the battlefield after the opening.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) struct Restart {
    /// The player whose effect restarted the game: the starting player of the new game and the controller of `kept`.
    pub(crate) by: PlayerId,
    /// The cards left in exile.
    pub(crate) kept: Vec<ObjectId>,
}

/// Folds `GameRestarted`. The spell or ability that restarted the game stays on the stack until its resolution ends;
/// every other object that is no card ceases to exist (CR 727.2).
pub(crate) fn fold(state: &mut GameState, by: PlayerId, kept: &[ObjectId]) {
    let resolving = state.resolving.as_ref().map(|resolving| resolving.object);
    let mut zones = Zones::new(state.players.len());
    let mut objects = BTreeMap::new();
    let mut merged = Vec::new();
    for (id, mut object) in std::mem::take(&mut state.objects) {
        if Some(id) == resolving && object.zone == Zone::Stack {
            zones.insert(Zone::Stack, id, Placement::Top);
            objects.insert(id, object);
            continue;
        }
        // The cards of a merged permanent become objects of their own again (CR 729.3), its tokens cease to exist.
        merged.push((crate::merged::taken(&mut object.permanent), object.owner));
        if object.kind() != ObjectKind::Card {
            continue;
        }
        if kept.contains(&id) && object.zone == Zone::Exile {
            zones.insert(Zone::Exile, id, Placement::Top);
            objects.insert(id, object);
            continue;
        }
        let owner = object.owner;
        let zone = if state.commanders.contains(id) {
            Zone::Command(owner)
        } else {
            Zone::Library(owner)
        };
        let mut card = GameObject::new(id, owner, Arc::clone(&object.def), zone);
        // A new object (CR 400.7): no reference taken in the old game names it.
        card.counter = object.counter + 1;
        zones.insert(zone, id, Placement::Top);
        objects.insert(id, card);
    }
    state.objects = objects;
    state.zones = zones;
    for (cards, owner) in merged {
        let cards = cards.into_iter().map(|(card, _)| (card, None)).collect();
        crate::merged::scatter(state, cards, owner, Zone::Library(owner), Placement::Top);
    }
    state.ceased.clear();
    let starting: Vec<i32> = state
        .players
        .iter()
        .map(|player| state.ruleset().starting_life(player.id))
        .collect();
    for (player, life) in state.players.iter_mut().zip(starting) {
        player.life = life;
        player.mana = crate::mana::ManaPool::default();
        player.failed_draws = 0;
        player.mulligans = 0;
        player.mulligan = MulliganStatus::NeedsHand;
        player.counters.clear();
    }
    state.stage = Stage::PreGame;
    state.turn = TurnState {
        number: 0,
        active: by,
        step: Step::Untap,
        lands_played: 0,
        round: 0,
        regular: by,
        extra: false,
        combats: 0,
        ahead: Vec::new(),
        ending: false,
    };
    state.extra_turns.clear();
    state.player_controls.clear();
    state.priority = None;
    state.combat = crate::combat::CombatState::default();
    state.effects.clear();
    state.rule_effects.clear();
    state.pending_triggers.clear();
    state.delayed.clear();
    state.limits_spent.clear();
    state.game_limits_spent.clear();
    state.loyalty_used.clear();
    state.loyalty_again.clear();
    state.modes_ever.clear();
    state.tallies = Tallies::default();
    state.day_night = None;
    state.seat_turns.clear();
    state.last_turn_of = None;
    state.monarch = None;
    state.citys_blessing.clear();
    state.speeds = crate::speed::Speeds::default();
    state.ring = crate::ring::Rings::new();
    state.w11d = crate::w11d_dungeon::W11dDungeons::default();
    state.w15a_initiative = None;
    state.commanders = state.commanders.restarted();
    state.looked_by_hand.clear();
    state.revealed_by_hand.clear();
    state.notes.clear();
    state.memory = crate::memory::Memory::new();
    state.restart = Some(Restart {
        by,
        kept: kept.to_vec(),
    });
}

/// Folds `MulliganPhaseEnded`: the turns begin and a restart under way is over.
pub(crate) fn opening_ended(state: &mut GameState) {
    state.stage = Stage::Turns;
    state.restart = None;
}

/// The starting player of a restarted game: the player who restarted it (CR 727.3); `None` for a game that was not.
pub(crate) fn starting_player(state: &GameState) -> Option<PlayerId> {
    state.restart.as_ref().map(|restart| restart.by)
}

/// The events that put the kept cards still in exile onto the battlefield under the restarting player's control, as the
/// opening of a restarted game ends.
pub(crate) fn kept_enter(state: &GameState) -> Vec<Event> {
    let Some(restart) = &state.restart else {
        return Vec::new();
    };
    let mut events = Vec::new();
    for &object in &restart.kept {
        let Some(card) = state
            .object(object)
            .filter(|card| card.zone() == Zone::Exile)
        else {
            continue;
        };
        events.push(Event::ZoneChanged {
            object,
            from: Zone::Exile,
            to: Zone::Battlefield,
            cause: ZoneCause::Moved,
        });
        if card.owner() != restart.by {
            events.push(Event::Entered {
                object,
                tapped: false,
                controller: restart.by,
                chosen: None,
            });
        }
    }
    events
}
