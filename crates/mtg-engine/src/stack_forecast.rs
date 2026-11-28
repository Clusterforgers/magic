//! The stack forecast: what the top spell or ability of the stack would change if it resolved right now.
//!
//! Like the combat forecast it is derived data, worked out on a copy of the state with the engine's own code and never
//! stored: `stack::resolve_top` begins the resolution, `resolve::next` runs its steps, every batch passes the
//! replacement step (`replace_dry`) and the fold, and state-based actions settle after it. The copy is then compared
//! with the state: power and toughness, damage, counters, tapping, control and where a permanent or spell went, and each
//! player's life and counters.
//!
//! Every seat reads the same forecast, so it exists only when the public state alone decides it: a resolution that asks
//! a player anything (its own questions or the replacement step's), draws a random number, or touches a hidden card
//! (draws, looks, reveals, searches, a card that leaves a hand or library, a face-down card turned up) has none.

use std::collections::BTreeMap;

use mtg_cards::value::CounterKind;
use mtg_core::ids::{ObjectId, PlayerId};
use serde::{Deserialize, Serialize};

use crate::characteristics::{characteristics, controller_of_object};
use crate::combat::DamageTarget;
use crate::events::{Event, ResolutionOutcome, ZoneCause};
use crate::flow;
use crate::fold::apply_event;
use crate::replace_dry::{Picks, settle};
use crate::sba;
use crate::state::{GameState, Stage};
use crate::zones::Zone;

/// The most batches one resolution and the state-based actions after it may take; this bounds a bug, not a game.
const MAX_BATCHES: usize = 256;

/// Who or what a forecast line is about.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub enum Forecasted {
    /// A player.
    Player(PlayerId),
    /// A permanent or a spell or ability on the stack.
    Object(ObjectId),
}

/// Where a permanent or a spell goes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub enum Fate {
    /// Destroyed by the effect.
    Destroyed,
    /// Put into the graveyard by state-based actions (lethal damage, deathtouch, 0 toughness).
    Dies,
    /// Sacrificed.
    Sacrificed,
    /// Exiled.
    Exiled,
    /// Returned to its owner's hand.
    Hand,
    /// Put into its owner's library.
    Library,
    /// Put into a graveyard any other way.
    Graveyard,
    /// A spell or ability countered.
    Countered,
}

/// One change the resolution makes to one object or player.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub enum Change {
    /// Damage dealt to a permanent, after prevention.
    Damage {
        /// How much.
        amount: u32,
    },
    /// A creature's power and toughness, each as (now, after).
    Stats {
        /// Power now and after.
        power: (i32, i32),
        /// Toughness now and after.
        toughness: (i32, i32),
    },
    /// The counters of one kind, as (now, after).
    Counters {
        /// The kind.
        kind: CounterKind,
        /// How many now and after.
        count: (u32, u32),
    },
    /// The permanent becomes tapped.
    Tapped,
    /// The permanent becomes untapped.
    Untapped,
    /// Another player gains control of the permanent.
    Control {
        /// The new controller.
        controller: PlayerId,
    },
    /// The permanent or spell leaves its zone.
    Leaves {
        /// How and where to.
        fate: Fate,
    },
    /// A player's life total, before and after.
    Life {
        /// Life now.
        life: i32,
        /// Life after.
        life_after: i32,
    },
}

/// One line of a stack forecast: a change to one object or player.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct ForecastChange {
    /// Who or what changes.
    pub subject: Forecasted,
    /// The change.
    pub change: Change,
}

/// What the top item of the stack would change if it resolved now, players by seat first and then objects by id; `None`
/// when nothing resolves next, when nothing public changes or when the result is not the public state's alone (see the
/// module docs).
pub fn stack_forecast(state: &GameState) -> Option<Vec<ForecastChange>> {
    if state.stage() != Stage::Turns || state.resolving().is_some() {
        return None;
    }
    let top = *state.stack().last()?;
    if state.object(top)?.face_down().is_some() {
        return None;
    }
    let mut run = Resolution::new(state);
    run.commit(&crate::stack::resolve_top(state)?)?;
    run.resolve()?;
    run.settle()?;
    if run.picks.asked().iter().any(|&options| options > 1)
        || run.after.rng_index() != state.rng_index()
        || run.events.iter().any(|event| hidden(state, event))
        || hidden_zone_changed(state, &run.after)
    {
        return None;
    }
    let changes = changes(state, &run, top);
    (!changes.is_empty()).then_some(changes)
}

/// The resolution played out on a copy of the state.
struct Resolution {
    /// The copy.
    after: GameState,
    /// Every event folded onto it, as the replacement step left them.
    events: Vec<Event>,
    /// The answers given to the questions of the replacement step.
    picks: Picks,
}

impl Resolution {
    fn new(state: &GameState) -> Self {
        Resolution {
            after: state.clone(),
            events: Vec::new(),
            picks: Picks::default(),
        }
    }

    /// Sends `events` through the replacement step and folds what comes out; `None` if that fails.
    fn commit(&mut self, events: &[Event]) -> Option<()> {
        let events = settle(&mut self.after, events, &mut self.picks).ok()?;
        let mut fresh = crate::batch_ids::FreshIds::default();
        for event in events {
            let event = fresh.renamed(&self.after, &event).unwrap_or(event);
            apply_event(&mut self.after, &event).ok()?;
            self.events.push(event);
        }
        Some(())
    }

    /// Runs the steps of the resolution to its end; `None` when one asks a question or cannot run.
    fn resolve(&mut self) -> Option<()> {
        for _ in 0..MAX_BATCHES {
            if self.after.resolving().is_none() {
                return Some(());
            }
            match crate::resolve::next(&self.after) {
                flow::Progress::Events(events) => self.commit(&events)?,
                _ => return None,
            }
        }
        None
    }

    /// Plays the state-based actions the game runs after the resolution; the end of the game stops them.
    fn settle(&mut self) -> Option<()> {
        for _ in 0..MAX_BATCHES {
            if self.after.stage() == Stage::Finished {
                return Some(());
            }
            let mut events = sba::departures_first(&self.after);
            if events.is_empty() {
                events = sba::game_over(&self.after).into_iter().collect();
            }
            if events.is_empty() {
                events = sba::check(&self.after);
            }
            if events.is_empty() {
                return Some(());
            }
            self.commit(&events)?;
        }
        None
    }
}

/// Whether `event` shows, moves or reorders a card some seat may not see, or draws on chance.
fn hidden(state: &GameState, event: &Event) -> bool {
    match event {
        Event::ZoneChanged { object, from, .. } => {
            matches!(from, Zone::Library(_) | Zone::Hand(_))
                || state
                    .object(*object)
                    .is_some_and(|object| object.face_down().is_some())
        }
        Event::LibraryShuffled { .. }
        | Event::CardDrawn { .. }
        | Event::DrewFromEmptyLibrary { .. }
        | Event::R109DrawReplaced { .. }
        | Event::LibraryRefilled { .. }
        | Event::CardDiscarded { .. }
        | Event::LibraryLooked(_)
        | Event::HandLooked(_)
        | Event::CardsFound(_)
        | Event::Scried(_)
        | Event::Surveilled(_)
        | Event::Revealed(_)
        | Event::ChosenAtRandom { .. }
        | Event::PlayersChosenAtRandom { .. }
        | Event::KindChosenAtRandom { .. }
        | Event::Sought { .. }
        | Event::LibraryBottomed { .. }
        | Event::LibraryInserted { .. }
        | Event::Discovered { .. }
        | Event::DreadManifested { .. }
        | Event::Foraged { .. }
        | Event::DieRolled { .. }
        | Event::CoinFlipped { .. }
        | Event::Explored { .. }
        | Event::Clashed { .. }
        | Event::Connived { .. }
        | Event::TurnedFaceUp { .. }
        | Event::FaceDownLooked { .. }
        | Event::AttractionDeckShuffled { .. }
        | Event::P170bContraptionDeckShuffled { .. }
        | Event::RolledToVisit { .. } => true,
        _ => false,
    }
}

/// Whether a card left a hand or library, or a library was put in another order.
fn hidden_zone_changed(state: &GameState, after: &GameState) -> bool {
    state.players().iter().any(|player| {
        [Zone::Library(player.id()), Zone::Hand(player.id())]
            .into_iter()
            .any(|zone| {
                let before = state.zone(zone);
                let kept: Vec<ObjectId> = after
                    .zone(zone)
                    .iter()
                    .copied()
                    .filter(|id| before.contains(id))
                    .collect();
                kept != before
            })
    })
}

/// The lines of the forecast: players by seat, then the permanents and stack objects of `state` by id.
fn changes(state: &GameState, run: &Resolution, top: ObjectId) -> Vec<ForecastChange> {
    let mut lines = Vec::new();
    for player in state.players() {
        let id = player.id();
        let Some(later) = run.after.player(id) else {
            continue;
        };
        let mut push = |change| {
            lines.push(ForecastChange {
                subject: Forecasted::Player(id),
                change,
            });
        };
        if later.life() != player.life() {
            push(Change::Life {
                life: player.life(),
                life_after: later.life(),
            });
        }
        for (kind, from, to) in counter_changes(player.counters(), later.counters()) {
            push(Change::Counters {
                kind,
                count: (from, to),
            });
        }
    }
    let damage = damage_by_object(&run.events);
    let mut ids: Vec<ObjectId> = state
        .zone(Zone::Battlefield)
        .iter()
        .chain(state.stack())
        .copied()
        .filter(|&id| id != top)
        .collect();
    ids.sort_unstable();
    for id in ids {
        let mut push = |change| {
            lines.push(ForecastChange {
                subject: Forecasted::Object(id),
                change,
            });
        };
        if let Some(&amount) = damage.get(&id) {
            push(Change::Damage { amount });
        }
        if let Some(fate) = fate(&run.events, id) {
            push(Change::Leaves { fate });
            continue;
        }
        let (Some(now), Some(later)) = (state.object(id), run.after.object(id)) else {
            continue;
        };
        if now.zone() != Zone::Battlefield || later.zone() != Zone::Battlefield {
            continue;
        }
        let (Some(was), Some(will)) = (now.permanent(), later.permanent()) else {
            continue;
        };
        if let Some(stats) = stats(state, &run.after, id) {
            push(stats);
        }
        for (kind, from, to) in counter_changes(was.counters(), will.counters()) {
            push(Change::Counters {
                kind,
                count: (from, to),
            });
        }
        match (was.tapped(), will.tapped()) {
            (false, true) => push(Change::Tapped),
            (true, false) => push(Change::Untapped),
            _ => {}
        }
        let controller = controller_of_object(&run.after, later);
        if controller != controller_of_object(state, now) {
            push(Change::Control { controller });
        }
    }
    lines
}

/// The power and toughness of the creature `id` before and after, when they differ.
fn stats(state: &GameState, after: &GameState, id: ObjectId) -> Option<Change> {
    let pt = |state: &GameState| {
        let chars = characteristics(state, id)?;
        Some((chars.power?, chars.toughness?))
    };
    let (from, to) = (pt(state)?, pt(after)?);
    (from != to).then_some(Change::Stats {
        power: (from.0, to.0),
        toughness: (from.1, to.1),
    })
}

/// The kinds whose count differs, with the counts before and after, in the kinds' order.
fn counter_changes<'a>(
    now: impl Iterator<Item = (&'a CounterKind, u32)>,
    later: impl Iterator<Item = (&'a CounterKind, u32)>,
) -> Vec<(CounterKind, u32, u32)> {
    let mut counts: BTreeMap<CounterKind, (u32, u32)> = BTreeMap::new();
    for (kind, count) in now {
        counts.entry(kind.clone()).or_default().0 = count;
    }
    for (kind, count) in later {
        counts.entry(kind.clone()).or_default().1 = count;
    }
    counts
        .into_iter()
        .filter(|(_, (from, to))| from != to)
        .map(|(kind, (from, to))| (kind, from, to))
        .collect()
}

/// The damage each object was dealt, after prevention.
fn damage_by_object(events: &[Event]) -> BTreeMap<ObjectId, u32> {
    let mut dealt: BTreeMap<ObjectId, u32> = BTreeMap::new();
    for event in events {
        if let Event::DamageDealt {
            target: DamageTarget::Object(object),
            amount,
            ..
        } = event
        {
            let total = dealt.entry(*object).or_default();
            *total = total.saturating_add(*amount);
        }
    }
    dealt
}

/// How the object `id` leaves the battlefield or the stack, by the first event that moves it.
fn fate(events: &[Event], id: ObjectId) -> Option<Fate> {
    events.iter().find_map(|event| match event {
        Event::CreatureDied { object, .. } if *object == id => Some(Fate::Dies),
        Event::ResolutionEnded {
            object,
            outcome: ResolutionOutcome::Countered,
        } if *object == id => Some(Fate::Countered),
        Event::ZoneChanged {
            object,
            from: Zone::Battlefield | Zone::Stack,
            to,
            cause,
        } if *object == id => match (cause, to) {
            (ZoneCause::Destroyed | ZoneCause::Buried, _) => Some(Fate::Destroyed),
            (ZoneCause::StateBased(_), _) => Some(Fate::Dies),
            (ZoneCause::Sacrificed, _) => Some(Fate::Sacrificed),
            (ZoneCause::Countered, _) => Some(Fate::Countered),
            (_, Zone::Exile) => Some(Fate::Exiled),
            (_, Zone::Hand(_)) => Some(Fate::Hand),
            (_, Zone::Library(_)) => Some(Fate::Library),
            (_, Zone::Graveyard(_)) => Some(Fate::Graveyard),
            // Phasing out, a move to the command zone or back onto the battlefield is no fate the forecast names.
            _ => None,
        },
        _ => None,
    })
}
