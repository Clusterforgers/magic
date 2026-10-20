//! The oracle-text compiler: an imported card's text becomes rules data, and what the compiler
//! cannot read stays a `Manual` clause with its span (`docs/cards-dsl.md`, Compiler and Manual fallback).
//!
//! Five pure stages, one file each: `normalize` strips reminder text and the card's own name,
//! `clauses` splits the text, `classify` names each clause's shape, `keywords` reads keyword
//! lines and `manual` leaves every other clause to the table.

mod classify;
mod clauses;
mod keywords;
mod manual;
mod normalize;

use crate::ability::Ability;
use crate::card::{Card, Face, ImplStatus, Span};
use crate::support::{PrintedCard, PrintedFace, has_rules_text, oracle_fingerprint};

pub use classify::ClauseKind;

/// One clause of a face's oracle text, read or not.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Clause {
    /// The face, as an index into the card's faces.
    pub face: usize,
    /// Where the clause lies in the face's imported oracle text.
    pub span: Span,
    /// The clause as the compiler saw it: reminder text removed and the card's name as `~`.
    pub text: String,
    /// Its shape.
    pub kind: ClauseKind,
    /// Whether the compiler turned it into rules; otherwise it is a `Manual` ability.
    pub read: bool,
}

/// What the compiler made of one card.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Compiled {
    /// The rules, or `None` when no face has rules text once reminder text is removed.
    pub card: Option<Card>,
    /// Every clause of every face in text order, read or not, for the coverage report.
    pub clauses: Vec<Clause>,
}

/// Compiles an imported card: every clause is read or left to the table as a `Manual` ability.
///
/// The status is `Full` when every clause is read, `Partial` when some are, and `Manual` (no
/// abilities) when none is. Every face of a `Full` or `Partial` card carries the fingerprint of its text.
pub fn compile(printed: &PrintedCard) -> Compiled {
    let mut listed = Vec::new();
    let mut faces = Vec::with_capacity(printed.faces.len());
    for (index, shown) in printed.faces.iter().enumerate() {
        let text = normalize::normalize(&shown.oracle_text, &own_names(shown));
        let spell = is_spell(shown);
        let mut abilities = Vec::new();
        for raw in clauses::split(&text) {
            let clause_text = text.slice(raw.range.clone());
            let Some(span) = text.span(raw.range, raw.ends_line) else {
                continue;
            };
            let kind = classify::classify(&clause_text, spell);
            let keyword = (kind == ClauseKind::Keyword)
                .then(|| keywords::read(&clause_text))
                .flatten();
            let read = keyword.is_some();
            abilities.push(keyword.map_or_else(|| manual::manual(kind, span), Ability::Keyword));
            listed.push(Clause {
                face: index,
                span,
                text: clause_text,
                kind,
                read,
            });
        }
        faces.push(Face {
            name: shown.name.clone(),
            oracle_fingerprint: oracle_fingerprint(&shown.oracle_text),
            abilities,
        });
    }
    let card = build(printed, faces, &mut listed);
    Compiled {
        card,
        clauses: listed,
    }
}

/// The face name, and for a legendary face the part of the name before the comma.
fn own_names(face: &PrintedFace) -> Vec<String> {
    let mut names = vec![face.name.clone()];
    let legendary = face
        .type_line
        .split_whitespace()
        .any(|word| word == "Legendary");
    if legendary && let Some((short, _)) = face.name.split_once(',') {
        names.push(short.trim().to_owned());
    }
    names
}

/// Whether the face is an instant or a sorcery.
fn is_spell(face: &PrintedFace) -> bool {
    let types = face.type_line.split('\u{2014}').next().unwrap_or_default();
    types
        .split_whitespace()
        .any(|word| matches!(word, "Instant" | "Sorcery"))
}

/// The card with its status, or `None` when no face has rules text.
fn build(printed: &PrintedCard, mut faces: Vec<Face>, listed: &mut [Clause]) -> Option<Card> {
    if !printed
        .faces
        .iter()
        .any(|face| has_rules_text(&face.oracle_text))
    {
        return None;
    }
    let read = listed.iter().filter(|clause| clause.read).count();
    let unread = listed.len() - read;
    // A `Full` instant or sorcery must have a `Spell` ability (`check_printed`), which only keyword lines cannot give.
    let spell_missing = printed.faces.iter().zip(&faces).any(|(shown, face)| {
        is_spell(shown)
            && !face
                .abilities
                .iter()
                .any(|ability| matches!(ability, Ability::Spell { .. }))
    });
    let impl_status = if read == 0 || (unread == 0 && spell_missing) {
        ImplStatus::Manual
    } else if unread == 0 {
        ImplStatus::Full
    } else {
        ImplStatus::Partial
    };
    if impl_status == ImplStatus::Manual {
        for face in &mut faces {
            face.abilities.clear();
            face.oracle_fingerprint.clear();
        }
        for clause in listed.iter_mut() {
            clause.read = false;
        }
    }
    Some(Card {
        oracle_id: printed.oracle_id.clone(),
        name: printed.name.clone(),
        impl_status,
        faces,
        notes: None,
    })
}

#[cfg(test)]
mod tests {
    //! Goal: a card compiles to the status its clauses earn, with fingerprints, names and spans that the card files require.
    use super::*;
    use crate::keyword::Keyword;
    use rstest::rstest;

    fn face(name: &str, type_line: &str, oracle_text: &str) -> PrintedFace {
        PrintedFace {
            name: name.into(),
            type_line: type_line.into(),
            oracle_text: oracle_text.into(),
            ..PrintedFace::default()
        }
    }

    fn card(faces: Vec<PrintedFace>) -> PrintedCard {
        let name = faces
            .iter()
            .map(|face| face.name.as_str())
            .collect::<Vec<_>>()
            .join(" // ");
        PrintedCard {
            oracle_id: "4457ed35-7c10-48c8-9776-456485fdf070".into(),
            name,
            keywords: vec![],
            faces,
        }
    }

    fn one(name: &str, type_line: &str, text: &str) -> Compiled {
        compile(&card(vec![face(name, type_line, text)]))
    }

    fn status(compiled: &Compiled) -> Option<ImplStatus> {
        compiled.card.as_ref().map(|card| card.impl_status)
    }

    #[rstest]
    #[case::only_keywords("Creature \u{2014} Angel", "Flying\nVigilance (Attacking doesn't cause this creature to tap.)", Some(ImplStatus::Full))]
    #[case::keyword_list("Creature \u{2014} Beast", "Vigilance, deathtouch, haste", Some(ImplStatus::Full))]
    #[case::keyword_and_trigger("Creature \u{2014} Elf", "Haste\nWhen ~ enters, draw a card.", Some(ImplStatus::Partial))]
    #[case::keyword_and_static("Artifact \u{2014} Equipment", "Equipped creature gets +2/+0.\nEquip {1}", Some(ImplStatus::Partial))]
    #[case::nothing_read("Instant", "~ deals 3 damage to any target.", Some(ImplStatus::Manual))]
    #[case::unreadable_keyword_only("Creature \u{2014} Elf", "Undying", Some(ImplStatus::Manual))]
    #[case::reminder_only("Basic Land \u{2014} Forest", "({T}: Add {G}.)", None)]
    #[case::vanilla("Creature \u{2014} Bear", "", None)]
    #[case::whitespace_only("Creature \u{2014} Bear", "  \n ", None)]
    #[case::spell_with_only_a_keyword("Instant", "Flash", Some(ImplStatus::Manual))]
    #[case::spell_with_keyword_and_effect("Instant", "Kicker {1}\nDraw a card.", Some(ImplStatus::Partial))]
    fn the_status_follows_the_clauses(
        #[case] type_line: &str,
        #[case] text: &str,
        #[case] expected: Option<ImplStatus>,
    ) {
        assert_eq!(status(&one("Test Card", type_line, text)), expected);
    }

    #[test]
    fn a_full_card_has_a_keyword_per_clause_and_the_fingerprint_of_its_text() {
        let text = "Flying\nVigilance (Attacking doesn't cause this creature to tap.)";
        let compiled = one("Serra Angel", "Creature \u{2014} Angel", text);
        let card = compiled.card.unwrap();
        assert_eq!(card.name, "Serra Angel");
        assert_eq!(card.faces[0].oracle_fingerprint, oracle_fingerprint(text));
        assert_eq!(
            card.faces[0].abilities,
            [
                Ability::Keyword(Keyword::Flying),
                Ability::Keyword(Keyword::Vigilance)
            ]
        );
        assert!(compiled.clauses.iter().all(|clause| clause.read));
        assert_eq!(compiled.clauses[1].text, "Vigilance");
        assert_eq!(
            compiled.clauses[1].span.slice(text),
            Some("Vigilance (Attacking doesn't cause this creature to tap.)")
        );
    }

    #[test]
    fn a_partial_card_keeps_a_manual_ability_for_each_unread_clause_in_order() {
        let text = "Flying\nWhenever ~ attacks, draw a card.\n{T}: Add {G}.\nVigilance";
        let compiled = one("Test Card", "Creature \u{2014} Bird", text);
        let card = compiled.card.unwrap();
        let abilities = &card.faces[0].abilities;
        assert_eq!(abilities.len(), 4);
        assert_eq!(abilities[0], Ability::Keyword(Keyword::Flying));
        assert_eq!(abilities[3], Ability::Keyword(Keyword::Vigilance));
        let Ability::Manual { reason, span } = &abilities[1] else {
            panic!("{:?}", abilities[1]);
        };
        assert_eq!(reason, "a triggered ability the compiler does not read yet");
        assert_eq!(span.slice(text), Some("Whenever ~ attacks, draw a card."));
        assert_eq!(
            abilities[2],
            Ability::Manual {
                reason: "an activated ability the compiler does not read yet".into(),
                span: Span { start: 40, end: 53 }
            }
        );
        let kinds: Vec<_> = compiled.clauses.iter().map(|clause| clause.kind).collect();
        assert_eq!(
            kinds,
            [
                ClauseKind::Keyword,
                ClauseKind::Triggered,
                ClauseKind::Activated,
                ClauseKind::Keyword
            ]
        );
        let read: Vec<_> = compiled.clauses.iter().map(|clause| clause.read).collect();
        assert_eq!(read, [true, false, false, true]);
    }

    #[test]
    fn a_manual_card_has_no_abilities_and_lists_its_clauses() {
        let compiled = one("Test Card", "Instant", "Counter target spell.\nDraw a card.");
        let card = compiled.card.unwrap();
        assert_eq!(card.impl_status, ImplStatus::Manual);
        assert!(card.faces[0].abilities.is_empty());
        assert!(card.faces[0].oracle_fingerprint.is_empty());
        assert_eq!(compiled.clauses.len(), 2);
        assert!(compiled.clauses.iter().all(|clause| {
            !clause.read && clause.kind == ClauseKind::Spell
        }));
    }

    #[test]
    fn a_text_with_only_reminder_text_has_no_card_and_no_clauses() {
        let compiled = one("Forest", "Basic Land \u{2014} Forest", "({T}: Add {G}.)");
        assert_eq!(compiled.card, None);
        assert!(compiled.clauses.is_empty());
    }

    #[test]
    fn every_face_of_a_double_faced_card_carries_its_own_fingerprint_and_name() {
        let front = "Haste\nAt the beginning of your upkeep, transform Delver of Secrets.";
        let back = "Flying";
        let compiled = compile(&card(vec![
            face("Delver of Secrets", "Creature \u{2014} Human Wizard", front),
            face("Insectile Aberration", "Creature \u{2014} Human Insect", back),
        ]));
        let card = compiled.card.unwrap();
        assert_eq!(card.name, "Delver of Secrets // Insectile Aberration");
        assert_eq!(card.impl_status, ImplStatus::Partial);
        assert_eq!(card.faces[0].oracle_fingerprint, oracle_fingerprint(front));
        assert_eq!(card.faces[1].oracle_fingerprint, oracle_fingerprint(back));
        assert_eq!(card.faces[1].abilities, [Ability::Keyword(Keyword::Flying)]);
        assert_eq!(
            compiled.clauses.iter().map(|clause| clause.face).collect::<Vec<_>>(),
            [0, 0, 1]
        );
        assert_eq!(
            compiled.clauses[1].text,
            "At the beginning of your upkeep, transform ~."
        );
    }

    #[test]
    fn a_face_without_text_next_to_a_face_with_text_still_gets_a_fingerprint() {
        let compiled = compile(&card(vec![
            face("Front", "Creature \u{2014} Bear", ""),
            face("Back", "Creature \u{2014} Bear", "Flying"),
        ]));
        let card = compiled.card.unwrap();
        assert_eq!(card.impl_status, ImplStatus::Full);
        assert_eq!(card.faces[0].oracle_fingerprint, oracle_fingerprint(""));
        assert!(card.faces[0].abilities.is_empty());
    }

    #[rstest]
    #[case::full_name("Atraxa, Praetors' Voice", "Legendary Creature \u{2014} Phyrexian", "Atraxa, Praetors' Voice has flying.", "~ has flying.")]
    #[case::short_name("Atraxa, Praetors' Voice", "Legendary Creature \u{2014} Phyrexian", "Whenever Atraxa attacks, draw a card.", "Whenever ~ attacks, draw a card.")]
    #[case::not_legendary("Fire, Ice", "Instant", "Fire deals 2 damage. Fire, Ice deals 1.", "Fire deals 2 damage. ~ deals 1.")]
    #[case::no_comma("Questing Beast", "Legendary Creature \u{2014} Beast", "Questing Beast can't be blocked.", "~ can't be blocked.")]
    fn the_card_names_become_tildes_in_the_clause_text(
        #[case] name: &str,
        #[case] type_line: &str,
        #[case] text: &str,
        #[case] expected: &str,
    ) {
        let compiled = one(name, type_line, text);
        assert_eq!(compiled.clauses[0].text, expected);
    }

    #[test]
    fn spans_stay_in_the_original_text_when_names_and_reminders_are_removed() {
        let text = "Haste (This creature can attack as soon as it comes under your control.)\nWhenever Atraxa attacks, draw a card. (It is nice.)\n\u{2212}X: Atraxa deals X damage.";
        let compiled = one(
            "Atraxa, Praetors' Voice",
            "Legendary Planeswalker \u{2014} Phyrexian",
            text,
        );
        let slices: Vec<_> = compiled
            .clauses
            .iter()
            .map(|clause| clause.span.slice(text).unwrap())
            .collect();
        assert_eq!(
            slices,
            [
                "Haste (This creature can attack as soon as it comes under your control.)",
                "Whenever Atraxa attacks, draw a card. (It is nice.)",
                "\u{2212}X: Atraxa deals X damage."
            ]
        );
        assert_eq!(compiled.clauses[2].kind, ClauseKind::Loyalty);
        assert_eq!(compiled.clauses[2].text, "\u{2212}X: ~ deals X damage.");
    }
}
