//! Projecting the engine's event log for one viewer without leaking hidden cards.

use mtg_core::ids::{ObjectId, PlayerId};
use mtg_engine::abilities::Cause;
use mtg_engine::apply_event;
use mtg_engine::events::{Event, ZoneCause};
use mtg_engine::manual::{ManualAction, ManualZone};
use mtg_engine::object::ObjectRef;
use mtg_engine::snapshot::Snapshot;
use mtg_engine::state::GameState;
use mtg_engine::zones::Zone;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::diff::{ObjectDiff, diff};
use crate::error::ProtocolError;
use crate::project::{visible_objects, zone_is_visible_in};
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
    /// A player's graveyard was shuffled into their library in place of losing to an empty library; the new order
    /// is known to nobody.
    LibraryRefilled {
        /// Owner of the library.
        player: PlayerId,
        /// Cards the library holds now.
        count: u32,
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
    /// A player surveilled and chose which cards stay on top, which only they know; the cards put into the graveyard
    /// follow as moves everyone sees.
    Surveilled {
        /// Who surveilled.
        player: PlayerId,
        /// Cards that stay on top.
        top: u32,
        /// Cards put into the graveyard.
        graveyard: u32,
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
    /// A triggered ability triggered on a card the viewer cannot see (a card drawn, say): which ability, never the card.
    AbilityTriggered {
        /// The object with the ability, as it was.
        source: ObjectRef,
        /// Its index among the rules of the source's card.
        ability: usize,
        /// Who controls it.
        controller: PlayerId,
    },
    /// A triggered ability went on the stack for a card the viewer cannot see.
    AbilityPutOnStack {
        /// The ability's new object.
        object: ObjectId,
        /// The object it comes from, as it was.
        source: ObjectRef,
        /// Who controls it.
        controller: PlayerId,
    },
    /// A waiting triggered ability was removed; what it triggered on is a card the viewer cannot see.
    TriggerRemoved {
        /// The object with the ability, as it was.
        source: ObjectRef,
        /// Its index among the rules of the source's card.
        ability: usize,
    },
    /// A delayed triggered ability was set up with a card the viewer cannot see among the names it took.
    DelayedTriggerCreated {
        /// The object whose spell or ability set it up.
        source: ObjectRef,
        /// Who controls it.
        controller: PlayerId,
    },
    /// A player moved a card by hand between places the viewer cannot see into, so which card is withheld.
    ManualMove {
        /// Who moved it.
        player: PlayerId,
        /// Where they put it.
        to: ManualZone,
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
#[allow(clippy::too_many_lines)]
fn redact(before: &GameState, event: &Event, viewer: PlayerId) -> Option<HiddenEvent> {
    // Every seat sees every hand, so what is dealt, bottomed or drawn is told whole.
    let own = |player: &PlayerId| *player == viewer || before.ruleset().open_hands();
    match event {
        Event::LibraryShuffled { player, .. } => {
            Some(HiddenEvent::LibraryShuffled { player: *player })
        }
        Event::LibraryRefilled { player, order } => Some(HiddenEvent::LibraryRefilled {
            player: *player,
            count: count(order.len()),
        }),
        Event::MulliganTaken { player, .. } => Some(HiddenEvent::MulliganTaken { player: *player }),
        Event::HandDealt { player, cards } if !own(player) => Some(HiddenEvent::HandDealt {
            player: *player,
            count: count(cards.len()),
        }),
        Event::CardsBottomed { player, cards } if !own(player) => {
            Some(HiddenEvent::CardsBottomed {
                player: *player,
                count: count(cards.len()),
            })
        }
        Event::CardDrawn { player, .. } if !own(player) => {
            Some(HiddenEvent::CardDrawn { player: *player })
        }
        Event::ObjectLeftGame { object } => {
            (!knows(before, *object, viewer)).then_some(HiddenEvent::ObjectLeftGame)
        }
        // The cards are in a library by now, which nobody sees into: the seat that put them there learned which
        // from the events that moved or showed them, the others learn how many.
        Event::LibraryBottomed { player, cards, .. } => Some(HiddenEvent::CardsBottomed {
            player: *player,
            count: count(cards.len()),
        }),
        // What the effects of spells and abilities make known: some of it is private to a player or to cards.
        Event::LibraryLooked(_)
        | Event::CardsFound(_)
        | Event::Scried(_)
        | Event::Surveilled(_)
        | Event::ZoneChanged { .. }
        | Event::ChoiceMade { .. }
        | Event::StepDone { .. } => redact_effects(before, event, viewer),
        // A card moved by hand is named unless it goes between places the viewer cannot see into, as `ZoneChanged` does.
        Event::ManualAction { player, action } => redact_manual(before, *player, action, viewer),
        // A triggered ability can be about a card that only some seats can see.
        Event::AbilityTriggered(_)
        | Event::AbilityPutOnStack(_)
        | Event::TriggerRemoved { .. }
        | Event::DelayedTriggerCreated(_) => redact_triggers(before, event, viewer),
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
        | Event::ManaBurned { .. }
        | Event::LifePaid { .. }
        | Event::DrewFromEmptyLibrary { .. }
        | Event::CardDiscarded { .. }
        | Event::Cycled { .. }
        | Event::AttackersDeclared { .. }
        | Event::BlockersDeclared { .. }
        | Event::DamageAssigned { .. }
        | Event::DamageDealt { .. }
        | Event::CombatEnded
        | Event::DamageCleared
        | Event::Regenerated { .. }
        | Event::ShieldUsed { .. }
        | Event::EffectCreated { .. }
        | Event::EffectExpired { .. }
        | Event::RuleCreated { .. }
        | Event::RuleExpired { .. }
        | Event::CreatureDied { .. }
        | Event::PlayerLost { .. }
        | Event::GameEnded { .. }
        // The ability on the stack is public, and so is what was announced for it.
        | Event::AbilityActivated { .. }
        | Event::TriggersOrdered { .. }
        | Event::DelayedTriggerExpired { .. }
        | Event::ResolutionBegan { .. }
        | Event::ResolutionEnded { .. }
        | Event::LifeGained { .. }
        | Event::LifeLost { .. }
        | Event::CountersChanged(_)
        | Event::TokenCreated(_)
        | Event::BecameCopy(_)
        // A copy of a spell or ability on the stack shows what the stack shows of the original, and no more.
        | Event::SpellCopied(_)
        | Event::AbilityCopied(_)
        | Event::CopyEnded { .. }
        | Event::Entered { .. }
        | Event::Attached { .. }
        | Event::AttachedToPlayer { .. }
        | Event::Unattached { .. }
        | Event::JoinedAttack { .. }
        | Event::Transformed { .. }
        | Event::DayNightChanged { .. }
        | Event::MonarchChanged { .. }
        | Event::CitysBlessingGained { .. }
        | Event::Learned { .. }
        | Event::Proliferated { .. }
        | Event::Explored { .. }
        | Event::Connived { .. }
        | Event::Amassed { .. }
        | Event::Monstrous { .. }
        | Event::TurnedFaceUp { .. }
        | Event::Mutated { .. }
        | Event::Adapted { .. }
        | Event::Bolstered { .. }
        | Event::CommanderStayed { .. }
        | Event::Revealed(_) => None,
    }
}

/// The hidden form of a manual action: a card moved between places the viewer cannot see into is not named, as the
/// `ZoneChanged` that follows is not; no other action names a card the viewer cannot see.
fn redact_manual(
    before: &GameState,
    player: PlayerId,
    action: &ManualAction,
    viewer: PlayerId,
) -> Option<HiddenEvent> {
    let ManualAction::Move { object, to } = action else {
        return None;
    };
    let moved = before.object(*object)?;
    let from_known = identified_in(before, *object, moved.zone(), viewer);
    let goes_unseen = !zone_is_visible_in(before, to.zone(moved.owner()), viewer);
    (!from_known && goes_unseen).then_some(HiddenEvent::ManualMove { player, to: *to })
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
        Event::Surveilled(surveilled) if surveilled.player != viewer => {
            Some(HiddenEvent::Surveilled {
                player: surveilled.player,
                top: count(surveilled.top.len()),
                graveyard: count(surveilled.graveyard.len()),
            })
        }
        // A card that moves between places the viewer cannot see into is not told apart, and neither is one that
        // lies face down at either end and that the viewer may not look at.
        Event::ZoneChanged {
            object,
            from,
            to,
            cause,
        } if !identified_in(before, *object, *from, viewer)
            && (!zone_is_visible_in(before, *to, viewer)
                || !arrives_known(before, *object, *cause, viewer)) =>
        {
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
                .all(|shot| snapshot_known(before, shot, viewer)) =>
        {
            Some(HiddenEvent::StepDone {
                position: position.clone(),
            })
        }
        _ => None,
    }
}

/// The hidden form of an event about a triggered ability, when what it triggered on is a card the viewer cannot see.
///
/// A card is judged by the zone it is in as the event happens and by the zone it was in when it was photographed for
/// the ability, so a card drawn is hidden from the other seats whether or not it has been played since.
fn redact_triggers(before: &GameState, event: &Event, viewer: PlayerId) -> Option<HiddenEvent> {
    let unseen_cause = |cause: &Cause| {
        cause
            .snapshots
            .iter()
            .any(|shot| !snapshot_known(before, shot, viewer))
            || cause
                .subjects
                .iter()
                .filter_map(|subject| subject.object())
                .any(|object| !knows(before, object.id, viewer))
    };
    match event {
        Event::AbilityTriggered(t)
            if !snapshot_known(before, &t.snapshot, viewer) || unseen_cause(&t.cause) =>
        {
            Some(HiddenEvent::AbilityTriggered {
                source: t.source,
                ability: t.ability,
                controller: t.controller,
            })
        }
        Event::AbilityPutOnStack(put)
            if put.cause.as_ref().is_some_and(|cause| {
                !snapshot_known(before, &put.snapshot, viewer) || unseen_cause(cause)
            }) =>
        {
            Some(HiddenEvent::AbilityPutOnStack {
                object: put.object,
                source: put.source,
                controller: put.controller,
            })
        }
        Event::TriggerRemoved {
            source,
            ability,
            cause,
            ..
        } if unseen_cause(cause) => Some(HiddenEvent::TriggerRemoved {
            source: *source,
            ability: *ability,
        }),
        Event::DelayedTriggerCreated(created)
            if !snapshot_known(before, &created.snapshot, viewer)
                || created
                    .bindings
                    .iter()
                    .flat_map(|(_, targets)| targets)
                    .filter_map(|target| target.object())
                    .any(|object| !knows(before, object.id, viewer)) =>
        {
            Some(HiddenEvent::DelayedTriggerCreated {
                source: created.source,
                controller: created.controller,
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
        .is_some_and(|o| identified_in(before, object, o.zone(), viewer))
        || before.is_shown_to(viewer, object)
}

/// Whether the zone `zone` is open to `viewer` and no face-down rule hides the object `object` there from them.
fn identified_in(before: &GameState, object: ObjectId, zone: Zone, viewer: PlayerId) -> bool {
    zone_is_visible_in(before, zone, viewer) && before.may_look_at(viewer, object)
}

/// Whether `viewer` can tell which card arrives from a move for `cause`: not when it lies face down after the move and
/// they may not look at it.
fn arrives_known(before: &GameState, object: ObjectId, cause: ZoneCause, viewer: PlayerId) -> bool {
    cause.face_down(before.turn().number()).is_none_or(|how| {
        before
            .object(object)
            .is_some_and(|o| how.may_look(o.owner(), o.controller(), viewer))
    })
}

/// Whether `viewer` could identify the object a snapshot describes where it was photographed.
fn snapshot_known(before: &GameState, shot: &Snapshot, viewer: PlayerId) -> bool {
    zone_is_visible_in(before, shot.zone, viewer)
        && shot.face_down.is_none_or(|how| {
            how.shows_characteristics() || how.may_look(shot.owner, shot.controller, viewer)
        })
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

#[cfg(test)]
mod tests {
    //! Goal: every event of the effect pipeline has its visibility decided: what a player learns by looking or by choosing among cards the others cannot see is shown to that player only (the others get counts or the position), a card moving between places nobody can see into is not named, and everything else is public.
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use mtg_cards::trigger::TriggerPattern;
    use mtg_cards::value::CounterKind;
    use mtg_core::ids::ObjectId;
    use mtg_engine::abilities::{Cause, PutOnStack};
    use mtg_engine::choose::{Choice, Choices, Target};
    use mtg_engine::counters::CountersChanged;
    use mtg_engine::events::{ResolutionOutcome, ZoneCause};
    use mtg_engine::library::{CardsFound, LibraryLooked, Revealed, Scried, Surveilled};
    use mtg_engine::snapshot::snapshot;
    use mtg_engine::tokens::TokenCreated;
    use mtg_engine::triggers::{DelayedCreated, DelayedId, Triggered};
    use mtg_testkit::cards::{bear, forest, free_creature, giant};
    use mtg_testkit::{Scenario, ready};

    use super::*;

    const OWNER: PlayerId = PlayerId(0);
    const OTHER: PlayerId = PlayerId(1);

    /// Seat 0 has a card in hand and one in the library, and a creature on the battlefield; seat 1 has a creature too.
    struct Table {
        state: GameState,
        hand: ObjectId,
        library: ObjectId,
        creature: ObjectId,
    }

    fn table() -> Table {
        let state = Scenario::new(2)
            .hand(0, [bear()])
            .library(0, [giant(), forest()])
            .battlefield(0, [ready(free_creature("Mine", 1, 1))])
            .battlefield(1, [ready(free_creature("Theirs", 2, 2))])
            .build()
            .unwrap()
            .state()
            .clone();
        let hand = state.zone(Zone::Hand(OWNER))[0];
        let library = *state.zone(Zone::Library(OWNER)).last().unwrap();
        let creature = state.zone(Zone::Battlefield)[0];
        Table {
            state,
            hand,
            library,
            creature,
        }
    }

    /// What `viewer` is told of `event`: `Err` with the hidden form, `Ok` when it is the event as logged.
    fn told(state: &GameState, event: &Event, viewer: PlayerId) -> Result<(), HiddenEvent> {
        redact(state, event, viewer).map_or(Ok(()), Err)
    }

    #[test]
    fn what_a_player_learned_by_looking_is_theirs_and_the_others_get_counts() {
        let t = table();
        let looked = Event::LibraryLooked(LibraryLooked {
            player: OWNER,
            cards: vec![t.library, t.hand],
        });
        let found = Event::CardsFound(CardsFound {
            player: OWNER,
            zone: Zone::Library(OWNER),
            cards: vec![t.library],
        });
        let scried = Event::Scried(Scried {
            player: OWNER,
            top: vec![t.library],
            bottom: vec![t.hand],
        });
        let surveilled = Event::Surveilled(Surveilled {
            player: OWNER,
            top: vec![t.library],
            graveyard: vec![t.hand],
        });
        for event in [&looked, &found, &scried, &surveilled] {
            assert_eq!(told(&t.state, event, OWNER), Ok(()), "{event:?}");
        }
        assert_eq!(
            told(&t.state, &surveilled, OTHER),
            Err(HiddenEvent::Surveilled {
                player: OWNER,
                top: 1,
                graveyard: 1
            })
        );
        assert_eq!(
            told(&t.state, &looked, OTHER),
            Err(HiddenEvent::LibraryLooked {
                player: OWNER,
                count: 2
            })
        );
        assert_eq!(
            told(&t.state, &found, OTHER),
            Err(HiddenEvent::CardsFound {
                player: OWNER,
                count: 1
            })
        );
        assert_eq!(
            told(&t.state, &scried, OTHER),
            Err(HiddenEvent::Scried {
                player: OWNER,
                top: 1,
                bottom: 1
            })
        );
    }

    #[test]
    fn a_card_that_moves_is_named_to_those_who_can_see_either_end_of_the_move() {
        let t = table();
        let moved = |object, from, to| Event::ZoneChanged {
            object,
            from,
            to,
            cause: ZoneCause::Moved,
        };
        // Library to hand (a search): only the owner sees where it goes.
        let found = moved(t.library, Zone::Library(OWNER), Zone::Hand(OWNER));
        assert_eq!(told(&t.state, &found, OWNER), Ok(()));
        assert_eq!(
            told(&t.state, &found, OTHER),
            Err(HiddenEvent::ZoneChanged {
                from: Zone::Library(OWNER),
                to: Zone::Hand(OWNER),
                cause: ZoneCause::Moved
            })
        );
        // Hand to library (put back): the owner knows which card; the other seat sees neither zone.
        let back = moved(t.hand, Zone::Hand(OWNER), Zone::Library(OWNER));
        assert_eq!(told(&t.state, &back, OWNER), Ok(()));
        assert!(told(&t.state, &back, OTHER).is_err());
        // The battlefield and graveyards are open to both.
        let dies = moved(t.creature, Zone::Battlefield, Zone::Graveyard(OWNER));
        assert_eq!(told(&t.state, &dies, OWNER), Ok(()));
        assert_eq!(told(&t.state, &dies, OTHER), Ok(()));
        // Hand to graveyard (a discard): everyone sees it arrive.
        let discard = moved(t.hand, Zone::Hand(OWNER), Zone::Graveyard(OWNER));
        assert_eq!(told(&t.state, &discard, OTHER), Ok(()));
        // Library to graveyard (a mill): the graveyard is open, so every seat sees which card was milled.
        let milled = Event::ZoneChanged {
            object: t.library,
            from: Zone::Library(OWNER),
            to: Zone::Graveyard(OWNER),
            cause: ZoneCause::Milled,
        };
        assert_eq!(told(&t.state, &milled, OWNER), Ok(()));
        assert_eq!(told(&t.state, &milled, OTHER), Ok(()));
    }

    #[test]
    fn an_answer_about_cards_is_shown_to_those_who_can_see_the_cards_and_to_whoever_gave_it() {
        let t = table();
        let choose = |player, cards: Vec<ObjectId>| Event::ChoiceMade {
            player,
            choice: Choice::Cards(cards),
        };
        // Choosing among your own hand: you know, the other seat does not.
        let hand = choose(OWNER, vec![t.hand]);
        assert_eq!(told(&t.state, &hand, OWNER), Ok(()));
        assert_eq!(
            told(&t.state, &hand, OTHER),
            Err(HiddenEvent::ChoiceMade { player: OWNER })
        );
        // The other seat choosing among your hand is not told to you either, but the chooser knows what they chose.
        let order = Event::ChoiceMade {
            player: OTHER,
            choice: Choice::Order(vec![t.library]),
        };
        assert_eq!(told(&t.state, &order, OTHER), Ok(()));
        assert!(told(&t.state, &order, OWNER).is_err());
        // Cards everyone can see, and answers that name no card, are public.
        assert_eq!(
            told(&t.state, &choose(OWNER, vec![t.creature]), OTHER),
            Ok(())
        );
        for choice in [
            Choice::YesNo(true),
            Choice::Number(3),
            Choice::Option(1),
            Choice::Replacement(0),
        ] {
            let event = Event::ChoiceMade {
                player: OWNER,
                choice,
            };
            assert_eq!(told(&t.state, &event, OTHER), Ok(()), "{event:?}");
        }
    }

    #[test]
    fn a_step_is_shown_whole_unless_it_bound_or_moved_cards_the_viewer_cannot_see() {
        let t = table();
        let step = |bound: Vec<(String, Vec<Target>)>, snapshots| Event::StepDone {
            position: vec![2],
            bound,
            snapshots,
        };
        let named = |object: ObjectId| {
            vec![(
                "x".to_owned(),
                vec![Target::Object(t.state.object(object).unwrap().reference())],
            )]
        };
        assert_eq!(told(&t.state, &step(Vec::new(), Vec::new()), OTHER), Ok(()));
        assert_eq!(
            told(&t.state, &step(named(t.creature), Vec::new()), OTHER),
            Ok(())
        );
        let hidden = step(named(t.hand), Vec::new());
        assert_eq!(told(&t.state, &hidden, OWNER), Ok(()));
        assert_eq!(
            told(&t.state, &hidden, OTHER),
            Err(HiddenEvent::StepDone { position: vec![2] })
        );
        // A player named in a binding is always known.
        let player = vec![("p".to_owned(), vec![Target::Player(OWNER)])];
        assert_eq!(told(&t.state, &step(player, Vec::new()), OTHER), Ok(()));
        // An object moved away from a place the viewer cannot see into is not shown either.
        let shot = snapshot(&t.state, t.hand).unwrap();
        assert!(told(&t.state, &step(Vec::new(), vec![shot]), OTHER).is_err());
        let shot = snapshot(&t.state, t.creature).unwrap();
        assert_eq!(told(&t.state, &step(Vec::new(), vec![shot]), OTHER), Ok(()));
    }

    #[test]
    fn the_rest_of_the_pipeline_is_public() {
        let t = table();
        let source = t.state.object(t.creature).unwrap().reference();
        let events = [
            Event::AbilityPutOnStack(Box::new(PutOnStack {
                origin: mtg_engine::abilities::AbilitySource::default(),
                object: ObjectId(900),
                source,
                snapshot: snapshot(&t.state, t.creature).unwrap(),
                ability: 0,
                controller: OWNER,
                choices: Choices::default(),
                cause: None,
                delayed: None,
            })),
            Event::AbilityActivated {
                player: OWNER,
                source,
                ability: 0,
                origin: mtg_engine::abilities::AbilitySource::default(),
            },
            Event::AbilityTriggered(Box::new(Triggered {
                origin: mtg_engine::abilities::AbilitySource::default(),
                source,
                ability: 0,
                controller: OWNER,
                cause: Cause::default(),
                snapshot: snapshot(&t.state, t.creature).unwrap(),
                delayed: None,
            })),
            Event::TriggersOrdered {
                player: OWNER,
                order: vec![1, 0],
            },
            Event::DelayedTriggerExpired { id: DelayedId(0) },
            Event::ResolutionBegan {
                object: ObjectId(900),
            },
            Event::ResolutionEnded {
                object: ObjectId(900),
                outcome: ResolutionOutcome::Fizzled,
            },
            Event::LifeGained {
                player: OWNER,
                amount: 2,
            },
            Event::LifeLost {
                player: OTHER,
                amount: 2,
            },
            Event::CountersChanged(CountersChanged {
                on: Target::Player(OTHER),
                counter: CounterKind::Poison,
                delta: 1,
            }),
            Event::TokenCreated(TokenCreated {
                object: ObjectId(901),
                copy_of: None,
                controller: OWNER,
                name: "Treasure".into(),
                tapped: false,
                attacking: None,
                definition: std::sync::Arc::new(mtg_cards::CardDefinition::new("Treasure")),
            }),
            Event::Revealed(Revealed {
                player: OWNER,
                cards: vec![t.hand],
            }),
            Event::SpellCast {
                player: OWNER,
                object: t.hand,
                choices: Choices::default(),
            },
        ];
        for event in &events {
            for viewer in [OWNER, OTHER] {
                assert_eq!(told(&t.state, event, viewer), Ok(()), "{event:?}");
            }
        }
    }

    #[test]
    fn a_cycled_card_is_public_like_the_discard_it_accompanies() {
        let t = table();
        for event in [
            Event::CardDiscarded {
                player: OWNER,
                object: t.hand,
            },
            Event::Cycled {
                player: OWNER,
                object: t.hand,
            },
        ] {
            for viewer in [OWNER, OTHER] {
                assert_eq!(told(&t.state, &event, viewer), Ok(()), "{event:?}");
            }
        }
    }

    /// A trigger of the creature in play, about `subject`.
    fn triggered_on(t: &Table, subject: ObjectId) -> Triggered {
        let source = t.state.object(t.creature).unwrap().reference();
        let reference = t.state.object(subject).unwrap().reference();
        Triggered {
            origin: mtg_engine::abilities::AbilitySource::default(),
            source,
            ability: 1,
            controller: OWNER,
            cause: Cause {
                subjects: vec![Target::Object(reference)],
                snapshots: vec![snapshot(&t.state, subject).unwrap()],
                amount: 0,
            },
            snapshot: snapshot(&t.state, t.creature).unwrap(),
            delayed: None,
        }
    }

    #[test]
    fn a_trigger_about_a_card_only_its_owner_can_see_is_shown_to_the_others_without_the_card() {
        let t = table();
        let source = t.state.object(t.creature).unwrap().reference();
        let in_hand = triggered_on(&t, t.hand);
        let on_the_table = triggered_on(&t, t.creature);

        let event = Event::AbilityTriggered(Box::new(in_hand.clone()));
        assert_eq!(told(&t.state, &event, OWNER), Ok(()));
        assert_eq!(
            told(&t.state, &event, OTHER),
            Err(HiddenEvent::AbilityTriggered {
                source,
                ability: 1,
                controller: OWNER
            })
        );
        let event = Event::AbilityTriggered(Box::new(on_the_table.clone()));
        for viewer in [OWNER, OTHER] {
            assert_eq!(told(&t.state, &event, viewer), Ok(()));
        }

        let put = |trigger: &Triggered| {
            Event::AbilityPutOnStack(Box::new(PutOnStack {
                origin: mtg_engine::abilities::AbilitySource::default(),
                object: ObjectId(900),
                source,
                snapshot: trigger.snapshot.clone(),
                ability: 1,
                controller: OWNER,
                choices: Choices::default(),
                cause: Some(trigger.cause.clone()),
                delayed: None,
            }))
        };
        assert_eq!(told(&t.state, &put(&in_hand), OWNER), Ok(()));
        assert_eq!(
            told(&t.state, &put(&in_hand), OTHER),
            Err(HiddenEvent::AbilityPutOnStack {
                object: ObjectId(900),
                source,
                controller: OWNER
            })
        );
        assert_eq!(told(&t.state, &put(&on_the_table), OTHER), Ok(()));

        let removed = |trigger: &Triggered| Event::TriggerRemoved {
            source,
            ability: 1,
            origin: mtg_engine::abilities::AbilitySource::default(),
            delayed: None,
            cause: trigger.cause.clone(),
        };
        assert_eq!(told(&t.state, &removed(&in_hand), OWNER), Ok(()));
        assert_eq!(
            told(&t.state, &removed(&in_hand), OTHER),
            Err(HiddenEvent::TriggerRemoved { source, ability: 1 })
        );
        assert_eq!(told(&t.state, &removed(&on_the_table), OTHER), Ok(()));
    }

    #[test]
    fn a_trigger_keeps_hiding_the_card_it_photographed_in_a_hand_once_it_is_on_the_table() {
        let mut t = table();
        let in_hand = triggered_on(&t, t.hand);
        let played = Event::LandPlayed {
            player: OWNER,
            object: t.hand,
            face: None,
        };
        apply_event(&mut t.state, &played).unwrap();
        let event = Event::AbilityTriggered(Box::new(in_hand));
        assert!(told(&t.state, &event, OTHER).is_err());
        assert_eq!(told(&t.state, &event, OWNER), Ok(()));
    }

    #[test]
    fn a_delayed_trigger_that_took_a_hidden_card_is_shown_without_it() {
        let t = table();
        let source = t.state.object(t.creature).unwrap().reference();
        let created = |bound: ObjectId| {
            let reference = t.state.object(bound).unwrap().reference();
            Event::DelayedTriggerCreated(Box::new(DelayedCreated {
                id: DelayedId(0),
                source,
                snapshot: snapshot(&t.state, t.creature).unwrap(),
                controller: OWNER,
                ability: 0,
                when: TriggerPattern::AnyOf(Vec::new()),
                once: true,
                effects: Vec::new(),
                bindings: vec![("card".to_owned(), vec![Target::Object(reference)])],
            }))
        };
        assert_eq!(told(&t.state, &created(t.hand), OWNER), Ok(()));
        assert_eq!(
            told(&t.state, &created(t.hand), OTHER),
            Err(HiddenEvent::DelayedTriggerCreated {
                source,
                controller: OWNER
            })
        );
        assert_eq!(told(&t.state, &created(t.creature), OTHER), Ok(()));
    }
}
