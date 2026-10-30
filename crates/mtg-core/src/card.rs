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

/// A cheap copyable set of card types; on the wire a number whose bit n is the nth [`CardType`] in declaration order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
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

/// The subtypes that are not creature types: land, artifact, enchantment, spell and battle types (CR 205.3g to 205.3q).
const OTHER_TYPES: &[&str] = &[
    "Plains",
    "Island",
    "Swamp",
    "Mountain",
    "Forest",
    "Desert",
    "Gate",
    "Lair",
    "Locus",
    "Mine",
    "Power-Plant",
    "Tower",
    "Urza's",
    "Cave",
    "Sphere",
    "Town",
    "Planet",
    "Equipment",
    "Fortification",
    "Vehicle",
    "Clue",
    "Food",
    "Treasure",
    "Blood",
    "Gold",
    "Powerstone",
    "Incubator",
    "Junk",
    "Map",
    "Shard",
    "Contraption",
    "Attraction",
    "Aura",
    "Background",
    "Cartouche",
    "Case",
    "Class",
    "Curse",
    "Role",
    "Room",
    "Saga",
    "Shrine",
    "Arcane",
    "Trap",
    "Lesson",
    "Omen",
    "Siege",
    "Adventure",
    "Dungeon",
    "Plane",
    "Scheme",
    "Vanguard",
    "Phenomenon",
];

/// The land types a player may name (CR 205.3i): "Urza's" is one type, and "Urza's Mine" is it with "Mine".
pub const LAND_TYPES: [&str; 17] = [
    "Cave",
    "Desert",
    "Forest",
    "Gate",
    "Island",
    "Lair",
    "Locus",
    "Mine",
    "Mountain",
    "Plains",
    "Planet",
    "Power-Plant",
    "Sphere",
    "Swamp",
    "Tower",
    "Town",
    "Urza's",
];

impl Subtype {
    /// Whether the subtype is a creature type, which an effect that sets creature types replaces (CR 205.1b).
    pub fn is_creature_type(&self) -> bool {
        !OTHER_TYPES.contains(&self.0.as_str())
    }

    /// The two land types a name of two words stands for: "Urza's Mine" is the types "Urza's" and "Mine" (CR 205.3i),
    /// so a permanent is it when it has both.
    pub fn two_word_land(&self) -> Option<(Subtype, Subtype)> {
        let (first, second) = self.0.split_once(' ')?;
        (first == "Urza's" && matches!(second, "Mine" | "Power-Plant" | "Tower"))
            .then(|| (Subtype(first.to_owned()), Subtype(second.to_owned())))
    }
}

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
    fn only_the_creature_types_are_replaced_by_an_effect_that_sets_them() {
        for creature in ["Elf", "Weird", "Angel", "Equipment-Golem"] {
            assert!(Subtype(creature.into()).is_creature_type(), "{creature}");
        }
        for other in ["Forest", "Equipment", "Aura", "Treasure", "Saga", "Siege"] {
            assert!(!Subtype(other.into()).is_creature_type(), "{other}");
        }
    }

    #[test]
    fn no_land_type_is_a_creature_type() {
        for land in LAND_TYPES {
            assert!(!Subtype(land.into()).is_creature_type(), "{land}");
        }
    }

    #[test]
    fn a_land_type_of_two_words_is_two_types() {
        let tower = Subtype("Urza's Tower".into()).two_word_land();
        assert_eq!(
            tower,
            Some((Subtype("Urza's".into()), Subtype("Tower".into())))
        );
        for other in ["Tower", "Urza's", "Time Lord", "Urza's Saga", "Mine"] {
            assert_eq!(Subtype(other.into()).two_word_land(), None, "{other}");
        }
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
