//! A card's rules file: abilities per face, keyed by oracle id, and the tokens cards can make.

use mtg_core::card::{CardType, Subtype, Supertype};
use mtg_core::color::Color;
use serde::{Deserialize, Serialize};

use crate::ability::Ability;

/// How much of a card the rules data covers, as its author or the compiler claims.
///
/// This is the declared value (the database column `cards.impl_status`); the computed support
/// state is [`crate::support::Support`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ImplStatus {
    /// Every clause of every face is modelled.
    Full,
    /// Modelled except the clauses listed as `Manual` abilities.
    Partial,
    /// Nothing is modelled; the table resolves the whole text.
    Manual,
    /// The engine cannot host the card at all.
    Unplayable,
}

/// A stretch of a face's oracle text: Unicode scalar values from `start` up to, not including, `end`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(deny_unknown_fields)]
pub struct Span {
    /// First character.
    pub start: u32,
    /// One past the last character.
    pub end: u32,
}

impl Span {
    /// The text the span covers, or `None` if it does not lie inside `text`.
    pub fn slice<'a>(&self, text: &'a str) -> Option<&'a str> {
        if self.start >= self.end {
            return None;
        }
        let offset = |chars: u32| {
            text.char_indices()
                .map(|(at, _)| at)
                .chain([text.len()])
                .nth(usize::try_from(chars).ok()?)
        };
        text.get(offset(self.start)?..offset(self.end)?)
    }

    /// Whether the two spans share a character.
    pub const fn overlaps(&self, other: &Span) -> bool {
        self.start < other.end && other.start < self.end
    }
}

/// One face of a card: its abilities and the oracle text they were written for.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Face {
    /// The face name, as imported (a check that the file belongs to this face).
    pub name: String,
    /// Fingerprint of the face's oracle text when the abilities were written; empty for a card with nothing implemented.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub oracle_fingerprint: String,
    /// The abilities, in printed order.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub abilities: Vec<Ability>,
}

/// The rules of one oracle card. Characteristics come from the imported record, not from here.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Card {
    /// The Scryfall oracle id; the key.
    pub oracle_id: String,
    /// The full card name (`A // B` for several faces); its lowercase ASCII slug is the file name.
    pub name: String,
    /// What the data covers.
    pub impl_status: ImplStatus,
    /// One entry per printed face, in Scryfall's order.
    pub faces: Vec<Face>,
    /// Modelling notes for reviewers.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub notes: Option<String>,
}

impl Card {
    /// Every ability with the index of its face.
    pub fn abilities(&self) -> impl Iterator<Item = (usize, &Ability)> {
        self.faces
            .iter()
            .enumerate()
            .flat_map(|(face, f)| f.abilities.iter().map(move |ability| (face, ability)))
    }
}

/// What a token is; the effect that creates it defines it, so it carries its own characteristics.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TokenSpec {
    /// The token's name.
    pub name: String,
    /// Its colors.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub colors: Vec<Color>,
    /// Its supertypes.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub supertypes: Vec<Supertype>,
    /// Its card types.
    pub types: Vec<CardType>,
    /// Its subtypes.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub subtypes: Vec<Subtype>,
    /// Its power, for a creature.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub power: Option<i32>,
    /// Its toughness, for a creature.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub toughness: Option<i32>,
    /// Its abilities.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub abilities: Vec<Ability>,
}

/// A predefined token in `cards/tokens/`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TokenFile {
    /// The id other files use (`treasure`); also the file name.
    pub id: String,
    /// The token.
    pub token: TokenSpec,
}

#[cfg(test)]
mod tests {
    //! Goal: spans slice text by characters, and a card file leaves out everything that is empty or default.
    use super::*;
    use rstest::rstest;

    #[rstest]
    #[case::start(0, 2, Some("ab"))]
    #[case::unicode_minus(3, 6, Some("\u{2212}1:"))]
    #[case::to_the_end(6, 9, Some(" ok"))]
    #[case::last_word(7, 9, Some("ok"))]
    #[case::past_the_end(6, 10, None)]
    #[case::empty(2, 2, None)]
    #[case::backwards(4, 2, None)]
    fn spans_count_characters_not_bytes(
        #[case] start: u32,
        #[case] end: u32,
        #[case] expected: Option<&str>,
    ) {
        let text = "ab \u{2212}1: ok";
        assert_eq!(Span { start, end }.slice(text), expected);
    }

    #[test]
    fn overlap_needs_a_shared_character() {
        let a = Span { start: 0, end: 5 };
        assert!(a.overlaps(&Span { start: 4, end: 8 }));
        assert!(!a.overlaps(&Span { start: 5, end: 8 }));
    }

    #[test]
    fn a_vanilla_card_is_four_fields() {
        let card = Card {
            oracle_id: "14c8f55d-d177-4c25-a931-ebeb9e6062a0".into(),
            name: "Grizzly Bears".into(),
            impl_status: ImplStatus::Full,
            faces: vec![Face {
                name: "Grizzly Bears".into(),
                oracle_fingerprint: String::new(),
                abilities: vec![],
            }],
            notes: None,
        };
        let json = serde_json::to_string(&card).unwrap();
        assert_eq!(
            json,
            r#"{"oracle_id":"14c8f55d-d177-4c25-a931-ebeb9e6062a0","name":"Grizzly Bears","impl_status":"Full","faces":[{"name":"Grizzly Bears"}]}"#
        );
        assert_eq!(serde_json::from_str::<Card>(&json).unwrap(), card);
    }
}
