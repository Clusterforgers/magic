//! The initiative (CR 725): the designation, the object in its holder's command zone that carries its two venture
//! triggers, the way combat damage takes it and who takes it when its holder leaves the game.
//!
//! At most one player has the initiative. `Effect::TakeInitiative` and combat damage log `InitiativeTaken`, also for a
//! player who already has it (CR 725.2: they venture again). The holder's command zone holds an object of
//! `ObjectKind::Initiative` whose triggered abilities are the inherent ones of CR 725.2: "whenever you take the
//! initiative" and "at the beginning of your upkeep", each "venture into Undercity", so both use the stack. The object
//! moves with the designation: a new holder gets a new one and the old one leaves the game.

use std::sync::{Arc, OnceLock};

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_cards::assemble::token_definition;
use mtg_cards::card::TokenSpec;
use mtg_cards::effect::Effect;
use mtg_cards::refs::{Ref, ZoneKind};
use mtg_cards::trigger::{Step, TriggerPattern};
use mtg_core::ids::{ObjectId, PlayerId};

use crate::combat::DamageTarget;
use crate::error::EngineError;
use crate::events::Event;
use crate::fold::{invalid, reset_passes};
use crate::object::{GameObject, ObjectKind};
use crate::state::GameState;
use crate::zones::{Placement, Zone};

/// The name of the object that stands for the initiative in its holder's command zone.
pub const W15A_INITIATIVE_NAME: &str = "The Initiative";

/// Who has the initiative and the object in their command zone that stands for it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct W15aHeld {
    pub(crate) player: PlayerId,
    pub(crate) object: ObjectId,
}

impl GameState {
    /// The player who has the initiative, or nobody before a card has given it.
    pub fn w15a_initiative(&self) -> Option<PlayerId> {
        self.w15a_initiative.map(|held| held.player)
    }
}

/// The definition of the initiative's object: the two inherent triggered abilities, each "venture into Undercity",
/// working from the command zone.
fn w15a_definition() -> Arc<CardDefinition> {
    static DEFINITION: OnceLock<Arc<CardDefinition>> = OnceLock::new();
    Arc::clone(DEFINITION.get_or_init(|| {
        let venture = vec![Effect::Venture {
            who: Ref::You,
            undercity: true,
        }];
        let trigger = |when| Ability::Triggered {
            when,
            intervening_if: None,
            limit: None,
            from: ZoneKind::Command,
            targets: Vec::new(),
            effects: venture.clone(),
        };
        let spec = TokenSpec {
            name: W15A_INITIATIVE_NAME.to_owned(),
            colors: Vec::new(),
            supertypes: Vec::new(),
            types: Vec::new(),
            subtypes: Vec::new(),
            power: None,
            toughness: None,
            abilities: vec![
                trigger(TriggerPattern::TakesInitiative { who: Ref::You }),
                trigger(TriggerPattern::StepBegins {
                    step: Step::Upkeep,
                    whose: Ref::You,
                }),
            ],
        };
        Arc::new(token_definition(&spec, &crate::caps::caps()))
    }))
}

/// The event of `player` taking the initiative from `held`: the object stays when they have it, else a new one is made
/// with the next object id.
pub(crate) fn w15a_take(held: Option<W15aHeld>, player: PlayerId, next_object: &mut u64) -> Event {
    let object = match held {
        Some(held) if held.player == player => held.object,
        _ => {
            let object = ObjectId(*next_object);
            *next_object += 1;
            object
        }
    };
    Event::InitiativeTaken { player, object }
}

/// Folds `InitiativeTaken`: the old holder's object leaves the game and the new holder's is made.
pub(crate) fn w15a_fold_taken(
    state: &mut GameState,
    player: PlayerId,
    object: ObjectId,
) -> Result<(), EngineError> {
    if let Some(held) = state.w15a_initiative
        && held.object != object
    {
        state.zones.remove(Zone::Command(held.player), held.object);
        state.objects.remove(&held.object);
    }
    if !state.objects.contains_key(&object) {
        if state.ceased.contains_key(&object) {
            return Err(invalid(format!("{object:?} already existed")));
        }
        let definition = w15a_definition();
        let zone = Zone::Command(player);
        let mut made = GameObject::new(object, player, Arc::clone(&definition), zone);
        made.kind = ObjectKind::Initiative;
        crate::copies::note_definition(state, &definition);
        state.objects.insert(object, made);
        state.zones.insert(zone, object, Placement::Top);
        state.next_object = state.next_object.max(object.get() + 1);
    }
    state.w15a_initiative = Some(W15aHeld { player, object });
    reset_passes(state);
    Ok(())
}

/// Who takes the initiative after `events` of combat damage: the controller of a creature that dealt damage to the
/// player who has it, the first in turn order from the active player when several did.
pub(crate) fn w15a_taken_by_combat(state: &GameState, events: &[Event]) -> Option<Event> {
    let holder = state.w15a_initiative()?;
    let hit_by: Vec<PlayerId> = events
        .iter()
        .filter_map(|event| match event {
            Event::DamageDealt {
                source,
                target: DamageTarget::Player(hurt),
                ..
            } if *hurt == holder => crate::characteristics::controller_of(state, *source),
            _ => None,
        })
        .filter(|player| *player != holder)
        .collect();
    let start = state.active_player().unwrap_or(holder);
    let taker = state
        .turn_order_from(start)
        .into_iter()
        .find(|player| hit_by.contains(player))?;
    Some(w15a_take(
        state.w15a_initiative,
        taker,
        &mut state.next_object.clone(),
    ))
}

/// The state-based check of CR 725.4: when the player who has the initiative has left the game, the active player
/// takes it, or the next player in turn order when the active player is the one who left.
pub(crate) fn w15a_sba(state: &GameState) -> Vec<Event> {
    let Some(holder) = state.w15a_initiative() else {
        return Vec::new();
    };
    if state.is_alive(holder) {
        return Vec::new();
    }
    crate::monarch::successor(state, holder)
        .map(|taker| w15a_take(state.w15a_initiative, taker, &mut state.next_object.clone()))
        .into_iter()
        .collect()
}
