//! The cards above the source in its owner's graveyard (`Amount::CardsAbove`): a graveyard keeps the order its cards were
//! put there in, the last one on top (Krovikan Horror, Death Spark, Nether Shadow).

use mtg_cards::refs::Selector;

use super::{ResolveError, Scope};
use crate::zones::Zone;

impl Scope<'_> {
    /// The cards the filter selects above the source in its graveyard, or only the one directly above it; 0 when the source
    /// is in no graveyard.
    pub(super) fn r287_cards_above(
        &self,
        filter: &Selector,
        directly: bool,
    ) -> Result<i64, ResolveError> {
        let Some(source) = self
            .state
            .object(self.source.id)
            .filter(|object| object.counter() == self.source.counter)
        else {
            return Ok(0);
        };
        let Zone::Graveyard(owner) = source.zone() else {
            return Ok(0);
        };
        let cards = self.state.zone(Zone::Graveyard(owner));
        let Some(at) = cards.iter().position(|id| *id == self.source.id) else {
            return Ok(0);
        };
        let above = cards.get(at + 1..).unwrap_or_default();
        let above = if directly {
            above.get(..1).unwrap_or_default()
        } else {
            above
        };
        let mut count = 0;
        for id in above {
            let Some(card) = self.state.object(*id) else {
                continue;
            };
            if self.selects(filter, card.reference())? {
                count += 1;
            }
        }
        Ok(count)
    }
}
