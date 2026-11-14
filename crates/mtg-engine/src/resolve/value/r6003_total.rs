//! The budget of a search or choice whose filter holds `Pred::TotalAtMost` ("any number of creature cards with total
//! mana value 6 or less"): the cards chosen together add up to at most the amount (`CardsTotal`).

use mtg_cards::refs::{Pred, Selector};
use mtg_core::ids::ObjectId;

use super::{ResolveError, Scope};
use crate::choose::CardsTotal;

impl Scope<'_> {
    /// The total the filter sets for choosing among the cards, none when it sets no total.
    pub(crate) fn r6003_total(
        &self,
        filter: &Selector,
        from: &[ObjectId],
    ) -> Result<Option<CardsTotal>, ResolveError> {
        let Some((stat, most)) = filter.0.iter().find_map(|pred| match pred {
            Pred::TotalAtMost { stat, most } => Some((*stat, most)),
            _ => None,
        }) else {
            return Ok(None);
        };
        let most = i32::try_from(self.amount(most)?).unwrap_or(i32::MAX);
        let values = from
            .iter()
            .map(|id| {
                self.state
                    .object(*id)
                    .and_then(|object| self.seen(object.reference()))
                    .and_then(|seen| Self::stat(&seen, stat))
                    .map_or(0, |value| i32::try_from(value).unwrap_or(i32::MAX))
            })
            .collect();
        let stat = if stat == mtg_cards::value::StatKind::Power {
            "power"
        } else {
            "mana value"
        };
        Ok(Some(CardsTotal {
            most,
            values,
            stat: stat.to_owned(),
        }))
    }
}
