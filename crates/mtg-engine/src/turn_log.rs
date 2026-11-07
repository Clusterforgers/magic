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
    /// What it was done to, when that is a player or a public object.
    pub(crate) other: Option<Target>,
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
        let subject = self.public_object(subject);
        self.tallies.records.push(Record {
            event,
            player,
            subject,
            other,
            amount,
        });
    }

    /// The player who controls the spell or ability that is resolving.
    pub(crate) fn resolving_controller(&self) -> Option<PlayerId> {
        let running = self.resolving()?;
        self.object(running.object)
            .map(crate::object::GameObject::controller)
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
        TallyEvent::Damaged | TallyEvent::CountersPut | TallyEvent::PlusOneCountersPut
    )
}

/// Whether the kind is one of the turn's records rather than of the snapshot log.
pub(crate) const fn is_record(event: TallyEvent) -> bool {
    matches!(
        event,
        TallyEvent::Damaged
            | TallyEvent::Attacked
            | TallyEvent::Blocked
            | TallyEvent::Activated
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
    /// damage and counters by their amount.
    pub(crate) fn recorded(
        &self,
        event: TallyEvent,
        players: &[PlayerId],
        filter: &Selector,
    ) -> i64 {
        let mut total = 0i64;
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
                    .subject
                    .is_some_and(|subject| self.selects(filter, subject).unwrap_or(false));
            if selected {
                total += if weighs(event) {
                    i64::from(record.amount)
                } else {
                    1
                };
            }
        }
        total
    }
}
