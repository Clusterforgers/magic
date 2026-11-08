//! `ReplaceBy::IntoLibrary` (CR 614.1a): an object that would move goes into its owner's library instead, on top, on
//! the bottom or shuffled in ("reveal it and shuffle it into its owner's library instead").

use mtg_cards::effect::Place;
use mtg_core::ids::ObjectId;

use super::{Item, Origin, Pass, Slot};
use crate::events::Event;
use crate::library::Revealed;
use crate::state::GameState;
use crate::zones::Zone;

impl Pass<'_> {
    /// Sends `object`, which would move from `from`, into its owner's library at `place` instead; a reveal of a card in a
    /// hidden zone comes first, and the bottoming or the shuffle right after the move.
    pub(super) fn g12a_into_library(
        &mut self,
        at: usize,
        (object, from): (ObjectId, Zone),
        (place, reveal): (Place, bool),
        applied: &[Origin],
    ) {
        let Some(owner) = self
            .state
            .object(object)
            .map(crate::object::GameObject::owner)
        else {
            self.slots[at].applied = applied.to_vec();
            return;
        };
        let library = Zone::Library(owner);
        let frozen = |event: Event| Slot {
            item: Item::One(event),
            applied: Vec::new(),
            frozen: true,
        };
        // A card already in the library stays where it is until the library is ordered.
        let after = if from == library {
            self.slots.remove(at);
            at
        } else {
            self.redirect(at, object, from, library, applied);
            let moved = (at..self.slots.len()).find(|slot| {
                matches!(&self.slots[*slot].item,
                    Item::One(Event::ZoneChanged { object: id, to, .. }) if *id == object && *to == library)
            });
            moved.map_or(at, |slot| slot + 1)
        };
        let follow = match place {
            Place::Bottom => Some(Event::LibraryBottomed {
                player: owner,
                cards: vec![object],
                random: false,
            }),
            // The order is drawn once the batch is known (`g12a_order_shuffles`).
            Place::Shuffled => Some(Event::LibraryShuffled {
                player: owner,
                order: Vec::new(),
            }),
            Place::Top | Place::Below(_) => None,
        };
        if let Some(event) = follow {
            self.slots
                .insert(after.min(self.slots.len()), frozen(event));
        }
        if reveal && matches!(from, Zone::Hand(_) | Zone::Library(_)) {
            self.slots.insert(
                at.min(self.slots.len()),
                frozen(Event::Revealed(Revealed {
                    player: owner,
                    cards: vec![object],
                })),
            );
        }
    }
}

/// Draws the order of each shuffle the batch left open (an empty order) from the library as the events before it leave
/// it; a batch that does not fold drops them, so the card stays on top unshuffled rather than stopping the game.
pub(super) fn g12a_order_shuffles(state: &GameState, events: &mut Vec<Event>) {
    let open =
        |event: &Event| matches!(event, Event::LibraryShuffled { order, .. } if order.is_empty());
    if !events.iter().any(open) {
        return;
    }
    let mut folded = state.clone();
    let mut failed = false;
    for event in events.iter_mut() {
        if let Event::LibraryShuffled { player, order } = event
            && order.is_empty()
        {
            let cards = folded.zone(Zone::Library(*player)).to_vec();
            *order = crate::mulligan::shuffled(&folded, 0, cards);
        }
        if crate::fold::apply_event(&mut folded, event).is_err() {
            failed = true;
            break;
        }
    }
    if failed {
        events.retain(|event| !open(event));
    }
}
