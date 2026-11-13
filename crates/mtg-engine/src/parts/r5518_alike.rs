//! Picks of a cost whose cards must be alike as a set: "two nonland cards with the same name", "two cards from your hand
//! that share a color".

use mtg_cards::cost::Alike;
use mtg_core::color::{Color, ColorSet};
use mtg_core::ids::ObjectId;

use crate::state::GameState;

/// The name of a card, none for a face-down one or one that is gone.
fn r5518_name(state: &GameState, id: ObjectId) -> Option<&str> {
    state
        .object(id)
        .filter(|object| object.face_down().is_none())
        .map(|object| object.card().name.as_str())
}

/// The colors of a card as it is now, none for one that is gone.
fn r5518_colors(state: &GameState, id: ObjectId) -> ColorSet {
    crate::characteristics::characteristics(state, id)
        .map_or(ColorSet::default(), |chars| chars.colors)
}

/// Whether `cards` are alike as `alike` says: all of one name, or all of one color among them.
pub(super) fn r5518_alike(state: &GameState, cards: &[ObjectId], alike: Alike) -> bool {
    match alike {
        Alike::SameName => {
            let mut names = cards.iter().map(|id| r5518_name(state, *id));
            names
                .next()
                .is_none_or(|first| first.is_some() && names.all(|name| name == first))
        }
        Alike::ShareColor => Color::ALL.into_iter().any(|color| {
            cards
                .iter()
                .all(|id| r5518_colors(state, *id).contains(color))
        }),
    }
}

/// Whether `count` of the `candidates` can be picked alike.
pub(super) fn r5518_has_set(
    state: &GameState,
    candidates: &[ObjectId],
    count: usize,
    alike: Alike,
) -> bool {
    if count == 0 {
        return true;
    }
    match alike {
        Alike::SameName => candidates.iter().any(|first| {
            r5518_name(state, *first).is_some_and(|name| {
                candidates
                    .iter()
                    .filter(|id| r5518_name(state, **id) == Some(name))
                    .count()
                    >= count
            })
        }),
        Alike::ShareColor => Color::ALL.into_iter().any(|color| {
            candidates
                .iter()
                .filter(|id| r5518_colors(state, **id).contains(color))
                .count()
                >= count
        }),
    }
}

/// The words a question adds for the set ("with the same name").
pub(super) const fn r5518_words(alike: Alike) -> &'static str {
    match alike {
        Alike::SameName => " with the same name",
        Alike::ShareColor => " that share a color",
    }
}
