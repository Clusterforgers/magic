//! The predicates of a soulbond pair (CR 702.95): paired with another creature, and paired with one of the objects a
//! reference names.

use mtg_cards::refs::Pred;

use super::{ResolveError, Scope};
use crate::object::{GameObject, ObjectRef};

impl Scope<'_> {
    /// Whether a predicate of this module holds for the object; false for any other predicate.
    pub(super) fn soulbond_pred(
        &self,
        pred: &Pred,
        reference: ObjectRef,
    ) -> Result<bool, ResolveError> {
        let Some(partner) = self.partner(reference) else {
            return Ok(false);
        };
        Ok(match pred {
            Pred::Paired => true,
            Pred::PairedWith(named) => self.objects(named)?.contains(&partner),
            _ => false,
        })
    }

    /// The creature the object is paired with, when `reference` is still the very permanent that was paired.
    fn partner(&self, reference: ObjectRef) -> Option<ObjectRef> {
        let partner = self.state.partner_of(reference.id)?;
        let mine = self.state.object(reference.id)?;
        let theirs = self.state.object(partner)?;
        (mine.reference() == reference).then(|| GameObject::reference(theirs))
    }
}
