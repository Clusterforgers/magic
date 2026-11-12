//! The source of an ability on the stack: "activated ability from an artifact source" and "that permanent" of a
//! countered ability (CR 113.7a, 608.2h).

use mtg_cards::refs::{Ref, Selector};

use super::{ResolveError, Scope};
use crate::choose::Target;
use crate::object::{GameObject, ObjectRef};
use crate::snapshot::Snapshot;

impl<'a> Scope<'a> {
    /// The source of the ability `reference` names, with its snapshot, or nothing when it names no ability on the stack.
    fn r302b_source_of(&self, reference: ObjectRef) -> Option<(ObjectRef, &'a Snapshot)> {
        self.state
            .object(reference.id)
            .filter(|object| object.counter() == reference.counter)
            .and_then(GameObject::ability)
            .map(|ability| (ability.source(), ability.snapshot()))
    }

    /// Whether the object is an ability on the stack whose source matches the selector: the source as it is while it is
    /// the same object, else as it last existed.
    pub(super) fn r302b_from_source(
        &self,
        source: &Selector,
        reference: ObjectRef,
    ) -> Result<bool, ResolveError> {
        let Some((of, snapshot)) = self.r302b_source_of(reference) else {
            return Ok(false);
        };
        let mut scope = self.clone();
        scope.snapshots.push(snapshot);
        scope.selects(source, of)
    }

    /// The sources of the abilities the reference names, each as the object it was when its ability was put on the stack.
    pub(super) fn r302b_sources(&self, of: &Ref) -> Result<Vec<Target>, ResolveError> {
        Ok(self
            .objects(of)?
            .into_iter()
            .filter_map(|ability| self.r302b_source_of(ability))
            .map(|(source, _)| Target::Object(source))
            .collect())
    }
}
