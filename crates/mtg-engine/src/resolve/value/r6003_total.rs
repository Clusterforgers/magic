//! The budget of a search or choice whose filter holds `Pred::TotalAtMost` ("any number of creature cards with total
//! mana value 6 or less"): each card weighs its stat and the cards chosen together weigh at most the amount.

use mtg_cards::refs::{Pred, Selector};
use mtg_core::ids::ObjectId;

use super::{ResolveError, Scope};
use crate::choose::Budget;

impl Scope<'_> {
    /// The budget the filter sets for choosing among the cards, none when it sets no total.
    pub(crate) fn r6003_budget(
        &self,
        filter: &Selector,
        from: &[ObjectId],
    ) -> Result<Option<Budget>, ResolveError> {
        let Some((stat, most)) = filter.0.iter().find_map(|pred| match pred {
            Pred::TotalAtMost { stat, most } => Some((*stat, most)),
            _ => None,
        }) else {
            return Ok(None);
        };
        let most = u32::try_from(self.amount(most)?.max(0)).unwrap_or(u32::MAX);
        let weights = from
            .iter()
            .map(|id| {
                self.state
                    .object(*id)
                    .and_then(|object| self.seen(object.reference()))
                    .and_then(|seen| Self::stat(&seen, stat))
                    .map_or(0, |value| u32::try_from(value.max(0)).unwrap_or(u32::MAX))
            })
            .collect();
        Ok(Some(Budget { weights, most }))
    }
}
