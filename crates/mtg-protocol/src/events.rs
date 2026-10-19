//! Projecting the engine's event log for one viewer without leaking hidden cards.

use mtg_core::ids::{ObjectId, PlayerId};
use mtg_engine::apply_event;
use mtg_engine::events::{Event, ZoneCause};
use mtg_engine::state::GameState;
use mtg_engine::zones::Zone;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::diff::{ObjectDiff, diff};
use crate::error::ProtocolError;
use crate::project::{visible_objects, zone_is_visible};
use crate::view::{ObjectView, TableView};

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
    /// A player looked at cards in a library; which, only they know.
    LibraryLooked {
        /// Who looked.
        player: PlayerId,
        /// Cards looked at.
        count: u32,
    },
    /// A player found cards by searching; which, only they know until they are revealed.
    CardsFound {
        /// Who searched.
        player: PlayerId,
        /// Cards found.
        count: u32,
    },
    /// A player scried and chose which cards stay on top, which only they know.
    Scried {
        /// Who scried.
        player: PlayerId,
        /// Cards that stay on top.
        top: u32,
        /// Cards put on the bottom.
        bottom: u32,
    },
    /// A card moved between zones the viewer cannot see into, so which card is withheld.
    ZoneChanged {
        /// Where it was.
        from: Zone,
        /// Where it went.
        to: Zone,
        /// Why.
        cause: ZoneCause,
    },
    /// A player answered a question about cards the viewer cannot see; the answer is withheld.
    ChoiceMade {
        /// Who answered.
        player: PlayerId,
    },
    /// A step of a resolution that bound or moved cards the viewer cannot see; only where it went on is shown.
    StepDone {
        /// Where the next step runs.
        position: Vec<usize>,
    },
}

/// One log event as a viewer sees it; the list stays aligned one-to-one with the log.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
pub enum EventView {
    /// The event exactly as logged.
    Public(Event),
    /// The event with its hidden details removed.
    Hidden(HiddenEvent),
}

/// A run of consecutive events for one viewer and the state they leave behind.
///
/// The batch is self-sufficient: replacing the table, upserting `objects` and dropping `gone` turns
/// the viewer's view before it into their view after it, with no need to interpret `events`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct EventBatch {
    /// Log position of the first event.
    pub from_seq: u64,
    /// The events in log order.
    pub events: Vec<EventView>,
    /// The whole table after the last event; replaces the client's copy.
    // Boxed so that `ServerMessage::Events` does not make every message that large.
    pub table: Box<TableView>,
    /// Visible objects after the batch that are new to the viewer or differ from how they saw them before.
    ///
    /// The batch at `from_seq` 0 lists every visible object, because no view precedes it.
    pub objects: Vec<ObjectView>,
    /// Objects the viewer could see before the batch and cannot see now.
    pub gone: Vec<ObjectId>,
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
            (!knows(before, *object, viewer)).then_some(HiddenEvent::ObjectLeftGame)
        }
        // What the effects of spells and abilities make known: some of it is private to a player or to cards.
        Event::LibraryLooked(_)
        | Event::CardsFound(_)
        | Event::Scried(_)
        | Event::ZoneChanged { .. }
        | Event::ChoiceMade { .. }
        | Event::StepDone { .. } => redact_effects(before, event, viewer),
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
        | Event::LifePaid { .. }
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
        | Event::GameEnded { .. }
        // The ability on the stack is public, and so is what was announced for it.
        | Event::AbilityPutOnStack(_)
        | Event::AbilityActivated { .. }
        | Event::AbilityTriggered(_)
        | Event::ResolutionBegan { .. }
        | Event::ResolutionEnded { .. }
        | Event::LifeGained { .. }
        | Event::LifeLost { .. }
        | Event::CountersChanged(_)
        | Event::TokenCreated(_)
        | Event::Revealed(_) => None,
    }
}

/// The hidden form of an event of the effect pipeline that may name cards or learn a player something private.
fn redact_effects(before: &GameState, event: &Event, viewer: PlayerId) -> Option<HiddenEvent> {
    match event {
        // What a player learned by looking is theirs alone.
        Event::LibraryLooked(looked) if looked.player != viewer => {
            Some(HiddenEvent::LibraryLooked {
                player: looked.player,
                count: count(looked.cards.len()),
            })
        }
        Event::CardsFound(found) if found.player != viewer => Some(HiddenEvent::CardsFound {
            player: found.player,
            count: count(found.cards.len()),
        }),
        Event::Scried(scried) if scried.player != viewer => Some(HiddenEvent::Scried {
            player: scried.player,
            top: count(scried.top.len()),
            bottom: count(scried.bottom.len()),
        }),
        // A card that moves between places the viewer cannot see into is not told apart.
        Event::ZoneChanged {
            from, to, cause, ..
        } if !zone_is_visible(*from, viewer) && !zone_is_visible(*to, viewer) => {
            Some(HiddenEvent::ZoneChanged {
                from: *from,
                to: *to,
                cause: *cause,
            })
        }
        // Whoever answers knows the answer; the others know it when everything it names is open to them.
        Event::ChoiceMade { player, choice }
            if *player != viewer
                && !choice.objects().iter().all(|id| knows(before, *id, viewer)) =>
        {
            Some(HiddenEvent::ChoiceMade { player: *player })
        }
        // A step is shown whole when every card it bound and every object it moved away is open to the viewer.
        Event::StepDone {
            position,
            bound,
            snapshots,
        } if !bound
            .iter()
            .flat_map(|(_, targets)| targets)
            .filter_map(|target| target.object())
            .all(|object| knows(before, object.id, viewer))
            || !snapshots
                .iter()
                .all(|shot| zone_is_visible(shot.zone, viewer)) =>
        {
            Some(HiddenEvent::StepDone {
                position: position.clone(),
            })
        }
        _ => None,
    }
}

/// Whether `viewer` may identify the object in the state it is in before the event; an object that is not
/// there is not known.
fn knows(before: &GameState, object: ObjectId, viewer: PlayerId) -> bool {
    before
        .object(object)
        .is_some_and(|o| zone_is_visible(o.zone(), viewer))
}

/// Projects `events` (which `before` has not yet seen) for each of `viewers`, one batch per viewer.
///
/// `from_seq` is the log position of the first event. The events are folded over a copy of
/// `before` once, so hidden-ness is judged against the zone each object was in at that moment.
/// What changed is read off each viewer's view of `before` and of the folded state, never guessed
/// from the events: a continuous effect can change an object that no event names.
pub fn project_events(
    before: &GameState,
    events: &[Event],
    viewers: &[PlayerId],
    from_seq: u64,
) -> Result<Vec<EventBatch>, ProtocolError> {
    let mut state = before.clone();
    let mut views: Vec<Vec<EventView>> = viewers.iter().map(|_| Vec::new()).collect();
    for event in events {
        for (list, viewer) in views.iter_mut().zip(viewers) {
            list.push(match redact(&state, event, *viewer) {
                Some(hidden) => EventView::Hidden(hidden),
                None => EventView::Public(event.clone()),
            });
        }
        apply_event(&mut state, event)?;
    }
    Ok(viewers
        .iter()
        .zip(views)
        .map(|(viewer, events)| {
            let seen = visible_objects(before, *viewer);
            let now = visible_objects(&state, *viewer);
            // Nothing precedes the first batch, so a client that joined before the game starts needs every object.
            let ObjectDiff { objects, gone } = diff(&seen, &now, from_seq == 0);
            EventBatch {
                from_seq,
                events,
                table: Box::new(TableView::of(&state, *viewer)),
                objects,
                gone,
            }
        })
        .collect())
}
