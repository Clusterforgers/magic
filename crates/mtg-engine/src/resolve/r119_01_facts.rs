//! Facts of the r119-01 pack: whether the objects a reference names share a card type ("if all cards exiled with this
//! Saga share a card type").

use mtg_cards::refs::{Pred, Ref, Selector};
use mtg_core::card::CardType;

use super::{ResolveError, Scope};

/// Every card type (CR 205.2a).
const R119_CARD_TYPES: [CardType; 9] = [
    CardType::Artifact,
    CardType::Battle,
    CardType::Creature,
    CardType::Enchantment,
    CardType::Instant,
    CardType::Kindred,
    CardType::Land,
    CardType::Planeswalker,
    CardType::Sorcery,
];

impl Scope<'_> {
    /// Whether the objects `what` names, one or more, all have some one card type.
    pub(super) fn r119_share_card_type(&self, what: &Ref) -> Result<bool, ResolveError> {
        let named = i64::try_from(self.objects(what)?.len()).unwrap_or(i64::MAX);
        if named == 0 {
            return Ok(false);
        }
        for kind in R119_CARD_TYPES {
            if self.count_of(what, &Selector(vec![Pred::Type(kind)]))? == named {
                return Ok(true);
            }
        }
        Ok(false)
    }
}
