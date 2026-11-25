//! The cards whose pictures a viewer may load ahead of play (`ServerMessage::DeckPictures`): its own deck whole, every
//! deck whose list is public whole, and of every other seat only what the table shows anyway, so no hidden list can be
//! read from the requests. With them, each seat's list as the viewer may look through it, and the tokens it can make.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use mtg_cards::card::TokenSpec;
use mtg_cards::definition::Layout;
use mtg_cards::{CardDefinition, CardIdentity};
use mtg_core::card::{CardType, Subtype, Supertype};
use mtg_core::color::Color;
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

/// Where a card stands in a deck list: the command zone first, then by its main card type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, TS)]
pub enum ListGroup {
    /// A commander.
    Commander,
    /// The companion.
    Companion,
    /// A creature, an artifact creature included.
    Creature,
    /// A planeswalker.
    Planeswalker,
    /// A battle.
    Battle,
    /// An instant.
    Instant,
    /// A sorcery.
    Sorcery,
    /// An artifact that is no creature or land.
    Artifact,
    /// An enchantment that is no creature or artifact.
    Enchantment,
    /// A land, an artifact land included.
    Land,
    /// Anything else.
    Other,
}

/// One line of a deck list: a card and how many copies of it the deck holds.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct ListCard {
    /// The card's name.
    pub name: String,
    /// The card in the printing its line chose; none for a card without a record, which has no picture.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub identity: Option<CardIdentity>,
    /// The copies.
    pub count: u32,
    /// Its group in the list.
    pub group: ListGroup,
}

/// A token a deck can make, by the facts its picture is found by.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct TokenPicture {
    /// The token's name.
    pub name: String,
    /// Its colors; none is colorless.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    #[ts(optional, as = "Option<Vec<Color>>")]
    pub colors: Vec<Color>,
    /// Its supertypes.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    #[ts(optional, as = "Option<Vec<Supertype>>")]
    pub supertypes: Vec<Supertype>,
    /// Its card types.
    pub types: Vec<CardType>,
    /// Its subtypes.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    #[ts(optional, as = "Option<Vec<Subtype>>")]
    pub subtypes: Vec<Subtype>,
    /// Its power, for a creature.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub power: Option<i32>,
    /// Its toughness, for a creature.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub toughness: Option<i32>,
}

/// A seat's deck as a viewer may look through it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct SeatList {
    /// The seat.
    pub seat: PlayerId,
    /// Whether `cards` is the whole deck (the viewer's own, or a public list); else only its commanders and companion.
    pub whole: bool,
    /// The cards by group, then by name.
    pub cards: Vec<ListCard>,
    /// The tokens the listed cards make, each once, in the order the cards write them.
    pub tokens: Vec<TokenPicture>,
}

/// The pictures `viewer` may load ahead: its own commanders, companion and deck (every card it starts with), then the
/// commanders and companions of the other seats, which are public from the start (CR 903.6, 702.139), and the whole
/// deck of a seat in `public` (an unchanged precon, an open-deck table). A spectator gets only the public ones. Each card
/// once, in that order; a card without an identity has no picture.
pub fn deck_pictures(setup: &GameSetup, viewer: PlayerId, public: &[PlayerId]) -> Vec<PictureCard> {
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
            if public.contains(&PlayerId::from_index(at)) {
                own_cards(seat).for_each(&mut push);
            }
        }
    }
    out
}

/// Every seat's deck as `viewer` may look through it, in seat order: its own and each seat in `public` whole, every
/// other seat's commanders and companion alone; each with the tokens its listed cards make. A seat with nothing to
/// show is left out.
pub fn deck_lists(setup: &GameSetup, viewer: PlayerId, public: &[PlayerId]) -> Vec<SeatList> {
    setup
        .players
        .iter()
        .enumerate()
        .filter_map(|(at, seat)| {
            let id = PlayerId::from_index(at);
            let whole = at == viewer.index() || public.contains(&id);
            let list = list_of(seat, id, whole);
            (!list.cards.is_empty() || !list.tokens.is_empty()).then_some(list)
        })
        .collect()
}

/// A line of a list by its group, name and picture (oracle and printing id), so copies of one printing count together.
type LineKey = (ListGroup, String, Option<(String, Option<String>)>);

/// The list of one seat: whole, or its commanders and companion alone.
fn list_of(seat: &PlayerSetup, id: PlayerId, whole: bool) -> SeatList {
    let mut lines: BTreeMap<LineKey, u32> = BTreeMap::new();
    let mut tokens = Vec::new();
    let mut add = |card: &Arc<CardDefinition>, group: ListGroup| {
        let key = card
            .identity
            .as_ref()
            .map(|identity| (identity.oracle_id.clone(), identity.printing_id.clone()));
        *lines.entry((group, card.name.clone(), key)).or_insert(0) += 1;
        for spec in mtg_engine::manual_tokens::made_by(card).iter() {
            let picture = picture_of(spec);
            if !tokens.contains(&picture) {
                tokens.push(picture);
            }
        }
    };
    seat.commanders
        .iter()
        .for_each(|card| add(card, ListGroup::Commander));
    if let Some(companion) = &seat.companion {
        add(companion, ListGroup::Companion);
    }
    if whole {
        own_cards(seat).for_each(|card| add(card, group_of(card)));
    }
    let cards = lines
        .into_iter()
        .map(|((group, name, key), count)| ListCard {
            name,
            identity: key.map(|(oracle_id, printing_id)| CardIdentity {
                oracle_id,
                printing_id,
            }),
            count,
            group,
        })
        .collect();
    SeatList {
        seat: id,
        whole,
        cards,
        tokens,
    }
}

/// The group of a card of the deck by its main card type: a creature or a land before its other types.
fn group_of(card: &CardDefinition) -> ListGroup {
    const ORDER: [(CardType, ListGroup); 8] = [
        (CardType::Creature, ListGroup::Creature),
        (CardType::Land, ListGroup::Land),
        (CardType::Planeswalker, ListGroup::Planeswalker),
        (CardType::Battle, ListGroup::Battle),
        (CardType::Instant, ListGroup::Instant),
        (CardType::Sorcery, ListGroup::Sorcery),
        (CardType::Artifact, ListGroup::Artifact),
        (CardType::Enchantment, ListGroup::Enchantment),
    ];
    ORDER
        .iter()
        .find(|(kind, _)| card.types.contains(kind))
        .map_or(ListGroup::Other, |(_, group)| *group)
}

/// The facts a token's picture is found by.
fn picture_of(spec: &TokenSpec) -> TokenPicture {
    TokenPicture {
        name: spec.name.clone(),
        colors: spec.colors.clone(),
        supertypes: spec.supertypes.clone(),
        types: spec.types.clone(),
        subtypes: spec.subtypes.clone(),
        power: spec.power,
        toughness: spec.toughness,
    }
}

/// The cards of a seat every player sees from the start: its commanders and its companion.
fn public_cards(seat: &PlayerSetup) -> impl Iterator<Item = &Arc<CardDefinition>> {
    seat.commanders.iter().chain(&seat.companion)
}

/// The other cards a seat starts with, which only its owner knows as a list unless the list is public.
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
