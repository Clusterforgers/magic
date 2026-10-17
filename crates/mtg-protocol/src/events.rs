//! Projecting the engine's event log for one viewer without leaking hidden cards.

use std::collections::BTreeSet;

use mtg_core::ids::{ObjectId, PlayerId};
use mtg_engine::apply_event;
use mtg_engine::events::Event;
use mtg_engine::state::GameState;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::error::ProtocolError;
use crate::project::zone_is_visible;
use crate::view::ObjectView;

/// What a viewer learns about an event that moved or revealed hidden cards: who and how many, never which.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
pub enum HiddenEvent {
    /// A player drew a card.
    CardDrawn {
        /// Who drew.
        player: PlayerId,
    },
    /// A player was dealt an opening hand.
    HandDealt {
        /// Who received it.
        player: PlayerId,
        /// Cards dealt.
        count: u32,
    },
    /// A library was shuffled; the new order is known to nobody.
    LibraryShuffled {
        /// Owner of the library.
        player: PlayerId,
    },
    /// A player shuffled their hand into their library to mulligan.
    MulliganTaken {
        /// Who mulliganed.
        player: PlayerId,
    },
    /// A player put cards on the bottom of their library.
    CardsBottomed {
        /// Whose cards.
        player: PlayerId,
        /// Cards placed.
        count: u32,
    },
    /// A card the viewer could not see left the game with its owner.
    ObjectLeftGame,
}

/// One log event as a viewer sees it; the list stays aligned one-to-one with the log.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
pub enum EventView {
    /// The event exactly as logged.
    Public(Event),
    /// The event with its hidden details removed.
    Hidden(HiddenEvent),
}

/// A run of consecutive events for one viewer plus the cards they reveal.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct EventBatch {
    /// Log position of the first event.
    pub from_seq: u64,
    /// The events in log order.
    pub events: Vec<EventView>,
    /// Views (as of the end of the batch) of visible objects the events touched.
    pub objects: Vec<ObjectView>,
}

fn count(len: usize) -> u32 {
    u32::try_from(len).unwrap_or(u32::MAX)
}

/// The hidden form of `event` for `viewer`, or `None` when they may see it as logged.
///
/// Every variant is listed so a new event forces a decision about what it reveals.
fn redact(before: &GameState, event: &Event, viewer: PlayerId) -> Option<HiddenEvent> {
    match event {
        Event::LibraryShuffled { player, .. } => {
            Some(HiddenEvent::LibraryShuffled { player: *player })
        }
        Event::MulliganTaken { player, .. } => Some(HiddenEvent::MulliganTaken { player: *player }),
        Event::HandDealt { player, cards } if *player != viewer => Some(HiddenEvent::HandDealt {
            player: *player,
            count: count(cards.len()),
        }),
        Event::CardsBottomed { player, cards } if *player != viewer => {
            Some(HiddenEvent::CardsBottomed {
                player: *player,
                count: count(cards.len()),
            })
        }
        Event::CardDrawn { player, .. } if *player != viewer => {
            Some(HiddenEvent::CardDrawn { player: *player })
        }
        Event::ObjectLeftGame { object } => {
            let seen = before
                .object(*object)
                .is_some_and(|o| zone_is_visible(o.zone(), viewer));
            (!seen).then_some(HiddenEvent::ObjectLeftGame)
        }
        Event::HandDealt { .. }
        | Event::CardsBottomed { .. }
        | Event::CardDrawn { .. }
        | Event::FirstPlayerChosen { .. }
        | Event::MulliganPhaseBegan
        | Event::MulliganDeclared { .. }
        | Event::MulliganKept { .. }
        | Event::MulliganPhaseEnded
        | Event::TurnBegan { .. }
        | Event::StepBegan { .. }
        | Event::NothingToDo { .. }
        | Event::PriorityGranted { .. }
        | Event::PriorityPassed { .. }
        | Event::LandPlayed { .. }
        | Event::SpellCast { .. }
        | Event::SpellResolved { .. }
        | Event::PermanentTapped { .. }
        | Event::PermanentUntapped { .. }
        | Event::ManaAdded { .. }
        | Event::ManaSpent { .. }
        | Event::ManaPoolEmptied { .. }
        | Event::DrewFromEmptyLibrary { .. }
        | Event::CardDiscarded { .. }
        | Event::AttackersDeclared { .. }
        | Event::BlockersDeclared { .. }
        | Event::DamageAssigned { .. }
        | Event::DamageDealt { .. }
        | Event::CombatEnded
        | Event::DamageCleared
        | Event::EffectCreated { .. }
        | Event::EffectExpired { .. }
        | Event::CreatureDied { .. }
        | Event::PlayerLost { .. }
        | Event::GameEnded { .. } => None,
    }
}

/// Projects `events` (which `before` has not yet seen) for each of `viewers`, one batch per viewer.
///
/// `from_seq` is the log position of the first event. The events are folded over a copy of
/// `before` once, so hidden-ness is judged against the zone each object was in at that moment.
pub fn project_events(
    before: &GameState,
    events: &[Event],
    viewers: &[PlayerId],
    from_seq: u64,
) -> Result<Vec<EventBatch>, ProtocolError> {
    let mut state = before.clone();
    let mut views: Vec<Vec<EventView>> = viewers.iter().map(|_| Vec::new()).collect();
    let mut touched = BTreeSet::<ObjectId>::new();
    for event in events {
        for (list, viewer) in views.iter_mut().zip(viewers) {
            list.push(match redact(&state, event, *viewer) {
                Some(hidden) => EventView::Hidden(hidden),
                None => EventView::Public(event.clone()),
            });
        }
        touched.extend(event.touched_objects());
        apply_event(&mut state, event)?;
    }
    Ok(viewers
        .iter()
        .zip(views)
        .map(|(viewer, events)| EventBatch {
            from_seq,
            events,
            objects: touched
                .iter()
                .filter_map(|id| ObjectView::of(&state, *viewer, *id))
                .collect(),
        })
        .collect())
}
