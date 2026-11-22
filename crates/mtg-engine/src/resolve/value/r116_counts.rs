//! "The number of creature types among non-Borg creatures you control" (Federation): each creature type once among the
//! objects, an object of several adding each of them and one with every creature type (changeling) adding every one.

use std::collections::BTreeSet;

use mtg_cards::refs::Selector;
use mtg_core::card::{CardType, Subtype};
use mtg_core::subtypes::KNOWN_SUBTYPES;

use super::{ResolveError, Scope};

impl Scope<'_> {
    /// How many creature types there are among the objects the selector matches.
    pub(super) fn r116_creature_types_among(&self, of: &Selector) -> Result<i64, ResolveError> {
        let mut found: BTreeSet<String> = BTreeSet::new();
        for object in self.matching(of)? {
            let Some(seen) = self.seen(object) else {
                continue;
            };
            if seen.chars.all_creature_types && seen.chars.types.contains(CardType::Creature) {
                let every = KNOWN_SUBTYPES
                    .iter()
                    .filter(|name| Subtype((**name).to_owned()).is_creature_type())
                    .count();
                return Ok(i64::try_from(every).unwrap_or(i64::MAX));
            }
            let printed: &[Subtype] = seen.card.map_or(&[], |card| &card.card().subtypes);
            found.extend(
                seen.chars
                    .subtypes
                    .iter()
                    .chain(printed)
                    .filter(|subtype| {
                        subtype.is_creature_type() && seen.chars.has_subtype(printed, subtype)
                    })
                    .map(|subtype| subtype.0.clone()),
            );
        }
        Ok(i64::try_from(found.len()).unwrap_or(i64::MAX))
    }
}
