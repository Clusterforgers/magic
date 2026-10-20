//! Tokens: the `TokenCreated` event, the object it makes and the rule that a token exists only on the battlefield
//! (CR 111.7).

use std::collections::BTreeMap;
use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_core::ids::{ObjectId, PlayerId};
use serde::{Deserialize, Serialize};

use crate::combat::{AttackTarget, Attacker};
use crate::error::EngineError;
use crate::fold::{invalid, reset_passes};
use crate::object::{GameObject, ObjectKind, PermanentState};
use crate::state::GameState;
use crate::zones::{Placement, Zone};

/// Whether a flag is off, so serialization can leave it out (serde hands over a reference).
#[allow(clippy::trivially_copy_pass_by_ref)]
const fn is_false(flag: &bool) -> bool {
    !*flag
}

/// A token that was created: it enters the battlefield under its controller's control, who is also its owner.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct TokenCreated {
    /// The token's object.
    pub object: ObjectId,
    /// Who controls it.
    pub controller: PlayerId,
    /// What it is called.
    pub name: String,
    /// It enters tapped.
    #[serde(default, skip_serializing_if = "is_false")]
    #[cfg_attr(feature = "ts", ts(as = "Option<bool>", optional))]
    pub tapped: bool,
    /// It enters attacking this player or planeswalker: it was never declared as an attacker (CR 508.4).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub attacking: Option<AttackTarget>,
    /// Its characteristics and abilities, so a log folds to the same game with no card data at hand.
    #[cfg_attr(feature = "ts", ts(skip))]
    pub definition: Arc<CardDefinition>,
}

/// Folds the event: the token is a new object on the battlefield, tapped and attacking as the event says.
pub(crate) fn fold(state: &mut GameState, event: &TokenCreated) -> Result<(), EngineError> {
    let id = event.object;
    if state.objects.contains_key(&id) || state.ceased.contains_key(&id) {
        return Err(invalid(format!("{id:?} already exists")));
    }
    if !state.is_alive(event.controller) {
        return Err(invalid(format!(
            "{:?} is out of the game and cannot control a token",
            event.controller
        )));
    }
    if event.attacking.is_some() && !state.combat.attackers_declared {
        return Err(invalid(
            "a token cannot enter attacking before attackers are declared",
        ));
    }
    let mut token = GameObject::new(
        id,
        event.controller,
        Arc::clone(&event.definition),
        Zone::Battlefield,
    );
    token.kind = ObjectKind::Token;
    token.permanent = Some(PermanentState {
        tapped: event.tapped,
        damage: 0,
        deathtouched: false,
        summoning_sick: true,
        counters: BTreeMap::new(),
        attached_to: None,
    });
    state.objects.insert(id, token);
    state.zones.insert(Zone::Battlefield, id, Placement::Top);
    state.next_object = state.next_object.max(id.get() + 1);
    if let Some(target) = event.attacking {
        state.combat.attackers.push(Attacker {
            object: id,
            target,
            blocked: false,
            blockers: Vec::new(),
            assignment: None,
        });
    }
    reset_passes(state);
    Ok(())
}

/// Ends a token that has left the battlefield (CR 111.7, 704.5d).
///
/// It is gone from every zone and from `objects`, but the object is kept apart: a trigger or an ability of it that
/// is already on its way (a dies trigger, a sacrifice cost's ability) still needs the card it was printed on.
pub(crate) fn cease(state: &mut GameState, id: ObjectId) {
    for zone in crate::zones::Zones::all_zones(state.players.len()) {
        if state.zones.remove(zone, id) {
            break;
        }
    }
    if let Some(token) = state.objects.remove(&id) {
        state.ceased.insert(id, token);
    }
}
