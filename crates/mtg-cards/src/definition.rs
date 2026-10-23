//! The card definition the engine plays: printed characteristics and the abilities it runs.
//!
//! The server assembles one per oracle card from the imported record and the rules data
//! ([`crate::assemble()`]); the test helpers in [`crate::vanilla`] build the plain ones directly.

use std::collections::BTreeSet;
use std::sync::Arc;

use mtg_core::card::{CardType, CardTypes, Subtype, Supertype};
use mtg_core::color::{Color, ColorSet};
use mtg_core::mana::{ManaCost, ManaType};
use serde::{Deserialize, Serialize};

use crate::ability::Ability;
use crate::modify::Modification;
use crate::value::Amount;

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

/// Whether a count is zero, so serialization can leave it out (serde hands over a reference).
#[allow(clippy::trivially_copy_pass_by_ref)]
const fn is_zero(count: &u8) -> bool {
    *count == 0
}

/// A clause of a card's rules text that the engine does not run: the table follows it by hand.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct ManualClause {
    /// The clause as the card prints it.
    pub text: String,
    /// Why the engine leaves it to the table, in plain words.
    pub reason: String,
}

/// How the faces of a card with more than one are used (CR 708 to 715).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub enum Layout {
    /// One face.
    #[default]
    Normal,
    /// A split card (CR 709): both halves everywhere but on the stack, where only the half cast counts.
    Split,
    /// A card with an Adventure (CR 715): the permanent card everywhere but on the stack cast as the Adventure.
    Adventure,
    /// A modal double-faced card (CR 712): the front face everywhere but on the stack and the battlefield, where the face chosen counts.
    ModalDfc,
    /// A transforming double-faced card (CR 712.3): the front face everywhere but on the battlefield, where the face it is turned to counts.
    Transform,
}

impl Layout {
    /// The layout a Scryfall `layout` keyword names, if the engine plays it.
    pub fn from_scryfall(keyword: &str) -> Option<Layout> {
        match keyword {
            "split" => Some(Layout::Split),
            "adventure" => Some(Layout::Adventure),
            "modal_dfc" => Some(Layout::ModalDfc),
            "transform" => Some(Layout::Transform),
            _ => None,
        }
    }

    /// Whether this is the layout of a card with a single face, so serialization can leave it out.
    #[allow(clippy::trivially_copy_pass_by_ref)]
    pub const fn is_normal(&self) -> bool {
        matches!(self, Layout::Normal)
    }
}

/// The static, printed side of a card.
///
/// A field added after the first fingerprints were stored is left out of the serialized form while it
/// holds its default (`docs/server.md`, deck check), so older decks keep their fingerprints.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct CardDefinition {
    /// Card name.
    pub name: String,
    /// Mana cost without its `{X}` symbols; `None` for cards without a cost (lands).
    pub mana_cost: Option<ManaCost>,
    /// How many `{X}` symbols the printed cost holds. The engine cannot pay them yet, so such a card
    /// is not offered as castable.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub x_in_cost: u8,
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
    /// Starting loyalty (planeswalkers).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub loyalty: Option<u32>,
    /// Starting defense (battles).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub defense: Option<u32>,
    /// The colors a color indicator gives (CR 204), when the mana cost does not.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color_indicator: Option<ColorSet>,
    /// Printed abilities.
    pub abilities: Vec<AbilityDef>,
    /// The rules the effect pipeline runs (spells, activated abilities and the like), shared by every copy of the card.
    ///
    /// An ability is named by its index in this list.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub rules: Vec<Arc<Ability>>,
    /// How the faces of the card are used; the fields above are the card outside the stack and the battlefield.
    #[serde(default, skip_serializing_if = "Layout::is_normal")]
    pub layout: Layout,
    /// The clauses of this face's rules text the engine does not run. Every seat reads them as the card's own text; the
    /// card is cast and resolves without them and its controller does them by hand.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub manual: Vec<ManualClause>,
    /// Every face of a card with more than one, in Scryfall's order, each with its own rules; empty for a card with one face.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub faces: Vec<CardDefinition>,
}

impl CardDefinition {
    /// A blank definition with just a name.
    pub fn new(name: impl Into<String>) -> Self {
        CardDefinition {
            name: name.into(),
            mana_cost: None,
            x_in_cost: 0,
            supertypes: BTreeSet::new(),
            types: BTreeSet::new(),
            subtypes: Vec::new(),
            power: None,
            toughness: None,
            loyalty: None,
            defense: None,
            color_indicator: None,
            abilities: Vec::new(),
            rules: Vec::new(),
            layout: Layout::Normal,
            manual: Vec::new(),
            faces: Vec::new(),
        }
    }

    /// Sets the faces of a card with more than one and how they are used.
    #[must_use]
    pub fn with_faces(mut self, layout: Layout, faces: Vec<CardDefinition>) -> Self {
        self.layout = layout;
        self.faces = faces;
        self
    }

    /// The face `index` of a card with several faces, or the card itself when it has none.
    pub fn face(&self, index: u8) -> &CardDefinition {
        self.faces.get(usize::from(index)).unwrap_or(self)
    }

    /// Whether the card has more than one face.
    pub fn is_multi_face(&self) -> bool {
        !self.faces.is_empty()
    }

    /// The card as a whole and each of its faces: every definition a rule of the card may be printed on.
    pub fn each_face(&self) -> impl Iterator<Item = &CardDefinition> {
        std::iter::once(self).chain(self.faces.iter())
    }

    /// Sets the mana cost.
    #[must_use]
    pub fn with_mana_cost(mut self, cost: ManaCost) -> Self {
        self.mana_cost = Some(cost);
        self
    }

    /// Sets the number of `{X}` symbols in the printed cost.
    #[must_use]
    pub fn with_x_in_cost(mut self, x: u8) -> Self {
        self.x_in_cost = x;
        self
    }

    /// Sets the starting loyalty.
    #[must_use]
    pub fn with_loyalty(mut self, loyalty: u32) -> Self {
        self.loyalty = Some(loyalty);
        self
    }

    /// Sets the starting defense.
    #[must_use]
    pub fn with_defense(mut self, defense: u32) -> Self {
        self.defense = Some(defense);
        self
    }

    /// Sets the colors of a color indicator.
    #[must_use]
    pub fn with_color_indicator(mut self, colors: ColorSet) -> Self {
        self.color_indicator = Some(colors);
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

    /// Adds a rules ability for the effect pipeline.
    #[must_use]
    pub fn with_rules(mut self, ability: Ability) -> Self {
        self.rules.push(Arc::new(ability));
        self
    }

    /// Adds a clause of the rules text the engine does not run.
    #[must_use]
    pub fn with_manual(mut self, text: impl Into<String>, reason: impl Into<String>) -> Self {
        self.manual.push(ManualClause {
            text: text.into(),
            reason: reason.into(),
        });
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

    /// Printed colors: those of the color indicator if it has one (CR 204), otherwise those of the
    /// mana cost (CR 105.2).
    pub fn colors(&self) -> ColorSet {
        self.color_indicator.unwrap_or_else(|| {
            self.mana_cost
                .as_ref()
                .map(ManaCost::colors)
                .unwrap_or_default()
        })
    }

    /// Mana value of the card (0 without a cost).
    pub fn mana_value(&self) -> u32 {
        self.mana_cost.as_ref().map_or(0, ManaCost::mana_value)
    }

    /// This definition with the abilities of every definition of `cards` in the order given and none of its own, for a
    /// merged permanent (CR 729.2): its other characteristics stay.
    #[must_use]
    pub fn with_abilities_of<'a>(
        mut self,
        cards: impl IntoIterator<Item = &'a CardDefinition>,
    ) -> CardDefinition {
        self.rules.clear();
        self.abilities.clear();
        self.manual.clear();
        for card in cards {
            self.rules.extend(card.rules.iter().cloned());
            self.abilities.extend(card.abilities.iter().copied());
            self.manual.extend(card.manual.iter().cloned());
        }
        self
    }

    /// The copiable values of this face (CR 707.2): what it prints, as a card of one face. A copy of a face of a card
    /// with several faces has the values of that face alone (CR 707.8).
    #[must_use]
    pub fn copiable(&self) -> CardDefinition {
        let mut values = self.clone();
        values.layout = Layout::Normal;
        values.faces.clear();
        values
    }

    /// Applies one exception of a copy effect to these copiable values (CR 707.9b): an extra type or subtype, a
    /// keyword or an ability, colours, a fixed power and toughness. False, with nothing changed, for any other
    /// modification.
    pub fn except(&mut self, modification: &Modification) -> bool {
        match modification {
            Modification::AddType(ty) => {
                self.types.insert(*ty);
            }
            Modification::AddSubtype(subtype) => {
                if !self.subtypes.contains(subtype) {
                    self.subtypes.push(subtype.clone());
                }
            }
            Modification::AddKeyword(keyword) => {
                self.rules.push(Arc::new(Ability::Keyword(keyword.clone())));
            }
            Modification::GrantAbility(ability) => self.rules.push(Arc::clone(ability)),
            Modification::SetColors(colors) => {
                self.color_indicator = Some(
                    colors
                        .iter()
                        .fold(ColorSet::EMPTY, |set, color| set.with(*color)),
                );
            }
            Modification::SetPT {
                power: Amount::Fixed(power),
                toughness: Amount::Fixed(toughness),
            } => {
                self.power = Some(*power);
                self.toughness = Some(*toughness);
            }
            _ => return false,
        }
        true
    }
}

#[cfg(test)]
mod tests {
    //! Goal: card definition helpers derive colors, values and basic lands consistently.
    use super::*;
    use rstest::rstest;

    use crate::effect::Effect;
    use crate::refs::Ref;

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

    #[rstest]
    #[case::indicator_beats_the_cost("{2}{G}", Some("U"), "U")]
    #[case::indicator_without_a_cost("", Some("WB"), "WB")]
    #[case::indicator_of_nothing("{1}{R}", Some(""), "")]
    #[case::cost_when_there_is_no_indicator("{1}{R}", None, "R")]
    fn a_color_indicator_replaces_the_colors_of_the_cost(
        #[case] cost: &str,
        #[case] indicator: Option<&str>,
        #[case] expected: &str,
    ) {
        let letters =
            |text: &str| -> ColorSet { text.chars().filter_map(Color::from_symbol).collect() };
        let mut card = CardDefinition::new("X").with_type(CardType::Creature);
        if !cost.is_empty() {
            card = card.with_mana_cost(ManaCost::parse(cost).unwrap());
        }
        if let Some(indicator) = indicator {
            card = card.with_color_indicator(letters(indicator));
        }
        assert_eq!(card.colors(), letters(expected));
    }

    #[test]
    fn fields_added_later_stay_out_of_the_stored_form_until_they_are_used() {
        let plain = CardDefinition::basic_land(Some(Color::Green));
        let json = serde_json::to_value(&plain).unwrap();
        let keys: BTreeSet<&str> = json
            .as_object()
            .unwrap()
            .keys()
            .map(String::as_str)
            .collect();
        let expected = [
            "abilities",
            "mana_cost",
            "name",
            "power",
            "subtypes",
            "supertypes",
            "toughness",
            "types",
        ];
        assert_eq!(keys, expected.into_iter().collect());
        assert_eq!(
            serde_json::from_value::<CardDefinition>(json).unwrap(),
            plain
        );

        let used = CardDefinition::new("Y")
            .with_x_in_cost(2)
            .with_loyalty(3)
            .with_defense(4)
            .with_color_indicator(ColorSet::EMPTY.with(Color::Blue))
            .with_rules(Ability::Spell {
                targets: vec![],
                effects: vec![Effect::Draw {
                    who: Ref::You,
                    count: Amount::one(),
                }],
            });
        let json = serde_json::to_value(&used).unwrap();
        for key in [
            "x_in_cost",
            "loyalty",
            "defense",
            "color_indicator",
            "rules",
        ] {
            assert!(json.get(key).is_some(), "{key} is stored once it is used");
        }
        assert_eq!(
            serde_json::from_value::<CardDefinition>(json).unwrap(),
            used
        );
    }
}
