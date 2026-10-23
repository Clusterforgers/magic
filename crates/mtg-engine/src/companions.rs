//! Companion (CR 702.139): a card a seat names before the game and keeps outside it. It is revealed before the
//! libraries are shuffled, and once per game, any time its owner could cast a sorcery, they may pay {3} to put it into
//! their hand (a special action, CR 116.2g).
//!
//! The card is no object while it lies outside the game: the state keeps its definition and the id it will have in the
//! hand, which the payment of the action names as its source. The seats other than the owner learn that a companion
//! exists with the setup and which card it is when it is revealed.

use std::collections::BTreeMap;
use std::hash::{Hash, Hasher};
use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_core::ids::{ObjectId, PlayerId};
use mtg_core::mana::{ManaCost, ManaSymbol};
use serde::{Deserialize, Serialize};

use crate::error::EngineError;
use crate::events::Event;
use crate::fold::invalid;
use crate::mana::{ManaOption, ManaPool, can_afford};
use crate::object::GameObject;
use crate::priority::sorcery_timing;
use crate::spend::Spend;
use crate::state::GameState;
use crate::zones::{Placement, Zone};

/// Where a seat's companion is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub enum CompanionStage {
    /// Named by the seat's deck and not revealed yet: only its owner knows which card it is.
    Hidden,
    /// Revealed to the table and outside the game: its owner may bring it into their hand.
    Revealed,
    /// Brought into the game: it is an object, in the owner's hand or wherever it went from there.
    Brought,
}

/// A seat's companion as the views read it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompanionInfo {
    /// The id the card has once it is brought into the game.
    pub card: ObjectId,
    /// The card's name.
    pub name: String,
    /// Where it is.
    pub stage: CompanionStage,
}

/// One seat's companion.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Slot {
    id: ObjectId,
    def: Arc<CardDefinition>,
    stage: CompanionStage,
}

/// The companions of a game by owner; empty when no seat names one.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct Companions {
    slots: BTreeMap<PlayerId, Slot>,
}

impl Hash for Companions {
    fn hash<H: Hasher>(&self, state: &mut H) {
        // A game without companions hashes as it did before they existed.
        for (player, slot) in &self.slots {
            player.hash(state);
            slot.id.hash(state);
            slot.def.name.hash(state);
            slot.stage.hash(state);
        }
    }
}

impl Companions {
    /// The companion `player` names, outside the game in `stage`, with the `id` it will have in the hand.
    pub(crate) fn name(
        &mut self,
        player: PlayerId,
        id: ObjectId,
        def: Arc<CardDefinition>,
        stage: CompanionStage,
    ) {
        self.slots.insert(player, Slot { id, def, stage });
    }
}

/// The mana the special action costs: {3}, whatever the card. Cost modifiers do not apply to it.
pub(crate) fn action_cost() -> ManaCost {
    ManaCost::from_symbols([ManaSymbol::Generic(3)])
}

impl GameState {
    /// The companion `player` named, whatever stage it is at.
    pub fn companion(&self, player: PlayerId) -> Option<CompanionInfo> {
        self.companions
            .slots
            .get(&player)
            .map(|slot| CompanionInfo {
                card: slot.id,
                name: slot.def.name.clone(),
                stage: slot.stage,
            })
    }
}

/// The reveals of the companions at the start of the game, `first` and then the others in turn order: each seat with a
/// companion that is still hidden shows it before any library is shuffled.
pub(crate) fn reveals(state: &GameState, first: PlayerId) -> Vec<Event> {
    let seats = state.players().len();
    (0..seats)
        .map(|offset| PlayerId::from_index((first.index() + offset) % seats))
        .filter(|player| {
            state
                .companions
                .slots
                .get(player)
                .is_some_and(|slot| slot.stage == CompanionStage::Hidden)
        })
        .map(|player| Event::CompanionRevealed { player })
        .collect()
}

/// Folds the reveal of the companion of `player`.
pub(crate) fn fold_revealed(state: &mut GameState, player: PlayerId) -> Result<(), EngineError> {
    match state.companions.slots.get_mut(&player) {
        Some(slot) if slot.stage == CompanionStage::Hidden => {
            slot.stage = CompanionStage::Revealed;
            Ok(())
        }
        _ => Err(invalid(format!(
            "{player:?} has no hidden companion to reveal"
        ))),
    }
}

/// Folds the companion of `player` into their hand as the object `object`: the special action is used up for the game.
pub(crate) fn fold_brought(
    state: &mut GameState,
    player: PlayerId,
    object: ObjectId,
) -> Result<(), EngineError> {
    let Some(slot) = state.companions.slots.get_mut(&player) else {
        return Err(invalid(format!("{player:?} has no companion")));
    };
    if slot.stage != CompanionStage::Revealed || slot.id != object {
        return Err(invalid(format!(
            "{player:?} cannot bring {object:?} into the game"
        )));
    }
    slot.stage = CompanionStage::Brought;
    let def = Arc::clone(&slot.def);
    if state.objects.contains_key(&object) || state.ceased.contains_key(&object) {
        return Err(invalid(format!("{object:?} already exists")));
    }
    crate::copies::note_definition(state, &def);
    state.objects.insert(
        object,
        GameObject::new(object, player, def, Zone::Hand(player)),
    );
    state
        .zones
        .insert(Zone::Hand(player), object, Placement::Top);
    state.next_object = state.next_object.max(object.get() + 1);
    crate::fold::reset_passes(state);
    Ok(())
}

/// The events of using the special action: the companion goes into the hand of `player`, between the life and the mana of
/// the payment.
pub(crate) fn events(state: &GameState, player: PlayerId) -> Option<Vec<Event>> {
    let slot = state.companions.slots.get(&player)?;
    (slot.stage == CompanionStage::Revealed).then(|| {
        vec![Event::CompanionBrought {
            player,
            object: slot.id,
        }]
    })
}

/// Whether `player` can bring their companion into their hand now: it is revealed and still outside the game, they could
/// cast a sorcery, and they can pay {3} with the mana that floats and what they control.
pub(crate) fn can_bring(
    state: &GameState,
    player: PlayerId,
    pool: &ManaPool,
    sources: &[ManaOption],
    life: i32,
) -> bool {
    state
        .companions
        .slots
        .get(&player)
        .is_some_and(|slot| slot.stage == CompanionStage::Revealed)
        && state.is_alive(player)
        && sorcery_timing(state, player)
        && can_afford(&action_cost(), pool, sources, life, &Spend::other())
}

/// The id of the companion `player` can still bring into the game, or none.
pub(crate) fn outside(state: &GameState, player: PlayerId) -> Option<ObjectId> {
    state
        .companions
        .slots
        .get(&player)
        .filter(|slot| slot.stage == CompanionStage::Revealed)
        .map(|slot| slot.id)
}

#[cfg(test)]
mod tests {
    //! Goal: the cost of the special action is {3}, the reveal and the bringing fold only from the stage that leads to them (hidden, revealed, brought), the reveals at the start of the game come in turn order from the first player and skip seats without a companion, and the hash tells the stages apart.
    use std::hash::{DefaultHasher, Hash, Hasher};

    use super::*;

    fn card() -> Arc<CardDefinition> {
        Arc::new(CardDefinition::new("Companion"))
    }

    fn companions(stage: CompanionStage, seats: &[usize]) -> Companions {
        let mut all = Companions::default();
        for (n, seat) in seats.iter().enumerate() {
            all.name(
                PlayerId::from_index(*seat),
                ObjectId(100 + n as u64),
                card(),
                stage,
            );
        }
        all
    }

    fn hash_of(companions: &Companions) -> u64 {
        let mut hasher = DefaultHasher::new();
        companions.hash(&mut hasher);
        hasher.finish()
    }

    #[test]
    fn the_action_costs_three_generic_mana() {
        assert_eq!(action_cost().to_string(), "{3}");
        assert_eq!(action_cost().mana_value(), 3);
    }

    #[test]
    fn the_hash_tells_the_stages_apart_and_the_seats() {
        let hidden = companions(CompanionStage::Hidden, &[0]);
        let revealed = companions(CompanionStage::Revealed, &[0]);
        let brought = companions(CompanionStage::Brought, &[0]);
        assert_ne!(hash_of(&hidden), hash_of(&revealed));
        assert_ne!(hash_of(&revealed), hash_of(&brought));
        assert_ne!(
            hash_of(&hidden),
            hash_of(&companions(CompanionStage::Hidden, &[1]))
        );
        assert_ne!(hash_of(&hidden), hash_of(&Companions::default()));
    }
}
