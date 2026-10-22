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

use crate::announce::Granted;
use crate::mana::{ManaOption, ManaPool};
use crate::state::GameState;
use crate::ways;

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

/// The faces of the card `id` that can be cast or played, front first: `None` is the card itself, and a split card has
/// its two halves instead. A card with one face is `[None]`.
pub(crate) fn candidates(state: &GameState, id: ObjectId) -> Vec<Option<u8>> {
    let Some(object) = state.object(id) else {
        return Vec::new();
    };
    let whole = object.whole_card();
    match whole.layout {
        Layout::Normal => vec![None],
        Layout::Split => (0..whole.faces.len())
            .filter_map(|face| u8::try_from(face).ok().map(Some))
            .collect(),
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
