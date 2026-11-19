//! Shields used up by the first batch they change (CR 614.1a): "the next time one or more creatures enter this turn, they
//! enter as copies of the chosen creature" changes every entering of that batch it covers and then ends
//! (`ReplaceBy::NextBatch`); the targets the shield fixed as it began are read by its `Ref::Target`s.

use std::sync::Arc;

use mtg_cards::trigger::ReplaceBy;
use crate::object::ObjectRef;

use super::{Candidate, Origin, Pass};
use crate::resolve::Bindings;
use crate::rules::Rule;

/// The replacement a shield applies: the inner one of a shield used up by its next batch.
pub(super) fn r109_inner(by: &ReplaceBy) -> &ReplaceBy {
    match by {
        ReplaceBy::NextBatch(inner) => inner,
        other => other,
    }
}

impl Pass<'_> {
    /// The targets the shield behind the candidate fixed as it began, by the names `Ref::Target` reads; none for any
    /// other replacement.
    pub(super) fn r109_bound(&self, candidate: &Candidate) -> Bindings {
        let Origin::Shield(id) = candidate.origin else {
            return Bindings::new();
        };
        self.shields
            .iter()
            .find(|effect| effect.id == id)
            .and_then(|effect| match &effect.rule {
                Rule::Replaces { bound, .. } => Some(bound.iter().cloned().collect()),
                _ => None,
            })
            .unwrap_or_default()
    }

    /// The copiable values the shield behind the candidate kept of each named target that left the battlefield.
    pub(super) fn r109_lki(
        &self,
        candidate: &Candidate,
    ) -> Vec<(ObjectRef, Arc<mtg_cards::CardDefinition>)> {
        let Origin::Shield(id) = candidate.origin else {
            return Vec::new();
        };
        self.shields
            .iter()
            .find(|effect| effect.id == id)
            .and_then(|effect| match &effect.rule {
                Rule::Replaces { lki, .. } => Some(lki.clone()),
                _ => None,
            })
            .unwrap_or_default()
    }

    /// A shield used up by its next batch ends with the batch the candidate changes: the batch's other events it covers
    /// are still changed by it.
    pub(super) fn r109_end_with_batch(&mut self, candidate: &Candidate) {
        let Origin::Shield(id) = candidate.origin else {
            return;
        };
        let next = self.shields.iter().any(|effect| {
            effect.id == id
                && matches!(
                    &effect.rule,
                    Rule::Replaces {
                        by: ReplaceBy::NextBatch(_),
                        ..
                    }
                )
        });
        if next && !self.touched.contains(&id) {
            self.touched.push(id);
        }
    }
}
