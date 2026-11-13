//! The turn's log of what was done: damage, attacks, blocks, activations and counters put, as records of references that the
//! fold appends in event order and `Pred::DidThisTurn` and `Amount::Tally` read. It lives one turn and is derived from events.
//!
//! A record names an object only while the object is public (a permanent, a spell, a card in a graveyard or in exile face up).
//! A hidden object leaves a record with no object: it counts, and no filter on a characteristic selects it.

use mtg_cards::refs::{Ref, Selector};
use mtg_cards::value::TallyEvent;
use mtg_core::ids::{ObjectId, PlayerId};

use crate::choose::Target;
use crate::object::ObjectRef;
use crate::resolve::Scope;
use crate::snapshot::Snapshot;
use crate::state::GameState;
use crate::zones::Zone;

/// One thing that was done this turn.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) struct Record {
    /// The kind of event.
    pub(crate) event: TallyEvent,
    /// The player it counts for; none when no spell or ability caused it.
    pub(crate) player: Option<PlayerId>,
    /// The object that did it, when it is public.
    pub(crate) subject: Option<ObjectRef>,
    /// The subject as it was when the record was made, which a filter judges (CR 608.2h).
    pub(crate) shot: Option<Snapshot>,
    /// What it was done to, when that is a player or a public object.
    pub(crate) other: Option<Target>,
    /// The object it was done to as it was when the record was made (the attacker a creature blocked).
    pub(crate) other_shot: Option<Snapshot>,
    /// How much: the damage dealt, the counters put; 1 for the rest.
    pub(crate) amount: u32,
}

impl GameState {
    /// The object as a reference when anyone may see it: on the battlefield, on the stack, in a graveyard or in exile face up.
    fn public_object(&self, id: ObjectId) -> Option<ObjectRef> {
        let object = self.source_object(id)?;
        let public = match object.zone() {
            Zone::Battlefield | Zone::Stack | Zone::Graveyard(_) | Zone::Command(_) => true,
            Zone::Exile => object.face_down().is_none(),
            _ => false,
        };
        public.then(|| object.reference())
    }

    /// Adds a record to the turn's log. Nothing is logged for an amount of nothing.
    pub(crate) fn record(
        &mut self,
        event: TallyEvent,
        player: Option<PlayerId>,
        (subject, other): (ObjectId, Option<Target>),
        amount: u32,
    ) {
        if amount == 0 {
            return;
        }
        let other = other.and_then(|target| match target {
            Target::Object(object) => self.public_object(object.id).map(Target::Object),
            player @ Target::Player(_) => Some(player),
        });
        let other_shot = match other {
            Some(Target::Object(object)) => crate::snapshot::snapshot(self, object.id),
            _ => None,
        };
        let shot = self
            .public_object(subject)
            .and_then(|_| crate::snapshot::snapshot(self, subject));
        let subject = self.public_object(subject);
        self.tallies.records.push(Record {
            event,
            player,
            subject,
            shot,
            other,
            other_shot,
            amount,
        });
    }

    /// The player who controls the spell or ability that is resolving.
    pub(crate) fn resolving_controller(&self) -> Option<PlayerId> {
        let running = self.resolving()?;
        self.object(running.object)
            .map(crate::object::GameObject::controller)
    }

    /// Whether the object is the spell that was the `nth` spell cast this turn by any player, as it was cast.
    pub(crate) fn r283_nth_cast_is(&self, nth: u32, object: ObjectRef) -> bool {
        let Some(at) = usize::try_from(nth).ok().and_then(|nth| nth.checked_sub(1)) else {
            return false;
        };
        self.tallies
            .logged
            .iter()
            .filter(|(event, _, _)| *event == TallyEvent::SpellCast)
            .nth(at)
            .is_some_and(|(_, _, shot)| shot.id == object.id && shot.counter == object.counter)
    }

    /// The combat phase begins: "this combat" reads the records from here on.
    pub(crate) fn begin_combat_log(&mut self) {
        self.tallies.combat_from = self.tallies.records.len();
    }
}

/// The kinds that count what they carry (damage, counters) instead of events.
const fn weighs(event: TallyEvent) -> bool {
    matches!(
        event,
        TallyEvent::Damaged
            | TallyEvent::DealtExcess
            | TallyEvent::CountersPut
            | TallyEvent::PlusOneCountersPut
    )
}

/// The kinds that count the creatures that did it rather than the times.
const fn once_per_object(event: TallyEvent) -> bool {
    matches!(event, TallyEvent::Attacked | TallyEvent::Blocked)
}

/// Whether the kind is one of the turn's records rather than of the snapshot log.
pub(crate) const fn is_record(event: TallyEvent) -> bool {
    matches!(
        event,
        TallyEvent::Damaged
            | TallyEvent::DealtExcess
            | TallyEvent::Attacked
            | TallyEvent::Blocked
            | TallyEvent::Activated
            | TallyEvent::KindActivated(_)
            | TallyEvent::CountersPut
            | TallyEvent::PlusOneCountersPut
    )
}

impl Scope<'_> {
    /// Whether the object did the event this turn (this combat), for the players `by` names and to what `to` names.
    pub(crate) fn did_this_turn(
        &self,
        event: TallyEvent,
        (by, to): (Option<&Ref>, Option<&Ref>),
        combat: bool,
        object: ObjectRef,
    ) -> bool {
        let players = by.map(|who| self.players(who).unwrap_or_default());
        let to = to.map(|what| self.refs(what).unwrap_or_default());
        let log = &self.state.tallies;
        let from = if combat { log.combat_from } else { 0 };
        log.records[from.min(log.records.len())..]
            .iter()
            .any(|record| {
                record.event == event
                    && record.subject == Some(object)
                    && players.as_ref().is_none_or(|players| {
                        record
                            .player
                            .is_some_and(|player| players.contains(&player))
                    })
                    && to
                        .as_ref()
                        .is_none_or(|to| record.other.is_some_and(|other| to.contains(&other)))
            })
    }

    /// The different sources that dealt damage this turn while one of the players controlled them, each object once; a
    /// record of a source no one could see counts on its own.
    pub(crate) fn r5502_sources_dealt_damage(&self, players: &[PlayerId]) -> i64 {
        let mut seen: Vec<ObjectRef> = Vec::new();
        let mut hidden = 0i64;
        for record in &self.state.tallies.records {
            if record.event != TallyEvent::Damaged
                || !record
                    .player
                    .is_some_and(|player| players.contains(&player))
            {
                continue;
            }
            match record.subject {
                Some(subject) if !seen.contains(&subject) => seen.push(subject),
                Some(_) => {}
                None => hidden += 1,
            }
        }
        i64::try_from(seen.len()).unwrap_or(i64::MAX) + hidden
    }

    /// The damage dealt this turn to the players and objects named, as the objects were when it was dealt.
    pub(crate) fn g10c_damage_to(&self, to: &[Target]) -> i64 {
        self.state
            .tallies
            .records
            .iter()
            .filter(|record| {
                record.event == TallyEvent::Damaged
                    && record.other.is_some_and(|other| to.contains(&other))
            })
            .map(|record| i64::from(record.amount))
            .sum()
    }

    /// The records of the kind this turn that count for the players and whose object the filter selects: events counted,
    /// damage and counters by their amount, an attack or a block once per creature ("each creature that attacked this turn"
    /// counts a creature that attacked in two combats once).
    pub(crate) fn recorded(
        &self,
        event: TallyEvent,
        players: &[PlayerId],
        filter: &Selector,
    ) -> i64 {
        let mut scope = self.clone();
        scope.as_was = true;
        scope.snapshots.extend(
            self.state
                .tallies
                .records
                .iter()
                .filter_map(|record| record.shot.as_ref()),
        );
        let mut total = 0i64;
        let mut seen: Vec<ObjectRef> = Vec::new();
        for record in &self.state.tallies.records {
            if record.event != event
                || !record
                    .player
                    .is_some_and(|player| players.contains(&player))
            {
                continue;
            }
            let selected = filter.is_empty()
                || record
                    .shot
                    .as_ref()
                    .is_some_and(|shot| scope.selects(filter, shot.reference()).unwrap_or(false));
            let again = once_per_object(event)
                && record.subject.is_some_and(|subject| {
                    let known = seen.contains(&subject);
                    seen.push(subject);
                    known
                });
            if selected && !again {
                total += if weighs(event) {
                    i64::from(record.amount)
                } else {
                    1
                };
            }
        }
        total
    }

    /// Whether the object blocked or was blocked this turn by a creature `with` selects as it was when the block was
    /// declared; by any creature when `with` is empty.
    pub(crate) fn r379_blocked_this_turn(&self, with: &Selector, object: ObjectRef) -> bool {
        let state = self.state;
        // Each block is judged by the snapshot of its own record, put first.
        let selects = |shot: Option<&Snapshot>| {
            with.is_empty()
                || shot.is_some_and(|shot| {
                    let mut one = self.clone();
                    one.as_was = true;
                    one.snapshots.insert(0, shot);
                    one.selects(with, shot.reference()).unwrap_or(false)
                })
        };
        state
            .tallies
            .records
            .iter()
            .filter(|record| record.event == TallyEvent::Blocked)
            .any(|record| {
                let attacker = match record.other {
                    Some(Target::Object(attacker)) => Some(attacker),
                    _ => None,
                };
                (record.subject == Some(object)
                    && attacker.is_some()
                    && selects(record.other_shot.as_ref()))
                    || (attacker == Some(object)
                        && record.subject.is_some()
                        && selects(record.shot.as_ref()))
            })
    }
}
