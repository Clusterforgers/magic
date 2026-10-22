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
