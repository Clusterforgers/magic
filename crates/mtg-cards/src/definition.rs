//! The card definition the engine plays: printed characteristics and the abilities it runs.
//!
//! The server assembles one per oracle card from the imported record and the rules data
//! ([`crate::assemble`]); the test helpers in [`crate::vanilla`] build the plain ones directly.

use std::collections::BTreeSet;

use mtg_core::card::{CardType, CardTypes, Subtype, Supertype};
use mtg_core::color::{Color, ColorSet};
use mtg_core::mana::{ManaCost, ManaType};
use serde::{Deserialize, Serialize};

/// `{T}: Add one mana of a fixed type`; shaped so costs and choices can extend it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct ManaAbility {
    /// The mana produced.
    pub produces: ManaType,
}

/// An ability printed on a card; the card DSL will add variants.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub enum AbilityDef {
    /// A tap-for-mana ability.
    Mana(ManaAbility),
}

/// The static, printed side of a card.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct CardDefinition {
    /// Card name.
    pub name: String,
    /// Mana cost; `None` for cards without one (lands).
    pub mana_cost: Option<ManaCost>,
    /// Supertypes.
    pub supertypes: BTreeSet<Supertype>,
    /// Card types.
    pub types: BTreeSet<CardType>,
    /// Subtypes in printed order.
    pub subtypes: Vec<Subtype>,
    /// Printed power (creatures).
    pub power: Option<i32>,
    /// Printed toughness (creatures).
    pub toughness: Option<i32>,
    /// Printed abilities.
    pub abilities: Vec<AbilityDef>,
}

impl CardDefinition {
    /// A blank definition with just a name.
    pub fn new(name: impl Into<String>) -> Self {
        CardDefinition {
            name: name.into(),
            mana_cost: None,
            supertypes: BTreeSet::new(),
            types: BTreeSet::new(),
            subtypes: Vec::new(),
            power: None,
            toughness: None,
            abilities: Vec::new(),
        }
    }

    /// Sets the mana cost.
    #[must_use]
    pub fn with_mana_cost(mut self, cost: ManaCost) -> Self {
        self.mana_cost = Some(cost);
        self
    }

    /// Adds a card type.
    #[must_use]
    pub fn with_type(mut self, ty: CardType) -> Self {
        self.types.insert(ty);
        self
    }

    /// Adds a supertype.
    #[must_use]
    pub fn with_supertype(mut self, supertype: Supertype) -> Self {
        self.supertypes.insert(supertype);
        self
    }

    /// Adds a subtype.
    #[must_use]
    pub fn with_subtype(mut self, subtype: impl Into<String>) -> Self {
        self.subtypes.push(Subtype(subtype.into()));
        self
    }

    /// Sets printed power and toughness.
    #[must_use]
    pub fn with_power_toughness(mut self, power: i32, toughness: i32) -> Self {
        self.power = Some(power);
        self.toughness = Some(toughness);
        self
    }

    /// Adds a printed ability.
    #[must_use]
    pub fn with_ability(mut self, ability: AbilityDef) -> Self {
        self.abilities.push(ability);
        self
    }

    /// The basic land for `color`, or Wastes for `None`.
    pub fn basic_land(color: Option<Color>) -> Self {
        let (name, subtype, mana) = match color {
            Some(Color::White) => ("Plains", Some("Plains"), ManaType::White),
            Some(Color::Blue) => ("Island", Some("Island"), ManaType::Blue),
            Some(Color::Black) => ("Swamp", Some("Swamp"), ManaType::Black),
            Some(Color::Red) => ("Mountain", Some("Mountain"), ManaType::Red),
            Some(Color::Green) => ("Forest", Some("Forest"), ManaType::Green),
            None => ("Wastes", None, ManaType::Colorless),
        };
        let land = CardDefinition::new(name)
            .with_supertype(Supertype::Basic)
            .with_type(CardType::Land)
            .with_ability(AbilityDef::Mana(ManaAbility { produces: mana }));
        match subtype {
            Some(s) => land.with_subtype(s),
            None => land,
        }
    }

    /// Whether the card has `ty` among its printed types.
    pub fn is_type(&self, ty: CardType) -> bool {
        self.types.contains(&ty)
    }

    /// Printed types as a compact set.
    pub fn type_set(&self) -> CardTypes {
        self.types.iter().copied().collect()
    }

    /// Printed colors, derived from the mana cost (CR 105.2).
    pub fn colors(&self) -> ColorSet {
        self.mana_cost
            .as_ref()
            .map(ManaCost::colors)
            .unwrap_or_default()
    }

    /// Mana value of the card (0 without a cost).
    pub fn mana_value(&self) -> u32 {
        self.mana_cost.as_ref().map_or(0, ManaCost::mana_value)
    }
}

#[cfg(test)]
mod tests {
    //! Goal: card definition helpers derive colors, values and basic lands consistently.
    use super::*;
    use rstest::rstest;

    #[rstest]
    #[case(Some(Color::White), "Plains", ManaType::White)]
    #[case(Some(Color::Blue), "Island", ManaType::Blue)]
    #[case(Some(Color::Black), "Swamp", ManaType::Black)]
    #[case(Some(Color::Red), "Mountain", ManaType::Red)]
    #[case(Some(Color::Green), "Forest", ManaType::Green)]
    #[case(None, "Wastes", ManaType::Colorless)]
    fn basic_lands_tap_for_their_own_mana(
        #[case] color: Option<Color>,
        #[case] name: &str,
        #[case] mana: ManaType,
    ) {
        let land = CardDefinition::basic_land(color);
        assert_eq!(land.name, name);
        assert!(land.is_type(CardType::Land));
        assert!(land.supertypes.contains(&Supertype::Basic));
        assert_eq!(
            land.abilities,
            vec![AbilityDef::Mana(ManaAbility { produces: mana })]
        );
        assert!(land.colors().is_empty());
        assert_eq!(land.mana_value(), 0);
    }

    #[rstest]
    #[case("{1}{G}", 2, "G")]
    #[case("{W}{U}{B}", 3, "WUB")]
    #[case("{3}", 3, "")]
    #[case::hybrid("{1}{R/W}{R/W}", 3, "WR")]
    #[case::monocolored_hybrid("{2/G}", 2, "G")]
    #[case::phyrexian("{2}{U/P}", 3, "U")]
    #[case::colorless_hybrid("{C/B}", 1, "B")]
    #[case::colorless_phyrexian("{8}{C/P}{C/P}", 10, "")]
    fn colors_and_value_come_from_the_cost(
        #[case] cost: &str,
        #[case] value: u32,
        #[case] colors: &str,
    ) {
        let card = CardDefinition::new("X")
            .with_type(CardType::Creature)
            .with_mana_cost(ManaCost::parse(cost).unwrap());
        assert_eq!(card.mana_value(), value);
        let expected: ColorSet = colors.chars().filter_map(Color::from_symbol).collect();
        assert_eq!(card.colors(), expected);
    }

    #[test]
    fn type_set_matches_declared_types() {
        let card = CardDefinition::new("X")
            .with_type(CardType::Artifact)
            .with_type(CardType::Creature);
        let set = card.type_set();
        assert!(set.contains(CardType::Artifact) && set.contains(CardType::Creature));
        assert!(!set.contains(CardType::Land));
    }
}
