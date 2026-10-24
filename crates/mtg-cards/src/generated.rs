//! Generated rules: what the compiler writes for a stored card, checked like a rules file, and the
//! digests that tell when a stored row is out of date (`docs/cards-dsl.md`, Compiler and Manual fallback).

use std::collections::BTreeSet;

use sha2::{Digest, Sha256};

use crate::card::{Card, ImplStatus};
use crate::compile::compile_among;
use crate::load::{Library, LoadError};
use crate::support::{PrintedCard, hex};

/// The version of the compiler and of the rules data types it writes: a digest of their sources
/// taken at build time, so a row written by another build is read again.
pub const COMPILER_VERSION: &str = env!("MTG_COMPILER_VERSION");

/// What the compiler made of a stored card, checked.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Generation {
    /// The compiler reads nothing of the card: no rules text, nothing it can read, or data that cannot hold it.
    Nothing,
    /// Rules, `Full` or `Partial`, that pass the checks of a rules file.
    Rules(Box<Card>),
    /// Rules that fail a check; they are never stored.
    Invalid(Vec<LoadError>),
}

/// The digest of what the compiler reads of a card: its name and keywords and, for each face, the
/// name, cost, type line, oracle text, characteristics and colors. Another digest means the
/// stored rules were written for other data.
pub fn input_fingerprint(printed: &PrintedCard) -> String {
    let mut hasher = Sha256::new();
    let mut part = |text: &str| {
        hasher.update(text.len().to_le_bytes());
        hasher.update(text.as_bytes());
    };
    part(&printed.name);
    for keyword in &printed.keywords {
        part(keyword);
    }
    for face in &printed.faces {
        part("face");
        for text in [
            &face.name,
            &face.mana_cost,
            &face.type_line,
            &face.oracle_text,
        ] {
            part(text);
        }
        for value in [&face.power, &face.toughness, &face.loyalty, &face.defense] {
            part(value.as_deref().unwrap_or("-"));
        }
        for color in &face.colors {
            part(color);
        }
    }
    hex(&hasher.finalize()[..16])
}

/// Compiles a card and checks the result as a rules file is checked: structure, then the imported
/// card. `hand_written` supplies the predefined tokens and `names` the cards a rule may name.
pub fn generate(
    printed: &PrintedCard,
    hand_written: &Library,
    names: &BTreeSet<String>,
) -> Generation {
    let Some(card) = compile_among(printed, Some(names))
        .card
        .filter(|card| matches!(card.impl_status, ImplStatus::Full | ImplStatus::Partial))
    else {
        return Generation::Nothing;
    };
    let problems = hand_written.check_generated(&card, printed, names);
    if problems.is_empty() {
        Generation::Rules(Box::new(card))
    } else {
        Generation::Invalid(problems)
    }
}

#[cfg(test)]
mod tests {
    //! Goal: the fingerprint of a card follows every datum the compiler reads and nothing else, a card the compiler reads is generated with rules that pass the checks of a file, a card it reads nothing of generates nothing, and the version is a digest of the build.
    use rstest::rstest;

    use super::*;
    use crate::support::PrintedFace;

    fn bolt() -> PrintedCard {
        PrintedCard {
            layout: String::new(),
            oracle_id: "4457ed35-7c10-48c8-9776-456485fdf070".into(),
            name: "Lightning Bolt".into(),
            keywords: Vec::new(),
            faces: vec![PrintedFace {
                name: "Lightning Bolt".into(),
                mana_cost: "{R}".into(),
                type_line: "Instant".into(),
                oracle_text: "Lightning Bolt deals 3 damage to any target.".into(),
                colors: vec!["R".into()],
                ..PrintedFace::default()
            }],
        }
    }

    #[test]
    fn the_fingerprint_is_32_hex_digits_and_stable() {
        let fingerprint = input_fingerprint(&bolt());
        assert_eq!(fingerprint.len(), 32);
        assert!(fingerprint.chars().all(|c| c.is_ascii_hexdigit()));
        assert_eq!(fingerprint, input_fingerprint(&bolt()));
    }

    #[rstest]
    #[case::the_text(|card: &mut PrintedCard| card.faces[0].oracle_text.push('!'))]
    #[case::the_name(|card: &mut PrintedCard| card.name.push('!'))]
    #[case::the_cost(|card: &mut PrintedCard| card.faces[0].mana_cost = "{X}{R}".into())]
    #[case::the_type_line(|card: &mut PrintedCard| card.faces[0].type_line = "Sorcery".into())]
    #[case::the_keywords(|card: &mut PrintedCard| card.keywords.push("Flash".into()))]
    #[case::a_characteristic(|card: &mut PrintedCard| card.faces[0].power = Some("3".into()))]
    #[case::the_colors(|card: &mut PrintedCard| card.faces[0].colors.clear())]
    #[case::another_face(|card: &mut PrintedCard| card.faces.push(PrintedFace::default()))]
    fn the_fingerprint_follows_what_the_compiler_reads(#[case] change: fn(&mut PrintedCard)) {
        let mut changed = bolt();
        change(&mut changed);
        assert_ne!(input_fingerprint(&changed), input_fingerprint(&bolt()));
    }

    #[test]
    fn the_oracle_id_is_no_part_of_what_is_read() {
        let mut other = bolt();
        other.oracle_id = "00000000-0000-0000-0000-000000000001".into();
        assert_eq!(input_fingerprint(&other), input_fingerprint(&bolt()));
    }

    #[test]
    fn a_card_the_compiler_reads_is_generated_and_checked() {
        let names = BTreeSet::from(["lightning bolt".to_owned()]);
        let Generation::Rules(card) = generate(&bolt(), &Library::default(), &names) else {
            panic!("rules were expected");
        };
        assert_eq!(card.impl_status, ImplStatus::Full);
        assert_eq!(card.oracle_id, bolt().oracle_id);
    }

    #[test]
    fn a_card_with_no_rules_text_generates_nothing() {
        let mut land = bolt();
        land.faces[0].oracle_text.clear();
        assert_eq!(
            generate(&land, &Library::default(), &BTreeSet::new()),
            Generation::Nothing
        );
    }

    #[test]
    fn a_card_whose_rules_fail_a_check_is_invalid_and_says_where() {
        // The compiler leaves a Scryfall keyword it cannot place to the check against the imported card.
        let mut card = bolt();
        card.oracle_id = "not an id".into();
        let outcome = generate(&card, &Library::default(), &BTreeSet::new());
        let Generation::Invalid(problems) = outcome else {
            panic!("a problem was expected, got {outcome:?}");
        };
        assert!(
            problems.iter().any(|p| p.path == "oracle_id"),
            "{problems:?}"
        );
    }

    #[test]
    fn the_version_is_a_digest_of_the_build() {
        assert_eq!(COMPILER_VERSION.len(), 16);
        assert!(COMPILER_VERSION.chars().all(|c| c.is_ascii_hexdigit()));
    }
}
