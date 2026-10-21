//! Goal: the oracle-text compiler turns every card of the design corpus into rules that load, match the imported text, earn the status their clauses say, and agree with the hand-written files on every keyword they read.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common;

use mtg_cards::ability::Ability;
use mtg_cards::card::ImplStatus::{Full, Manual, Partial};
use mtg_cards::card::{Card, ImplStatus};
use mtg_cards::compile::{Compiled, compile};
use mtg_cards::load::{Library, Source, canonical, slug};
use mtg_cards::support::{PrintedCard, oracle_fingerprint};

use common::{cards_dir, printed, token_sources};

/// Every corpus card with the status it compiles to (`None` when its text is only reminder text) and
/// how many of its clauses are read out of how many.
const EXPECTED: [(&str, Option<ImplStatus>, usize, usize); 120] = [
    ("Act of Treason", Some(Manual), 0, 1),
    ("Ajani, Caller of the Pride", Some(Manual), 0, 3),
    ("Anger", Some(Partial), 1, 2),
    ("Arvad the Cursed", Some(Full), 3, 3),
    ("Anointed Procession", Some(Manual), 0, 1),
    ("Archon of Cruelty", Some(Full), 2, 2),
    ("Batterskull", Some(Partial), 3, 4),
    ("Battlefield Forge", Some(Full), 2, 2),
    ("Black Knight", Some(Full), 2, 2),
    ("Bloodbraid Elf", Some(Full), 2, 2),
    ("Bonecrusher Giant // Stomp", Some(Manual), 0, 2),
    ("Armguard Familiar", Some(Partial), 2, 3),
    ("Bonesplitter", Some(Full), 2, 2),
    ("Bronzeplate Boar", Some(Full), 3, 3),
    ("Corpsejack Menace", Some(Manual), 0, 1),
    ("Doubling Season", Some(Manual), 0, 2),
    ("Faithful Watchdog", Some(Full), 2, 2),
    ("Furnace of Rath", Some(Manual), 0, 1),
    ("Hall of Triumph", Some(Full), 2, 2),
    ("Hardened Scales", Some(Manual), 0, 1),
    ("Idyllic Beachfront", Some(Full), 1, 1),
    ("Lizard Blades", Some(Full), 3, 3),
    ("Boneyard Scourge", Some(Partial), 1, 2),
    ("Brainstorm", Some(Full), 1, 1),
    ("Burst Lightning", Some(Partial), 1, 2),
    ("Caress of Phyrexia", Some(Manual), 0, 1),
    ("Carnage Tyrant", Some(Full), 3, 3),
    ("Cathars' Crusade", Some(Full), 1, 1),
    ("Control Magic", Some(Partial), 1, 2),
    ("Counterspell", Some(Full), 1, 1),
    ("Courtly Provocateur", Some(Manual), 0, 2),
    ("Covetous Dragon", Some(Partial), 1, 2),
    ("Cultivate", Some(Manual), 0, 1),
    ("Darksteel Juggernaut", Some(Partial), 2, 3),
    ("Death Ward", Some(Full), 1, 1),
    (
        "Delver of Secrets // Insectile Aberration",
        Some(Partial),
        1,
        2,
    ),
    ("Dockside Extortionist", Some(Full), 1, 1),
    ("Doom Blade", Some(Full), 1, 1),
    ("Drudge Skeletons", Some(Full), 1, 1),
    ("Elemental Masterpiece", Some(Partial), 1, 2),
    ("Epic Proportions", Some(Full), 3, 3),
    ("Eternal Witness", Some(Full), 1, 1),
    ("Faithless Looting", Some(Full), 2, 2),
    ("Favorable Winds", Some(Full), 1, 1),
    ("Felidar Sovereign", Some(Full), 3, 3),
    ("Fire // Ice", Some(Partial), 2, 3),
    ("Fireball", Some(Manual), 0, 2),
    ("Flooded Strand", Some(Full), 1, 1),
    ("Fog", Some(Manual), 0, 1),
    ("Forest", None, 0, 0),
    ("Gaea's Cradle", Some(Manual), 0, 1),
    ("Lotus Petal", Some(Full), 1, 1),
    ("Garruk Wildspeaker", Some(Manual), 0, 3),
    ("Gatekeeper of Malakir", Some(Partial), 1, 2),
    ("Giant Growth", Some(Full), 1, 1),
    ("Glorious Anthem", Some(Full), 1, 1),
    ("Goblin Chieftain", Some(Partial), 1, 2),
    ("Goblin Spy", Some(Full), 1, 1),
    ("Grizzly Bears", None, 0, 0),
    ("Hero of Bladehold", Some(Full), 2, 2),
    ("Hulking Raptor", Some(Full), 2, 2),
    ("Humility", Some(Manual), 0, 1),
    ("Inspiring Call", Some(Manual), 0, 1),
    ("Izzet Charm", Some(Full), 1, 1),
    ("Jace Beleren", Some(Manual), 0, 3),
    ("Jace, the Mind Sculptor", Some(Manual), 0, 4),
    ("Jungle Lion", Some(Full), 1, 1),
    ("Kongming, \"Sleeping Dragon\"", Some(Full), 1, 1),
    ("Kormus Bell", Some(Manual), 0, 1),
    ("Levitation", Some(Full), 1, 1),
    ("Lightning Bolt", Some(Full), 1, 1),
    ("Llanowar Elves", Some(Full), 1, 1),
    ("Maralen of the Mornsong", Some(Manual), 0, 2),
    ("Kor Firewalker", Some(Partial), 1, 2),
    ("Maro", Some(Manual), 0, 1),
    ("Mending Hands", Some(Manual), 0, 1),
    ("Mindlock Orb", Some(Manual), 0, 1),
    ("Monastery Swiftspear", Some(Full), 2, 2),
    ("Mother of Runes", Some(Manual), 0, 1),
    ("Mulldrifter", Some(Full), 3, 3),
    ("Naban, Dean of Iteration", Some(Manual), 0, 1),
    ("Nightmare", Some(Partial), 1, 2),
    ("Opt", Some(Full), 2, 2),
    ("Pacifism", Some(Full), 2, 2),
    ("Panharmonicon", Some(Manual), 0, 1),
    ("Parallel Lives", Some(Manual), 0, 1),
    ("Questing Beast", Some(Partial), 3, 6),
    ("Rally the Ranks", Some(Full), 2, 2),
    ("Rampaging Ferocidon", Some(Partial), 1, 3),
    ("Rampant Growth", Some(Full), 1, 1),
    ("Reassembling Skeleton", Some(Manual), 0, 1),
    ("Rest in Peace", Some(Manual), 0, 2),
    ("Rhox Faithmender", Some(Partial), 1, 2),
    ("Rhystic Study", Some(Full), 1, 1),
    ("Samite Healer", Some(Manual), 0, 1),
    ("Serra Angel", Some(Full), 2, 2),
    ("Sneak Attack", Some(Manual), 0, 1),
    ("Sodden Verdure", Some(Full), 1, 1),
    ("Sol Ring", Some(Full), 1, 1),
    ("Soul of Zendikar", Some(Partial), 2, 3),
    ("Shielded by Faith", Some(Partial), 2, 3),
    ("Spellbreaker Behemoth", Some(Partial), 1, 2),
    ("Sram, Senior Edificer", Some(Manual), 0, 1),
    ("Squee, Goblin Nabob", Some(Manual), 0, 1),
    ("Squirrel Nest", Some(Partial), 1, 2),
    ("Steam Vents", Some(Full), 1, 1),
    ("Supreme Verdict", Some(Full), 2, 2),
    ("Swiftfoot Boots", Some(Full), 2, 2),
    ("Swords to Plowshares", Some(Full), 1, 1),
    ("Teysa Karlov", Some(Manual), 0, 2),
    ("Thought Reflection", Some(Manual), 0, 1),
    ("Tireless Tracker", Some(Full), 2, 2),
    ("Transguild Courier", Some(Manual), 0, 1),
    ("Warmonger Hellkite", Some(Full), 3, 3),
    ("Wolfbriar Elemental", Some(Partial), 1, 2),
    ("Wrath of God", Some(Full), 1, 1),
    ("Blaze", Some(Full), 1, 1),
    ("Stroke of Genius", Some(Full), 1, 1),
    ("Kessig Wolf Run", Some(Full), 2, 2),
    ("Tezzeret the Seeker", Some(Manual), 0, 3),
];

fn compiled_corpus() -> Vec<(PrintedCard, Compiled)> {
    printed()
        .into_iter()
        .map(|card| {
            let compiled = compile(&card);
            (card, compiled)
        })
        .collect()
}

fn source_of(card: &Card) -> Source {
    Source {
        path: format!("cards/{}.json", slug(&card.name)),
        text: canonical(card),
    }
}

#[test]
fn the_table_lists_every_corpus_card_once() {
    let mut names: Vec<_> = printed().into_iter().map(|card| card.name).collect();
    let mut listed: Vec<_> = EXPECTED.iter().map(|row| row.0.to_owned()).collect();
    names.sort();
    listed.sort();
    assert_eq!(names, listed);
}

#[test]
fn every_corpus_card_earns_the_status_in_the_table() {
    let corpus = compiled_corpus();
    assert_eq!(corpus.len(), 120);
    let mut differing = Vec::new();
    for (card, compiled) in &corpus {
        let row = EXPECTED.iter().find(|row| row.0 == card.name).unwrap();
        let status = compiled.card.as_ref().map(|card| card.impl_status);
        let read = compiled.clauses.iter().filter(|clause| clause.read).count();
        let found = (status, read, compiled.clauses.len());
        if found != (row.1, row.2, row.3) {
            differing.push(format!(
                "{}: {found:?}, expected {:?}",
                card.name,
                (row.1, row.2, row.3)
            ));
        }
    }
    assert!(differing.is_empty(), "{differing:#?}");
}

#[test]
fn every_compiled_card_loads_and_matches_the_imported_text() {
    let corpus = compiled_corpus();
    let mut sources: Vec<Source> = corpus
        .iter()
        .filter_map(|(_, compiled)| compiled.card.as_ref())
        .map(source_of)
        .collect();
    assert_eq!(sources.len(), 118);
    sources.extend(token_sources());
    let library = Library::from_sources(&sources).unwrap_or_else(|errors| panic!("{errors}"));
    let imported: Vec<PrintedCard> = corpus.into_iter().map(|(card, _)| card).collect();
    let problems = library.check_printed(&imported);
    assert!(problems.is_empty(), "{problems:?}");
}

#[test]
fn the_files_the_compiler_writes_are_canonical_and_read_back_unchanged() {
    for (_, compiled) in compiled_corpus() {
        let Some(card) = compiled.card else { continue };
        let text = canonical(&card);
        let again: Card = mtg_cards::load::parse_json("compiled", &text).unwrap();
        assert_eq!(again, card);
        assert_eq!(canonical(&again), text);
    }
}

#[test]
fn live_cards_carry_the_fingerprint_of_every_face() {
    for (printed, compiled) in compiled_corpus() {
        let Some(card) = compiled.card else { continue };
        for (face, shown) in card.faces.iter().zip(&printed.faces) {
            let expected = if card.impl_status == Manual {
                String::new()
            } else {
                oracle_fingerprint(&shown.oracle_text)
            };
            assert_eq!(face.oracle_fingerprint, expected, "{}", printed.name);
        }
    }
}

#[test]
fn clauses_lie_inside_the_text_and_never_overlap() {
    for (printed, compiled) in compiled_corpus() {
        for (at, clause) in compiled.clauses.iter().enumerate() {
            let text = &printed.faces[clause.face].oracle_text;
            let length = u32::try_from(text.chars().count()).unwrap();
            assert!(clause.span.start < clause.span.end && clause.span.end <= length);
            assert!(clause.span.slice(text).is_some(), "{}", printed.name);
            assert!(
                compiled.clauses[..at]
                    .iter()
                    .filter(|earlier| earlier.face == clause.face)
                    .all(|earlier| !earlier.span.overlaps(&clause.span)),
                "{}",
                printed.name
            );
        }
    }
}

#[test]
fn a_live_card_has_one_manual_ability_for_each_unread_clause() {
    for (printed, compiled) in compiled_corpus() {
        let Some(card) = compiled.card else { continue };
        let unread = compiled
            .clauses
            .iter()
            .filter(|clause| !clause.read)
            .count();
        let manual = card
            .abilities()
            .filter(|(_, ability)| ability.is_manual())
            .count();
        let expected = if card.impl_status == Manual {
            0
        } else {
            unread
        };
        assert_eq!(manual, expected, "{}", printed.name);
    }
}

/// The keywords the compiler reads are among the abilities of the hand-written file of the same card, face by face.
#[test]
fn the_keywords_read_are_in_the_hand_written_files() {
    let hand_written = Library::load_dir(&cards_dir()).unwrap_or_else(|errors| panic!("{errors}"));
    for (printed, compiled) in compiled_corpus() {
        let Some(card) = compiled.card else { continue };
        let by_hand = hand_written.card(&printed.oracle_id).unwrap();
        for (face, ability) in card.abilities() {
            if matches!(ability, Ability::Keyword(_)) {
                assert!(
                    by_hand.faces[face].abilities.contains(ability),
                    "{}: {ability:?} is not in the hand-written file",
                    printed.name
                );
            }
        }
    }
}
