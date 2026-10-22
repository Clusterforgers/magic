//! The colour identity of a card definition (CR 903.4) and whether it can be a commander (CR 903.3).
//!
//! A definition does not store Scryfall's `color_identity`: it derives it from what it holds, so a hand-made card,
//! a compiled card and a stored one answer the same way. The sources are the colors of the card and of the symbols
//! in its mana cost, the symbols in its rules text (the cost and the mana of every ability the engine runs, and the
//! text of the clauses it leaves to the table, reminder text excluded), and the mana ability a basic land type gives.
//! The identity of a card with several faces is that of all its faces.

use mtg_core::card::{CardType, Supertype};
use mtg_core::color::{Color, ColorSet};
use mtg_core::mana::ManaType;
use serde_json::Value;

use crate::ability::Ability;
use crate::definition::{AbilityDef, CardDefinition};
use crate::keyword::Keyword;
use crate::modify::GameRule;

/// Basic land types and the color of the mana each one gives.
const BASIC_TYPES: [(&str, Color); 5] = [
    ("Plains", Color::White),
    ("Island", Color::Blue),
    ("Swamp", Color::Black),
    ("Mountain", Color::Red),
    ("Forest", Color::Green),
];

impl CardDefinition {
    /// The card's color identity (CR 903.4): its colors, the colors of every mana symbol in its cost and rules
    /// text, and those of its basic land types, over all its faces.
    pub fn color_identity(&self) -> ColorSet {
        self.each_face().fold(ColorSet::EMPTY, |all, face| {
            face.face_identity()
                .iter()
                .fold(all, |all, color| all.with(color))
        })
    }

    fn face_identity(&self) -> ColorSet {
        let mut set = self.color_indicator.unwrap_or(ColorSet::EMPTY);
        if let Some(cost) = &self.mana_cost {
            set = cost.colors().iter().fold(set, |set, color| set.with(color));
        }
        for subtype in &self.subtypes {
            if let Some((_, color)) = BASIC_TYPES.iter().find(|(name, _)| *name == subtype.0) {
                set = set.with(*color);
            }
        }
        for ability in &self.abilities {
            let AbilityDef::Mana(mana) = ability;
            if let Some(color) = color_of(mana.produces) {
                set = set.with(color);
            }
        }
        for rule in &self.rules {
            if let Ok(value) = serde_json::to_value(&**rule) {
                scan(&value, &mut set);
            }
        }
        for clause in &self.manual {
            symbols(&without_reminders(&clause.text), &mut set);
        }
        set
    }

    /// Whether the card can be a player's commander (CR 903.3): a legendary creature, a legendary Vehicle or
    /// Spacecraft with a power, or a card that says it can be.
    pub fn can_be_commander(&self) -> bool {
        self.each_face().next().is_some_and(|front| {
            let legendary = front.supertypes.contains(&Supertype::Legendary);
            let body = front.types.contains(&CardType::Creature)
                || (front.power.is_some()
                    && front
                        .subtypes
                        .iter()
                        .any(|s| matches!(s.0.as_str(), "Vehicle" | "Spacecraft")));
            (legendary && body) || self.rules.iter().any(|rule| says_it_can(rule))
        })
    }

    /// The copies of the card a singleton deck may hold: `None` for any number (a basic land, or a card that says
    /// "A deck can have any number of cards named ~"), else one, or the number a card that says "up to N" names.
    pub fn singleton_limit(&self) -> Option<u32> {
        if self.supertypes.contains(&Supertype::Basic) {
            return None;
        }
        let text = self
            .each_face()
            .flat_map(|face| face.manual.iter().map(|clause| clause.text.to_lowercase()))
            .collect::<Vec<_>>()
            .join("\n");
        if text.contains("a deck can have any number of cards named") {
            return None;
        }
        let marker = "a deck can have up to ";
        let up_to = text.find(marker).and_then(|at| {
            let word = text[at + marker.len()..].split_whitespace().next()?;
            const WORDS: [&str; 10] = [
                "one", "two", "three", "four", "five", "six", "seven", "eight", "nine", "ten",
            ];
            let at = WORDS.iter().position(|w| word.starts_with(w))?;
            u32::try_from(at + 1).ok()
        });
        Some(up_to.unwrap_or(1))
    }

    /// Whether two cards can be commanders together by Partner: both have it, or each names the other with
    /// "Partner with" (CR 702.124).
    pub fn partners_with(&self, other: &CardDefinition) -> bool {
        let has_partner = |card: &CardDefinition| {
            card.rules
                .iter()
                .any(|rule| matches!(&**rule, Ability::Keyword(Keyword::Partner)))
        };
        let names = |card: &CardDefinition, other: &CardDefinition| {
            card.rules.iter().any(|rule| {
                matches!(&**rule, Ability::Keyword(Keyword::PartnerWith(name)) if *name == other.name)
            })
        };
        (has_partner(self) && has_partner(other)) || (names(self, other) && names(other, self))
    }
}

fn says_it_can(rule: &Ability) -> bool {
    matches!(
        rule,
        Ability::Rule {
            rule: GameRule::CanBeCommander,
            ..
        }
    )
}

fn color_of(mana: ManaType) -> Option<Color> {
    match mana {
        ManaType::White => Some(Color::White),
        ManaType::Blue => Some(Color::Blue),
        ManaType::Black => Some(Color::Black),
        ManaType::Red => Some(Color::Red),
        ManaType::Green => Some(Color::Green),
        ManaType::Colorless => None,
    }
}

/// Adds the colors of the mana symbols in the strings of an ability's serialized form, and of the mana an effect
/// names by type (`Fixed`, `OneOf`).
fn scan(value: &Value, set: &mut ColorSet) {
    match value {
        Value::String(text) => symbols(text, set),
        Value::Array(items) => items.iter().for_each(|item| scan(item, set)),
        Value::Object(fields) => {
            for (key, inner) in fields {
                if matches!(key.as_str(), "Fixed" | "OneOf")
                    && let Value::Array(types) = inner
                {
                    for name in types.iter().filter_map(Value::as_str) {
                        if let Some(color) = Color::ALL.iter().find(|c| format!("{c:?}") == name) {
                            *set = set.with(*color);
                        }
                    }
                }
                scan(inner, set);
            }
        }
        _ => {}
    }
}

/// Adds the colors of every `{...}` symbol in `text`: `{W}`, `{G/U}`, `{2/W}`, `{B/P}`.
fn symbols(text: &str, set: &mut ColorSet) {
    let mut rest = text;
    while let Some(open) = rest.find('{') {
        let Some(len) = rest[open..].find('}') else {
            return;
        };
        for part in rest[open + 1..open + len].split('/') {
            let mut letters = part.chars();
            if let (Some(letter), None) = (letters.next(), letters.next())
                && let Some(color) = Color::from_symbol(letter)
            {
                *set = set.with(color);
            }
        }
        rest = &rest[open + len + 1..];
    }
}

/// `text` without the parenthesized reminder text (CR 207.2), which gives no color identity.
fn without_reminders(text: &str) -> String {
    let mut depth = 0u32;
    text.chars()
        .filter(|c| match c {
            '(' => {
                depth += 1;
                false
            }
            ')' => {
                depth = depth.saturating_sub(1);
                false
            }
            _ => depth == 0,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    //! Goal: the color identity of a definition counts every color its cost, indicator, rules text and basic land types name (hybrid, Phyrexian and activated-ability symbols included, reminder text and generic mana not), over all its faces; a card can be a commander when it is a legendary creature (or a Vehicle with a power) or says so; partners pair by Partner or by name; and the copies a deck may hold follow the card's own text.
    use rstest::rstest;

    use super::*;
    use crate::definition::{Layout, ManaAbility};
    use mtg_core::mana::ManaCost;

    fn rules(json: &str) -> Ability {
        serde_json::from_str(json).unwrap()
    }

    fn colors(letters: &str) -> ColorSet {
        letters
            .chars()
            .filter_map(Color::from_symbol)
            .fold(ColorSet::EMPTY, ColorSet::with)
    }

    fn costing(cost: &str) -> CardDefinition {
        CardDefinition::new("Card").with_mana_cost(ManaCost::parse(cost).unwrap())
    }

    fn mana(produces: ManaType) -> AbilityDef {
        AbilityDef::Mana(ManaAbility { produces })
    }

    #[rstest]
    #[case::two_colors(costing("{1}{R}{G}"), "RG")]
    #[case::generic_only(costing("{5}"), "")]
    #[case::colorless_symbol(costing("{C}{C}"), "")]
    #[case::hybrid(costing("{W/U}"), "WU")]
    #[case::mono_hybrid(costing("{2/B}"), "B")]
    #[case::phyrexian(costing("{G/P}"), "G")]
    #[case::hybrid_phyrexian(costing("{R/G/P}"), "RG")]
    #[case::colorless_hybrid(costing("{C/W}"), "W")]
    #[case::indicator(CardDefinition::new("Dryad").with_color_indicator(colors("G")), "G")]
    #[case::indicator_and_cost(costing("{U}").with_color_indicator(colors("R")), "UR")]
    #[case::land_type(CardDefinition::basic_land(Some(Color::Green)), "G")]
    #[case::wastes(CardDefinition::basic_land(None), "")]
    #[case::mana_ability(CardDefinition::new("Land").with_ability(mana(ManaType::Red)), "R")]
    #[case::colorless_mana_ability(CardDefinition::new("Land").with_ability(mana(ManaType::Colorless)), "")]
    #[case::activated_cost(costing("{1}").with_rules(rules(r#"{"Activated":{"cost":"{2}{U}","effects":[{"Draw":{}}]}}"#)), "U")]
    #[case::activated_hybrid_cost(costing("{1}").with_rules(rules(r#"{"Activated":{"cost":"{B/G}","effects":[{"Draw":{}}]}}"#)), "BG")]
    #[case::added_mana(costing("{1}").with_rules(rules(r#"{"Activated":{"cost":["Tap"],"effects":[{"AddMana":{"mana":{"Fixed":["Green","Colorless"]}}}]}}"#)), "G")]
    #[case::any_color_adds_none(costing("{1}").with_rules(rules(r#"{"Activated":{"cost":["Tap"],"effects":[{"AddMana":{"mana":{"AnyColor":1}}}]}}"#)), "")]
    #[case::rules_text_clause(costing("{1}").with_manual("{W}, {T}: Draw a card.", "not run"), "W")]
    #[case::reminder_text_is_no_rules_text(costing("{1}").with_manual("Flash (You may cast this for {W}.)", "not run"), "")]
    fn a_definition_names_its_color_identity(#[case] card: CardDefinition, #[case] expected: &str) {
        assert_eq!(card.color_identity(), colors(expected));
    }

    #[test]
    fn a_card_with_two_faces_has_the_identity_of_both() {
        let front = costing("{1}{G}");
        let back = CardDefinition::new("Back").with_color_indicator(colors("B"));
        let card = CardDefinition::new("Both").with_faces(Layout::Transform, vec![front, back]);
        assert_eq!(card.color_identity(), colors("GB"));
    }

    fn legendary(types: &[CardType]) -> CardDefinition {
        types
            .iter()
            .fold(CardDefinition::new("Legend"), |card, ty| {
                card.with_type(*ty)
            })
            .with_supertype(Supertype::Legendary)
    }

    #[rstest]
    #[case::legendary_creature(legendary(&[CardType::Creature]), true)]
    #[case::legendary_artifact_creature(legendary(&[CardType::Artifact, CardType::Creature]), true)]
    #[case::plain_creature(CardDefinition::new("Bear").with_type(CardType::Creature), false)]
    #[case::legendary_artifact(legendary(&[CardType::Artifact]), false)]
    #[case::legendary_planeswalker(legendary(&[CardType::Planeswalker]), false)]
    #[case::legendary_vehicle_with_power(legendary(&[CardType::Artifact]).with_subtype("Vehicle").with_power_toughness(3, 3), true)]
    #[case::legendary_vehicle_without_power(legendary(&[CardType::Artifact]).with_subtype("Vehicle"), false)]
    #[case::says_it_can(CardDefinition::new("Walker").with_type(CardType::Planeswalker).with_rules(rules(r#"{"Rule":{"rule":"CanBeCommander"}}"#)), true)]
    fn a_card_can_be_a_commander(#[case] card: CardDefinition, #[case] can: bool) {
        assert_eq!(card.can_be_commander(), can);
    }

    fn with(name: &str, ability: &str) -> CardDefinition {
        CardDefinition::new(name).with_rules(rules(ability))
    }

    #[rstest]
    #[case::both_have_partner(
        with("A", r#"{"Keyword":"Partner"}"#),
        with("B", r#"{"Keyword":"Partner"}"#),
        true
    )]
    #[case::only_one(with("A", r#"{"Keyword":"Partner"}"#), CardDefinition::new("B"), false)]
    #[case::neither(CardDefinition::new("A"), CardDefinition::new("B"), false)]
    #[case::each_names_the_other(
        with("A", r#"{"Keyword":{"PartnerWith":"B"}}"#),
        with("B", r#"{"Keyword":{"PartnerWith":"A"}}"#),
        true
    )]
    #[case::one_names_the_other(
        with("A", r#"{"Keyword":{"PartnerWith":"B"}}"#),
        with("B", r#"{"Keyword":"Partner"}"#),
        false
    )]
    #[case::names_someone_else(
        with("A", r#"{"Keyword":{"PartnerWith":"C"}}"#),
        with("B", r#"{"Keyword":{"PartnerWith":"A"}}"#),
        false
    )]
    fn partners_pair(#[case] a: CardDefinition, #[case] b: CardDefinition, #[case] pair: bool) {
        assert_eq!(a.partners_with(&b), pair);
    }

    #[rstest]
    #[case::ordinary(CardDefinition::new("Bear"), Some(1))]
    #[case::basic(CardDefinition::basic_land(Some(Color::Red)), None)]
    #[case::any_number(CardDefinition::new("Rats").with_manual("A deck can have any number of cards named Rats.", ""), None)]
    #[case::up_to_seven(CardDefinition::new("Dwarves").with_manual("A deck can have up to seven cards named Dwarves.", ""), Some(7))]
    fn a_singleton_deck_holds_the_copies_a_card_allows(
        #[case] card: CardDefinition,
        #[case] limit: Option<u32>,
    ) {
        assert_eq!(card.singleton_limit(), limit);
    }
}
