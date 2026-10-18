//! Card types, supertypes and subtypes: the vocabulary of a type line.

use serde::{Deserialize, Serialize};

/// A card type (CR 205.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub enum CardType {
    /// Artifact.
    Artifact,
    /// Battle.
    Battle,
    /// Creature.
    Creature,
    /// Enchantment.
    Enchantment,
    /// Instant.
    Instant,
    /// Kindred.
    Kindred,
    /// Land.
    Land,
    /// Planeswalker.
    Planeswalker,
    /// Sorcery.
    Sorcery,
}

impl CardType {
    const fn bit(self) -> u16 {
        1 << self as u16
    }

    /// Whether objects of this type are permanents once resolved.
    pub const fn is_permanent_type(self) -> bool {
        !matches!(self, CardType::Instant | CardType::Sorcery)
    }
}

/// A cheap copyable set of card types.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(transparent)]
pub struct CardTypes(u16);

impl CardTypes {
    /// Whether `ty` is in the set.
    pub const fn contains(self, ty: CardType) -> bool {
        self.0 & ty.bit() != 0
    }

    /// Returns the set with `ty` added.
    #[must_use]
    pub const fn with(self, ty: CardType) -> Self {
        CardTypes(self.0 | ty.bit())
    }

    /// Whether the set has no types.
    pub const fn is_empty(self) -> bool {
        self.0 == 0
    }
}

impl FromIterator<CardType> for CardTypes {
    fn from_iter<I: IntoIterator<Item = CardType>>(iter: I) -> Self {
        iter.into_iter().fold(CardTypes::default(), CardTypes::with)
    }
}

/// A supertype (CR 205.4).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub enum Supertype {
    /// Basic.
    Basic,
    /// Legendary.
    Legendary,
    /// Snow.
    Snow,
    /// World.
    World,
}

/// A subtype such as `Elf` or `Forest` (CR 205.3).
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(transparent)]
pub struct Subtype(pub String);

#[cfg(test)]
mod tests {
    //! Goal: card types know which of them are permanent types and collect into compact sets.
    use super::*;

    #[test]
    fn only_instants_and_sorceries_are_not_permanent_types() {
        for ty in [CardType::Instant, CardType::Sorcery] {
            assert!(!ty.is_permanent_type());
        }
        assert!(CardType::Creature.is_permanent_type());
        assert!(CardType::Land.is_permanent_type());
    }

    #[test]
    fn a_type_set_holds_what_it_was_built_from() {
        let set: CardTypes = [CardType::Artifact, CardType::Creature]
            .into_iter()
            .collect();
        assert!(set.contains(CardType::Artifact) && set.contains(CardType::Creature));
        assert!(!set.contains(CardType::Land));
        assert!(!set.is_empty());
        assert!(CardTypes::default().is_empty());
    }
}
