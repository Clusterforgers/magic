//! "When you do, ..." after a permanent entered as a copy (`ReplaceBy::AsCopy::r707_when_you_do`, CR 603.12, 707.9):
//! "You may have this creature enter as a copy of any creature on the battlefield, ... When you do, tap the copied
//! creature and it doesn't untap ... for as long as you control this creature." The reflexive trigger is set up and
//! triggers with the entering, its source the permanent the card becomes (CR 400.7), the copied object bound as `copied`.

use mtg_cards::effect::Effect;
use mtg_cards::trigger::TriggerPattern;
use mtg_core::ids::{ObjectId, PlayerId};

use super::{Candidate, Item, Origin, Pass, Slot};
use crate::abilities::{AbilitySource, Cause};
use crate::choose::Target;
use crate::events::Event;
use crate::object::ObjectRef;
use crate::triggers::{DelayedCreated, DelayedId, Triggered};
use crate::zones::Zone;

/// The name the copied object is bound to in the steps of the reflexive trigger.
const COPIED: &str = "copied";

impl Pass<'_> {
    /// Puts the reflexive trigger of the card `object` entering under `controller` as a copy of `copied` right after the
    /// slot `after`: nothing when it has no such steps.
    pub(super) fn r707_when_you_do(
        &mut self,
        after: usize,
        candidate: &Candidate,
        (object, controller): (ObjectId, PlayerId),
        copied: ObjectRef,
        effects: &[Effect],
    ) {
        if effects.is_empty() {
            return;
        }
        let Some(card) = self.state.object(object) else {
            return;
        };
        let source = ObjectRef {
            id: object,
            counter: card.counter() + 1,
        };
        let Some(mut snapshot) = crate::snapshot::snapshot(self.state, object) else {
            return;
        };
        snapshot.counter = source.counter;
        snapshot.zone = Zone::Battlefield;
        let (ability, origin) = match candidate.origin {
            Origin::Ability(address) => (address.ability, address.source),
            _ => (0, AbilitySource::default()),
        };
        let id = DelayedId(self.state.next_delayed);
        let created = DelayedCreated {
            id,
            source,
            snapshot: snapshot.clone(),
            controller,
            ability,
            // It triggers below, not on an event: this pattern never matches.
            when: TriggerPattern::AnyOf(Vec::new()),
            once: true,
            this_turn: true,
            until: None,
            effects: effects.to_vec(),
            targets: Vec::new(),
            bindings: vec![(COPIED.to_owned(), vec![Target::Object(copied)])],
            kept: None,
            origin,
        };
        let triggered = Triggered {
            source,
            ability,
            origin,
            controller,
            cause: Cause::default(),
            snapshot,
            delayed: Some(id),
        };
        for (offset, event) in [
            Event::DelayedTriggerCreated(Box::new(created)),
            Event::AbilityTriggered(Box::new(triggered)),
        ]
        .into_iter()
        .enumerate()
        {
            self.slots.insert(
                after + 1 + offset,
                Slot {
                    item: Item::One(event),
                    applied: Vec::new(),
                    frozen: false,
                },
            );
        }
    }
}
