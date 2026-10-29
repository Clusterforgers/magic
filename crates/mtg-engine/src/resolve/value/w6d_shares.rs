//! The predicates that compare an object with the objects a reference names (a name, a card type or a color in common,
//! read from the last known information of one that left) and those of its own state: {X} in its cost, goaded, and the
//! permanents attached to it.

use crate::zones::Zone;
use mtg_cards::refs::{Pred, Ref, Selector};
use mtg_core::card::CardType;

use super::{ResolveError, Scope, Seen};
use crate::characteristics::{goaded_by, goaders};
use crate::object::GameObject;

/// Every card type, in the order of the comprehensive rules.
const W6D_TYPES: [CardType; 9] = [
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

/// The name of the object as it was seen: none while it lies face down (CR 708.2a).
fn w6d_name<'s>(seen: &Seen<'s>) -> Option<&'s str> {
    seen.card
        .filter(|card| card.face_down().is_none())
        .map(|card| card.card().name.as_str())
}

impl Scope<'_> {
    /// Whether a predicate of this module holds for the object; false for any other predicate.
    pub(super) fn w6d_pred(&self, pred: &Pred, seen: &Seen<'_>) -> Result<bool, ResolveError> {
        Ok(match pred {
            Pred::SharesName(named) => self.w6d_shares_name(named, seen)?,
            Pred::SharesType(named) => self.w6d_shares_type(named, seen)?,
            Pred::SharesColor(named) => self.w6d_shares_color(named, seen)?,
            Pred::XInCost => Self::w6d_x_in_cost(seen),
            Pred::Goaded => self.w6d_goaded(seen),
            Pred::HasAttached(attached) => self.w6d_has_attached(attached, seen)?,
            Pred::AttachedTo(host) => self.w6d_attached_to(host, seen)?,
            _ => false,
        })
    }

    /// Whether one of the objects the reference names, as it is or as it last existed, stands in `relation` to `seen`.
    fn w6d_any_named(
        &self,
        named: &Ref,
        seen: &Seen<'_>,
        relation: impl Fn(&Seen<'_>, &Seen<'_>) -> bool,
    ) -> Result<bool, ResolveError> {
        Ok(self
            .objects(named)?
            .into_iter()
            .filter_map(|other| self.seen(other))
            .any(|theirs| relation(seen, &theirs)))
    }

    /// Whether the object has the name of one of the objects the reference names.
    fn w6d_shares_name(&self, named: &Ref, seen: &Seen<'_>) -> Result<bool, ResolveError> {
        self.w6d_any_named(named, seen, |mine, theirs| {
            w6d_name(mine).is_some_and(|name| w6d_name(theirs) == Some(name))
        })
    }

    /// Whether the object shares a card type with one of the objects the reference names.
    fn w6d_shares_type(&self, named: &Ref, seen: &Seen<'_>) -> Result<bool, ResolveError> {
        self.w6d_any_named(named, seen, |mine, theirs| {
            W6D_TYPES.iter().any(|card_type| {
                mine.chars.types.contains(*card_type) && theirs.chars.types.contains(*card_type)
            })
        })
    }

    /// Whether the object shares a color with one of the objects the reference names.
    fn w6d_shares_color(&self, named: &Ref, seen: &Seen<'_>) -> Result<bool, ResolveError> {
        self.w6d_any_named(named, seen, |mine, theirs| {
            mine.chars
                .colors
                .iter()
                .any(|color| theirs.chars.colors.contains(color))
        })
    }

    /// Whether the object has {X} in its mana cost: a face-down one has no mana cost (CR 708.2a).
    fn w6d_x_in_cost(seen: &Seen<'_>) -> bool {
        seen.card
            .filter(|card| card.face_down().is_none())
            .is_some_and(|card| card.card().x_in_cost > 0)
    }

    /// Whether the object is a creature on the battlefield that some player goads.
    fn w6d_goaded(&self, seen: &Seen<'_>) -> bool {
        seen.card.is_some_and(|card| {
            seen.zone == Zone::Battlefield
                && seen.chars.types.contains(CardType::Creature)
                && match self.layered {
                    // While the layers are worked out, the goad effects that have applied so far.
                    Some(view) => view
                        .get(card.id())
                        .is_some_and(|held| goaders(held).next().is_some()),
                    None => !goaded_by(self.state, card.id()).is_empty(),
                }
        })
    }

    /// Whether the object is a permanent on the battlefield attached to one of the objects the reference names.
    fn w6d_attached_to(&self, host: &Ref, seen: &Seen<'_>) -> Result<bool, ResolveError> {
        let Some(card) = seen.card.filter(|_| seen.zone == Zone::Battlefield) else {
            return Ok(false);
        };
        let Some(attached_to) = self.state.host_of(card.id()) else {
            return Ok(false);
        };
        Ok(self
            .objects(host)?
            .into_iter()
            .any(|named| named.id == attached_to))
    }

    /// Whether a permanent that matches the selector is attached to the object, which is still on the battlefield.
    fn w6d_has_attached(&self, attached: &Selector, seen: &Seen<'_>) -> Result<bool, ResolveError> {
        let Some(host) = seen.card.filter(|_| seen.zone == Zone::Battlefield) else {
            return Ok(false);
        };
        for id in self.state.attachments_of(host.id()) {
            let Some(reference) = self.state.object(id).map(GameObject::reference) else {
                continue;
            };
            if self.selects(attached, reference)? {
                return Ok(true);
            }
        }
        Ok(false)
    }
}
