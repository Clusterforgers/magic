//! The cards whose pictures a viewer may load ahead of play (`ServerMessage::DeckPictures`): its own deck whole, and
//! of every other seat only what the table shows anyway, so no hidden list can be read from the requests.

use std::collections::BTreeSet;
use std::sync::Arc;

use mtg_cards::definition::Layout;
use mtg_cards::{CardDefinition, CardIdentity};
use mtg_core::ids::PlayerId;
use mtg_engine::setup::{GameSetup, PlayerSetup};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// A card whose picture a client may load before the table shows it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct PictureCard {
    /// The card, in the printing its deck line chose.
    pub identity: CardIdentity,
    /// How many faces have a picture of their own: 2 for a double-faced card, else 1.
    pub faces: u8,
}

/// The pictures `viewer` may load ahead: its own commanders, companion and deck (every card it starts with), then the
/// commanders and companions of the other seats, which are public from the start (CR 903.6, 702.139). A spectator
/// gets only the public ones. Each card once, in that order; a card without an identity has no picture.
pub fn deck_pictures(setup: &GameSetup, viewer: PlayerId) -> Vec<PictureCard> {
    let mut seen = BTreeSet::new();
    let mut out = Vec::new();
    let mut push = |card: &Arc<CardDefinition>| {
        let Some(identity) = card.identity.clone() else {
            return;
        };
        if seen.insert((identity.oracle_id.clone(), identity.printing_id.clone())) {
            out.push(PictureCard {
                identity,
                faces: faces_of(card),
            });
        }
    };
    if let Some(own) = setup.players.get(viewer.index()) {
        public_cards(own).for_each(&mut push);
        own_cards(own).for_each(&mut push);
    }
    for (at, seat) in setup.players.iter().enumerate() {
        if at != viewer.index() {
            public_cards(seat).for_each(&mut push);
        }
    }
    out
}

/// The cards of a seat every player sees from the start: its commanders and its companion.
fn public_cards(seat: &PlayerSetup) -> impl Iterator<Item = &Arc<CardDefinition>> {
    seat.commanders.iter().chain(&seat.companion)
}

/// The other cards a seat starts with, which only its owner knows as a list.
fn own_cards(seat: &PlayerSetup) -> impl Iterator<Item = &Arc<CardDefinition>> {
    seat.library
        .iter()
        .chain(&seat.hand)
        .chain(seat.battlefield.iter().map(|preset| &preset.card))
        .chain(&seat.graveyard)
        .chain(seat.attractions.iter().map(|attraction| &attraction.card))
        .chain(&seat.satchel)
        .chain(&seat.satchel_reusable)
}

/// The faces of a card with a picture each: both sides of a double-faced card; a split, Adventure, flip or prepare card
/// is one picture.
fn faces_of(card: &CardDefinition) -> u8 {
    match card.layout {
        Layout::ModalDfc | Layout::Transform => u8::try_from(card.faces.len().max(1)).unwrap_or(1),
        _ => 1,
    }
}
