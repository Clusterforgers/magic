//! The faces of a card that has more than one (CR 708 to 715): which of them a player may cast or play, and the
//! state in which a face is tried.
//!
//! A card in a hand is its front face (both halves of a split card); the face is chosen as the card is cast or played
//! and the object is that face on the stack and the battlefield (`GameObject::face`). The options and the
//! announcement read a card as it will be once cast, so they run against [`GameState::with_face`]: the same state with
//! the card already in use as the face.

use std::borrow::Cow;

use mtg_cards::definition::Layout;
use mtg_core::card::CardType;
use mtg_core::ids::{ObjectId, PlayerId};
use mtg_core::mana::ManaCost;

use crate::ability_reader::definition_fuses;
use crate::announce::Granted;
use crate::events::Event;
use crate::mana::{ManaOption, ManaPool};
use crate::state::GameState;
use crate::ways;
use crate::zones::Zone;

impl GameState {
    /// This state with the card `id` in use as `face` (CR 601.2a): what the announcement of a cast and the options
    /// read, since the rules move the card to the stack before they ask anything. A card with one face, or the front
    /// face of an adventure or modal double-faced card (`None`), is the state itself.
    pub(crate) fn with_face(&self, id: ObjectId, face: Option<u8>) -> Cow<'_, GameState> {
        if face.is_none() {
            return Cow::Borrowed(self);
        }
        let mut view = self.clone();
        if let Some(object) = view.objects.get_mut(&id) {
            object.choices.face = face;
        }
        Cow::Owned(view)
    }
}

/// The state the replacement step reads a batch against: the cards that enter as one of their faces (a land played as
/// its back face, a card put onto the battlefield transformed) are already that face (CR 614.12, 712.14a).
pub(crate) fn entering_as<'a>(state: &'a GameState, proposed: &[Event]) -> Cow<'a, GameState> {
    let faces: Vec<(ObjectId, u8)> = proposed
        .iter()
        .filter_map(|event| match event {
            Event::LandPlayed {
                object,
                face: Some(face),
                ..
            } => Some((*object, *face)),
            // A permanent turned over is the same object with its face set; only a card that is not on the battlefield
            // is entering.
            Event::Transformed {
                object,
                face: Some(face),
            } if state
                .object(*object)
                .is_some_and(|card| card.permanent().is_none()) =>
            {
                Some((*object, *face))
            }
            _ => None,
        })
        .collect();
    if faces.is_empty() {
        return Cow::Borrowed(state);
    }
    let mut view = state.clone();
    for (id, face) in faces {
        if let Some(object) = view.objects.get_mut(&id) {
            object.choices.face = Some(face);
        }
    }
    Cow::Owned(view)
}

/// The faces of the card `id` that can be cast or played, front first: `None` is the card itself, and a split card has
/// its two halves instead. A card with one face is `[None]`.
pub(crate) fn candidates(state: &GameState, id: ObjectId) -> Vec<Option<u8>> {
    let Some(object) = state.object(id) else {
        return Vec::new();
    };
    let whole = object.whole_card();
    match whole.layout {
        // A transforming double-faced card is cast as its front face (CR 712.14); the back face is only turned up. A flip
        // card is its top half in every zone but the battlefield (CR 710.2).
        Layout::Normal | Layout::Transform | Layout::Flip | Layout::Prepare => vec![None],
        Layout::Split => {
            let halves = u8::try_from(whole.faces.len()).unwrap_or(u8::MAX);
            // A position past the last half is the card cast as all of them, from a hand only (CR 702.102a).
            let fused = matches!(object.zone(), Zone::Hand(_)) && definition_fuses(whole);
            (0..halves)
                .chain(fused.then_some(halves))
                .map(Some)
                .collect()
        }
        // The permanent card of an Adventure in exile is cast as that card alone (CR 715.4).
        Layout::Adventure if object.on_adventure().is_some() => vec![None],
        Layout::Adventure | Layout::ModalDfc => vec![None, Some(1)],
    }
}

/// Whether the card `id` is one of several faces.
pub(crate) fn is_multi_face(state: &GameState, id: ObjectId) -> bool {
    state
        .object(id)
        .is_some_and(|object| object.whole_card().is_multi_face())
}

/// The faces of the card `id` that `player` can cast now, with the mana that floats and the sources they control.
pub(crate) fn castable_faces(
    state: &GameState,
    player: PlayerId,
    id: ObjectId,
    pool: &ManaPool,
    sources: &[ManaOption],
    life: i32,
) -> Vec<Option<u8>> {
    candidates(state, id)
        .into_iter()
        .filter(|face| {
            let view = state.with_face(id, *face);
            ways::castable_as_is(&view, player, id, pool, sources, life)
        })
        .collect()
}

/// The faces of the card `id` that a resolution can have `player` cast.
pub(crate) fn grantable_faces(
    state: &GameState,
    player: PlayerId,
    id: ObjectId,
    granted: Granted,
) -> Vec<Option<u8>> {
    candidates(state, id)
        .into_iter()
        .filter(|face| {
            let view = state.with_face(id, *face);
            ways::grantable_as_is(&view, player, id, granted.free, granted.way)
        })
        .collect()
}

/// The faces of the card `id` that are lands (`None` is the card itself), for a card that may be played as one.
pub(crate) fn land_faces(state: &GameState, id: ObjectId) -> Vec<Option<u8>> {
    let Some(object) = state.object(id) else {
        return Vec::new();
    };
    if object.whole_card().layout != Layout::ModalDfc {
        return Vec::new();
    }
    candidates(state, id)
        .into_iter()
        .filter(|face| {
            object
                .whole_card()
                .face(face.unwrap_or(0))
                .is_type(CardType::Land)
        })
        .collect()
}

/// A face as the options list it: its name and what it costs.
pub(crate) fn describe(
    state: &GameState,
    id: ObjectId,
    face: Option<u8>,
) -> (String, Option<ManaCost>) {
    state.object(id).map_or_else(
        || (String::new(), None),
        |object| {
            let card = object.whole_card().face(face.unwrap_or(0));
            (card.name.clone(), card.mana_cost.clone())
        },
    )
}
