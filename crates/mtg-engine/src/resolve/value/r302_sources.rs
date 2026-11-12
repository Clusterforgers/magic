//! The predicates over the source of an ability on the stack: an ability from a source of a kind ("counter target
//! activated ability from an artifact source") and the source of a named ability ("that permanent").

use std::borrow::Cow;

use mtg_cards::refs::{Ref, Selector};

use super::{Counted, ResolveError, Scope, Seen};
use crate::object::{GameObject, ObjectKind, ObjectRef};
use crate::zones::Zone;

impl Scope<'_> {
    /// Whether the object is an ability on the stack whose source satisfies the selector: the source itself while it is
    /// the same object, else the source as the ability's snapshot holds it (CR 113.7a, 608.2h).
    pub(super) fn r302_ability_from(
        &self,
        sources: &Selector,
        seen: &Seen<'_>,
    ) -> Result<bool, ResolveError> {
        let Some(ability) = seen
            .card
            .filter(|_| seen.zone == Zone::Stack)
            .and_then(GameObject::ability)
        else {
            return Ok(false);
        };
        let source = ability.source();
        if self
            .state
            .object(source.id)
            .is_some_and(|object| object.counter() == source.counter)
        {
            return self.selects(sources, source);
        }
        let shot = ability.snapshot();
        let as_it_was = Seen {
            owner: shot.owner,
            controller: shot.controller,
            zone: shot.zone,
            kind: ObjectKind::Card,
            chars: Cow::Borrowed(&shot.characteristics),
            tapped: None,
            card: None,
            keywords: Some(shot.keywords.as_slice()),
            counters: Counted::Shot(&shot.counters),
            x: 0,
            suspected: shot.suspected,
            base_pt: None,
        };
        for pred in &sources.0 {
            if !self.pred(pred, shot.reference(), &as_it_was)? {
                return Ok(false);
            }
        }
        Ok(true)
    }

    /// Whether the object is the source of one of the abilities on the stack the reference names, as the same object.
    pub(super) fn r302_source_of(
        &self,
        named: &Ref,
        reference: ObjectRef,
    ) -> Result<bool, ResolveError> {
        Ok(self
            .objects(named)?
            .into_iter()
            .filter_map(|named| {
                self.state
                    .object(named.id)
                    .filter(|object| object.counter() == named.counter)
            })
            .filter_map(GameObject::ability)
            .any(|ability| ability.source() == reference))
    }
}
