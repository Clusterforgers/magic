//! Commanders (CR 903): which objects are commanders, how often each was cast from the command zone (the tax of CR
//! 903.8), the combat damage each has dealt to each player (CR 903.10a), and the owner's choice to put a commander that
//! is in a graveyard or exile into the command zone (CR 903.9a). The replacement of a move to a hand or a library
//! (CR 903.9b) is in `replace.rs`.
//!
//! A commander is a card, so all of it is kept by object id, which a zone change leaves as it is. A token and a copy
//! of a commander are other objects and so no commanders. The tallies are folded from events and hashed only once they
//! hold something.

use std::collections::{BTreeMap, BTreeSet};
use std::hash::{Hash, Hasher};

use mtg_core::ids::{ObjectId, PlayerId};

use crate::answers::Outcome;
use crate::choose::{YesNoChoice, YesNoOptions};
use crate::decision::{Decision, DecisionKind};
use crate::error::EngineError;
use crate::events::{Event, ZoneCause};
use crate::state::GameState;
use crate::zones::Zone;

/// The commanders of a game and what is counted for them.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct Commanders {
    /// The commander cards, fixed when the game is set up.
    ids: BTreeSet<ObjectId>,
    /// Times each was cast from the command zone.
    casts: BTreeMap<ObjectId, u32>,
    /// Combat damage each has dealt to each player.
    damage: BTreeMap<(ObjectId, PlayerId), u32>,
    /// The commanders their owner left in a graveyard or in exile when asked, until they move again.
    stays: BTreeSet<ObjectId>,
}

impl Hash for Commanders {
    fn hash<H: Hasher>(&self, state: &mut H) {
        if self.ids.is_empty() {
            return;
        }
        self.ids.hash(state);
        if !self.casts.is_empty() {
            1u8.hash(state);
            self.casts.hash(state);
        }
        if !self.damage.is_empty() {
            2u8.hash(state);
            self.damage.hash(state);
        }
        if !self.stays.is_empty() {
            3u8.hash(state);
            self.stays.hash(state);
        }
    }
}

impl Commanders {
    /// The commanders of a game set up with these cards.
    pub(crate) fn of(ids: impl IntoIterator<Item = ObjectId>) -> Commanders {
        Commanders {
            ids: ids.into_iter().collect(),
            ..Commanders::default()
        }
    }

    /// Whether the game has a commander.
    pub(crate) fn any(&self) -> bool {
        !self.ids.is_empty()
    }

    /// Whether `id` is a commander.
    pub(crate) fn contains(&self, id: ObjectId) -> bool {
        self.ids.contains(&id)
    }

    /// The commanders in id order.
    pub(crate) fn ids(&self) -> impl Iterator<Item = ObjectId> + '_ {
        self.ids.iter().copied()
    }

    /// A commander moved: its owner is asked again the next time it lies in a graveyard or exile.
    pub(crate) fn moved(&mut self, id: ObjectId) {
        self.stays.remove(&id);
    }

    /// A card was cast from `from`: from the command zone it counts for the tax.
    pub(crate) fn cast(&mut self, id: ObjectId, from: Zone) {
        if matches!(from, Zone::Command(_)) && self.ids.contains(&id) {
            let count = self.casts.entry(id).or_insert(0);
            *count = count.saturating_add(1);
        }
    }

    /// Combat damage `amount` from `source` reached `player`.
    pub(crate) fn dealt(&mut self, source: ObjectId, player: PlayerId, amount: u32) {
        if self.ids.contains(&source) && amount > 0 {
            let total = self.damage.entry((source, player)).or_insert(0);
            *total = total.saturating_add(amount);
        }
    }

    /// The owner of `id` left it in a graveyard or exile.
    pub(crate) fn stay(&mut self, id: ObjectId) {
        if self.ids.contains(&id) {
            self.stays.insert(id);
        }
    }
}

impl GameState {
    /// Whether `id` is a player's commander.
    pub fn is_commander(&self, id: ObjectId) -> bool {
        self.commanders.contains(id)
    }

    /// Every commander of the game, in object order.
    pub fn commanders(&self) -> impl Iterator<Item = ObjectId> + '_ {
        self.commanders.ids()
    }

    /// How many times the commander `id` was cast from the command zone this game.
    pub fn commander_casts(&self, id: ObjectId) -> u32 {
        self.commanders.casts.get(&id).copied().unwrap_or(0)
    }

    /// The generic mana the commander `id` costs more to cast from the command zone now (CR 903.8).
    pub fn commander_tax(&self, id: ObjectId) -> u32 {
        self.commander_casts(id)
            .saturating_mul(self.ruleset().commander_tax())
    }

    /// The combat damage the commander `id` has dealt to `player` this game.
    pub fn commander_damage(&self, id: ObjectId, player: PlayerId) -> u32 {
        self.commanders
            .damage
            .get(&(id, player))
            .copied()
            .unwrap_or(0)
    }
}

/// The most combat damage any one commander has dealt to `player`.
pub(crate) fn damage_dealt_to(state: &GameState, player: PlayerId) -> u32 {
    state
        .commanders
        .damage
        .iter()
        .filter(|((_, hit), _)| *hit == player)
        .map(|(_, amount)| *amount)
        .max()
        .unwrap_or(0)
}

/// The generic mana the spell `spell` costs more because it is a commander cast from the command zone.
pub(crate) fn tax_for(state: &GameState, spell: ObjectId) -> u32 {
    let in_command = state
        .object(spell)
        .is_some_and(|card| matches!(card.zone(), Zone::Command(_)));
    if in_command {
        state.commander_tax(spell)
    } else {
        0
    }
}

/// The commander in a graveyard or exile that its owner has not been asked about yet, owners in turn order from the
/// active player (CR 903.9a).
fn waiting(state: &GameState) -> Option<ObjectId> {
    if !state.commanders.any() {
        return None;
    }
    let order = state.turn_order_from(state.turn().active());
    let place = |id: ObjectId| {
        let owner = state.object(id)?.owner();
        order.iter().position(|seat| *seat == owner)
    };
    state
        .commanders
        .ids()
        .filter(|id| !state.commanders.stays.contains(id))
        .filter(|id| {
            state.object(*id).is_some_and(|card| {
                matches!(card.zone(), Zone::Graveyard(_) | Zone::Exile) && state.is_alive(card.owner())
            })
        })
        .min_by_key(|id| (place(*id), *id))
}

/// The question the owner of a commander in a graveyard or exile is asked.
pub(crate) fn decision(state: &GameState) -> Option<Decision> {
    let id = waiting(state)?;
    let card = state.object(id)?;
    Some(Decision {
        player: card.owner(),
        kind: DecisionKind::YesNo(YesNoOptions::new(
            id,
            format!("Put {} into the command zone?", card.card().name),
        )),
    })
}

/// Whether a yes-or-no question of the state-based actions is the one about a commander.
pub(crate) fn asks(state: &GameState, options: &YesNoOptions) -> bool {
    waiting(state) == Some(options.source)
}

/// The answer: yes puts the commander into the command zone, no leaves it where it is until it moves again.
pub(crate) fn answer(
    state: &GameState,
    options: &YesNoOptions,
    choice: YesNoChoice,
) -> Result<Outcome, EngineError> {
    let Some(card) = state.object(options.source) else {
        return Ok(Outcome::Events(Vec::new()));
    };
    let event = if choice.0 {
        Event::ZoneChanged {
            object: card.id(),
            from: card.zone(),
            to: Zone::Command(card.owner()),
            cause: ZoneCause::Moved,
        }
    } else {
        Event::CommanderStayed { object: card.id() }
    };
    Ok(Outcome::Events(vec![event]))
}
