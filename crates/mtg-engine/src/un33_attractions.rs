//! Attractions (CR 717): the Attraction deck a seat brings, face down in its command zone; opening an Attraction
//! (CR 701.51); rolling to visit (CR 701.52) as the precombat main phase begins (CR 505.5) or by an effect; the lights
//! a visit ability reads (CR 702.159a); and the junkyard, the face-up pile in the command zone an Attraction card goes
//! to in place of a hand, a library or a graveyard (CR 717.6).

use std::collections::BTreeMap;
use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_core::attraction::Lights;
use mtg_core::card::Subtype;
use mtg_core::ids::{ObjectId, PlayerId};

use crate::characteristics::seen_as;
use crate::error::EngineError;
use crate::events::{Event, ZoneCause};
use crate::fold::invalid;
use crate::object::GameObject;
use crate::state::{GameState, Tally};
use crate::zones::Zone;

/// One card of an Attraction deck: the card and the numbers lit up on its printing.
#[derive(Debug, Clone)]
pub struct AttractionCard {
    /// The card.
    pub card: Arc<CardDefinition>,
    /// The numbers lit up on it.
    pub lights: Lights,
}

/// The Attractions of a game: each seat's Attraction deck (top card last) and the lights of every Attraction card.
#[derive(Debug, Clone, Default, PartialEq, Eq, Hash)]
pub struct Attractions {
    pub(crate) decks: BTreeMap<PlayerId, Vec<ObjectId>>,
    pub(crate) lights: BTreeMap<ObjectId, Lights>,
    /// The answers given so far to the questions of the turn-based roll to visit while it asks (CR 706.2b, 706.6).
    pub(crate) r706_visit: Option<Vec<(PlayerId, crate::choose::Choice)>>,
}

impl Attractions {
    /// Whether no seat plays with Attractions.
    pub fn is_empty(&self) -> bool {
        self.lights.is_empty()
    }

    /// Puts the card `id` with these lights on top of `owner`'s Attraction deck (setup).
    pub(crate) fn add(&mut self, owner: PlayerId, id: ObjectId, lights: Lights) {
        self.decks.entry(owner).or_default().push(id);
        self.lights.insert(id, lights);
    }
}

/// The Attraction subtype.
fn attraction() -> Subtype {
    Subtype("Attraction".to_owned())
}

impl GameState {
    /// The cards of the player's Attraction deck, top card last.
    pub fn attraction_deck(&self, player: PlayerId) -> &[ObjectId] {
        self.un33.decks.get(&player).map_or(&[], Vec::as_slice)
    }

    /// The numbers lit up on the object: an Attraction card a seat brought (CR 717.1); `None` for any other object, a
    /// token or copy of an Attraction included.
    pub fn lights(&self, id: ObjectId) -> Option<Lights> {
        self.un33.lights.get(&id).copied()
    }

    /// Whether the object lies in an Attraction deck.
    pub fn in_attraction_deck(&self, id: ObjectId) -> bool {
        self.un33.decks.values().any(|deck| deck.contains(&id))
    }
}

/// Whether the permanent is an Attraction now: it prints the subtype or an effect gave it.
fn is_attraction(state: &GameState, object: &GameObject) -> bool {
    seen_as(state, object.id())
        .is_some_and(|(chars, _)| chars.has_subtype(&object.card().subtypes, &attraction()))
}

/// The Attractions the player controls with `result` lit up: the ones a roll of `result` visits (CR 701.52a).
fn visited(state: &GameState, player: PlayerId, result: u32) -> Vec<ObjectId> {
    state
        .zone(Zone::Battlefield)
        .iter()
        .filter_map(|id| state.object(*id))
        .filter(|object| {
            object.controller() == player
                && is_attraction(state, object)
                && state
                    .lights(object.id())
                    .is_some_and(|lights| lights.lit(result))
        })
        .map(GameObject::id)
        .collect()
}

/// The shuffle of every Attraction deck before the game begins (CR 103.3a), `offset` being the first substream of the
/// batch left to them; a seat without one shuffles nothing.
pub(crate) fn shuffles(state: &GameState, offset: u64) -> Vec<Event> {
    state
        .un33
        .decks
        .iter()
        .filter(|(_, deck)| !deck.is_empty())
        .zip(offset..)
        .map(|((player, deck), offset)| Event::AttractionDeckShuffled {
            player: *player,
            order: crate::mulligan::shuffled(state, offset, deck.clone()),
        })
        .collect()
}

/// "Open an Attraction" `count` times (CR 701.51b): the top cards of the player's Attraction deck, one move each onto
/// the battlefield under the player's control; a deck runs out quietly.
pub(crate) fn open_events(state: &GameState, player: PlayerId, count: usize) -> Vec<Event> {
    state
        .attraction_deck(player)
        .iter()
        .rev()
        .take(count)
        .map(|object| Event::ZoneChanged {
            object: *object,
            from: Zone::Command(player),
            to: Zone::Battlefield,
            cause: ZoneCause::Moved,
        })
        .collect()
}

/// The player rolls to visit their Attractions (CR 701.52a): a six-sided die from the game's random stream, changed by
/// the rules that change a roll with the answers of `journal` (`r6402_change_roll`), then the roll and the Attractions
/// it visits. `Err` is the next question.
pub(crate) fn roll_events(
    state: &GameState,
    player: PlayerId,
    journal: &[(PlayerId, crate::choose::Choice)],
) -> Result<Vec<Event>, Box<crate::resolve::Run>> {
    let natural = crate::resolve::prim::roll_dice::rolls(state, player, 6, 1);
    let changed = match crate::resolve::prim::r6402_change_roll::r6402_change_in(
        (state, journal),
        player,
        6,
        &natural,
        true,
    ) {
        Ok(Ok(changed)) => changed,
        Ok(Err(ask)) => return Err(Box::new(ask)),
        // An answer that doesn't fit leaves the roll as it fell.
        Err(_) => crate::resolve::prim::r6402_change_roll::R6402Changed::new(&natural),
    };
    let result = changed.results.last().copied().unwrap_or(0);
    let mut events = changed.events.clone();
    events.extend(changed.rolled(player, 6));
    events.push(Event::RolledToVisit {
        player,
        result,
        visited: visited(state, player, result),
    });
    events.extend(crate::phasing::r112_visit_releases(state, player, result));
    Ok(events)
}

/// What the turn-based roll to visit comes to.
pub(crate) enum Visit {
    /// The events of the roll, none when the player controls no Attraction.
    Rolled(Vec<Event>),
    /// The roll asks: before the first question the step pauses (`Event::R706VisitAsks`), then each question is asked.
    Asks(Box<Option<crate::decision::Decision>>),
}

/// The turn-based roll as the active player's precombat main phase begins, when they control an Attraction (CR 505.5),
/// with the answers given so far while it asks.
pub(crate) fn precombat(state: &GameState) -> Visit {
    let Some(active) = state.active_player() else {
        return Visit::Rolled(Vec::new());
    };
    let controls = state
        .zone(Zone::Battlefield)
        .iter()
        .filter_map(|id| state.object(*id))
        .any(|object| object.controller() == active && is_attraction(state, object));
    if !controls {
        return Visit::Rolled(Vec::new());
    }
    let journal = state.un33.r706_visit.as_deref();
    match roll_events(state, active, journal.unwrap_or_default()) {
        Ok(events) => Visit::Rolled(events),
        Err(run) => match *run {
            crate::resolve::Run::Ask(decision) if journal.is_some() => {
                Visit::Asks(Box::new(Some(decision)))
            }
            _ => Visit::Asks(Box::new(None)),
        },
    }
}

/// Whether the turn-based roll to visit is asking its questions.
pub(crate) fn r706_visiting(state: &GameState) -> bool {
    state.un33.r706_visit.is_some()
}

/// Folds an answer to a question of the turn-based roll to visit.
pub(crate) fn r706_fold_answer(
    state: &mut GameState,
    player: PlayerId,
    choice: &crate::choose::Choice,
) -> Result<(), EngineError> {
    state
        .un33
        .r706_visit
        .as_mut()
        .ok_or_else(|| invalid("an answer outside any resolution or roll"))?
        .push((player, choice.clone()));
    Ok(())
}

/// Folds a shuffle of an Attraction deck: the same cards in a new order.
pub(crate) fn fold_shuffled(
    state: &mut GameState,
    player: PlayerId,
    order: &[ObjectId],
) -> Result<(), EngineError> {
    let deck = state.un33.decks.entry(player).or_default();
    let mut same = order.to_vec();
    same.sort_unstable();
    let mut held = deck.clone();
    held.sort_unstable();
    if same != held {
        return Err(invalid(format!(
            "{player:?} shuffles other cards into their Attraction deck"
        )));
    }
    deck.clone_from(&order.to_vec());
    Ok(())
}

/// Folds a roll to visit: each Attraction visited counts for the player this turn.
pub(crate) fn fold_rolled(state: &mut GameState, player: PlayerId, visited: &[ObjectId]) {
    state.tally_by(
        player,
        Tally::AttractionsVisited,
        u32::try_from(visited.len()).unwrap_or(u32::MAX),
    );
    state.un33.r706_visit = None;
}

/// A card that leaves the command zone leaves its owner's Attraction deck.
pub(crate) fn fold_left(state: &mut GameState, object: ObjectId, from: Zone) {
    if let Zone::Command(owner) = from
        && let Some(deck) = state.un33.decks.get_mut(&owner)
    {
        deck.retain(|id| *id != object);
    }
}

/// Where an Attraction card goes in place of `to` (CR 717.6): its owner's command zone (the junkyard) in place of a
/// hand, a library or a graveyard; so does a Contraption card, to its scrapheap there (Unstable); `None` for any other
/// object or zone.
pub(crate) fn junkyard(state: &GameState, object: ObjectId, to: Zone) -> Option<Zone> {
    if (state.lights(object).is_none()
        && !crate::p170b_contraptions::is_contraption_card(state, object))
        || !matches!(to, Zone::Hand(_) | Zone::Library(_) | Zone::Graveyard(_))
    {
        return None;
    }
    state.object(object).map(|card| Zone::Command(card.owner()))
}
