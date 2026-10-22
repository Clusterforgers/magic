//! Goal: every file of the design corpus loads, matches the imported Scryfall data, is in canonical form, reads back to the same value, plays the part its status says, and together they cover the vocabulary.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common;

use std::collections::BTreeSet;
use std::path::Path;

use mtg_cards::ability::Ability;
use mtg_cards::card::{Card, TokenFile};
use mtg_cards::keyword::Keyword;
use mtg_cards::load::{Library, canonical, parse_json};
use mtg_cards::support::{Caps, Support};
use mtg_cards::validate::used_primitives;
use rstest::rstest;

use common::{ADDING, Corpus, UPDATE, cards_dir, printed, update_repo_once};

fn library() -> Library {
    Corpus::repo().library()
}

/// Every file is in canonical form. With `MTG_UPDATE_CARDS=1` the run first rewrites them, with the fingerprints of
/// the imported text.
#[test]
fn files_are_canonical_and_carry_current_fingerprints() {
    update_repo_once();
    let stale = Corpus::repo().non_canonical();
    assert!(
        stale.is_empty(),
        "not in canonical form: {stale:?}; run `{UPDATE}` ({ADDING})"
    );
}

#[test]
fn the_corpus_loads_every_card_file_and_every_token_file() {
    let corpus = Corpus::repo();
    let library = library();
    assert_eq!(library.cards().count(), corpus.card_files().len());
    let stem = |path: &std::path::PathBuf| path.file_stem().unwrap().to_string_lossy().into_owned();
    let on_disk: Vec<String> = corpus.token_files().iter().map(stem).collect();
    let loaded: Vec<String> = library.tokens().map(|t| t.id.clone()).collect();
    assert_eq!(loaded, on_disk);
    for predefined in ["clue", "food", "treasure"] {
        assert!(loaded.iter().any(|id| id == predefined), "{predefined}");
    }
}

#[test]
fn every_card_file_has_exactly_one_fixture_record_and_the_reverse() {
    let problems = Corpus::repo().pairing_problems();
    assert!(problems.is_empty(), "{problems:#?}");
}

#[test]
fn the_fixture_records_have_distinct_names_and_the_corpus_has_as_many_cards_as_records() {
    let corpus = Corpus::repo();
    let records = corpus.records();
    let names: BTreeSet<&str> = records.iter().map(|record| record.name.as_str()).collect();
    assert_eq!(names.len(), records.len(), "a name twice ({ADDING})");
    assert_eq!(records.len(), library().cards().count(), "{ADDING}");
}

#[test]
fn the_fingerprints_of_every_card_are_current() {
    let problems = Corpus::repo().fingerprint_problems();
    assert!(problems.is_empty(), "{problems:#?}");
}

#[test]
fn the_corpus_matches_the_imported_cards() {
    let errors = library().check_printed(&printed());
    assert!(
        errors.is_empty(),
        "{}\n(a drifted fingerprint: run `{UPDATE}`; see {ADDING})",
        errors
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join("\n")
    );
}

#[test]
fn every_card_and_token_reads_back_as_the_same_value() {
    let library = library();
    for card in library.cards() {
        let json = serde_json::to_string(card).unwrap();
        assert_eq!(
            &serde_json::from_str::<Card>(&json).unwrap(),
            card,
            "{}",
            card.name
        );
        let text = canonical(card);
        assert_eq!(
            &parse_json::<Card>("x", &text).unwrap(),
            card,
            "{}",
            card.name
        );
    }
    for token in library.tokens() {
        let json = serde_json::to_string(token).unwrap();
        assert_eq!(&serde_json::from_str::<TokenFile>(&json).unwrap(), token);
    }
}

const REQUESTED: [&str; 31] = [
    "Lightning Bolt",
    "Counterspell",
    "Giant Growth",
    "Llanowar Elves",
    "Serra Angel",
    "Doom Blade",
    "Swords to Plowshares",
    "Wrath of God",
    "Rampant Growth",
    "Cultivate",
    "Brainstorm",
    "Opt",
    "Mulldrifter",
    "Rhystic Study",
    "Hero of Bladehold",
    "Anointed Procession",
    "Fireball",
    "Izzet Charm",
    "Eternal Witness",
    "Pacifism",
    "Bonesplitter",
    "Flooded Strand",
    "Dockside Extortionist",
    "Tireless Tracker",
    "Burst Lightning",
    "Faithless Looting",
    "Bloodbraid Elf",
    "Jace, the Mind Sculptor",
    "Delver of Secrets // Insectile Aberration",
    "Fire // Ice",
    "Sol Ring",
];

#[test]
fn the_requested_cards_are_all_there() {
    let library = library();
    for name in REQUESTED {
        assert!(library.card_by_name(name).is_some(), "{name} is missing");
    }
    for name in [
        "Archon of Cruelty",
        "Bonecrusher Giant // Stomp",
        "Felidar Sovereign",
    ] {
        assert!(library.card_by_name(name).is_some(), "{name} is missing");
    }
}

#[rstest]
#[case::everything_runs(Caps::everything())]
fn declared_status_decides_the_support_when_the_engine_runs_everything(#[case] caps: Caps) {
    let library = library();
    for shown in printed() {
        let record = library.support(&shown, &caps);
        let expected = match shown.name.as_str() {
            "Aven Mindcensor" | "Courser of Kruphix" => Support::Partial,
            _ => Support::Verified,
        };
        assert_eq!(
            record.support, expected,
            "{}: {:?}",
            shown.name, record.clauses
        );
        assert_eq!(record.fingerprint.len(), 32);
    }
}

#[test]
fn an_engine_that_runs_nothing_leaves_every_card_with_abilities_to_the_table() {
    let library = library();
    for shown in printed() {
        let record = library.support(&shown, &Caps::none());
        let abilities: usize = library.card(&shown.oracle_id).unwrap().abilities().count();
        let expected = match (shown.name.as_str(), abilities) {
            (_, 0) => Support::Verified,
            _ => Support::Manual,
        };
        assert_eq!(record.support, expected, "{}", shown.name);
    }
}

#[test]
fn a_changed_oracle_text_turns_the_card_stale() {
    let library = library();
    let mut bolt = printed()
        .into_iter()
        .find(|p| p.name == "Lightning Bolt")
        .unwrap();
    bolt.faces[0].oracle_text = "Lightning Bolt deals 4 damage to any target.".into();
    let record = library.support(&bolt, &Caps::everything());
    assert_eq!(record.support, Support::Stale);
    let drift: Vec<_> = library
        .check_printed(&[bolt])
        .into_iter()
        .filter(|e| e.file.ends_with("lightning_bolt.json"))
        .collect();
    assert_eq!(
        drift.len(),
        1,
        "the drift is reported once, at the fingerprint: {drift:?}"
    );
    assert_eq!(drift[0].path, "faces[0].oracle_fingerprint");
}

#[test]
fn keywords_in_the_data_appear_in_the_oracle_text() {
    let library = library();
    for shown in printed() {
        let card = library.card(&shown.oracle_id).unwrap();
        for (index, face) in card.faces.iter().enumerate() {
            let text = shown.faces[index].oracle_text.to_lowercase();
            for ability in &face.abilities {
                if let Ability::Keyword(keyword) = ability
                    && !matches!(keyword, Keyword::Fuse)
                {
                    assert!(
                        text.contains(&keyword.name().to_lowercase()),
                        "{}: the text has no `{}`",
                        card.name,
                        keyword.name()
                    );
                }
            }
        }
    }
}

#[test]
fn the_corpus_exercises_the_range_of_the_vocabulary() {
    let library = library();
    let mut used = BTreeSet::new();
    for card in library.cards() {
        for (_, ability) in card.abilities() {
            used.extend(used_primitives(ability));
        }
    }
    for token in library.tokens() {
        for ability in &token.token.abilities {
            used.extend(used_primitives(ability));
        }
    }
    let required = [
        "Ability.Spell",
        "Ability.Activated",
        "Ability.Loyalty",
        "Ability.Triggered",
        "Ability.Static",
        "Ability.Replacement",
        "Ability.CostModifier",
        "Ability.Manual",
        "Effect.Modal",
        "Effect.Pay",
        "Effect.May",
        "Effect.If",
        "Effect.DelayedTrigger",
        "Effect.Continuous",
        "Effect.Rule",
        "Effect.CreateToken",
        "Effect.Search",
        "Effect.Choose",
        "Effect.LookAtTop",
        "Effect.Move",
        "Effect.Sacrifice",
        "Effect.WinGame",
        "Trigger.AnyOf",
        "Trigger.ZoneChange",
        "Trigger.Attacks",
        "Trigger.StepBegins",
        "Trigger.SpellCast",
        "Trigger.Sacrificed",
        "Trigger.BecomesTarget",
        "Cost.PayLife",
        "Cost.SacrificeSelf",
        "Mod.ModifyPT",
        "Mod.CantAttack",
        "Mod.AddKeyword",
        "ReplaceWhen.TokenCreation",
        "Keyword.Equip",
        "Keyword.Kicker",
        "Keyword.Multikicker",
        "Condition.Kicked",
        "Amount.Kicks",
        "Keyword.Flashback",
        "Keyword.Evoke",
        "Keyword.Cascade",
        "Keyword.BattleCry",
        "Keyword.Prowess",
        "Keyword.Ward",
        "Keyword.Fuse",
        "Keyword.Enchant",
    ];
    let missing: Vec<_> = required
        .iter()
        .filter(|name| !used.contains(**name))
        .collect();
    assert!(missing.is_empty(), "no corpus card uses {missing:?}");
}

/// The Archon of Cruelty example in `docs/cards-dsl.md` is the data of the file, not a copy that can drift.
#[test]
fn the_worked_example_in_the_design_doc_is_the_file() {
    let doc = std::fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/cards-dsl.md"),
    )
    .unwrap();
    let section = doc
        .split("## Worked example: Archon of Cruelty")
        .nth(1)
        .unwrap();
    let block = section
        .split("```json")
        .nth(1)
        .unwrap()
        .split("```")
        .next()
        .unwrap();
    let in_doc: serde_json::Value = serde_json::from_str(block).unwrap();
    let in_file: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(cards_dir().join("archon_of_cruelty.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(in_doc, in_file);
}

/// Ignored by default (needs a download). Set `MTG_REAL_DATA_DIR` to a directory holding `oracle-cards-*.jsonl.gz`
/// (as for `mtg-import`'s real-data tests) and run `cargo test -p mtg-cards corpus:: -- --ignored`.
#[test]
#[ignore = "needs MTG_REAL_DATA_DIR"]
fn the_corpus_still_matches_a_fresh_oracle_cards_download() {
    use std::io::BufReader;

    let dir = std::path::PathBuf::from(
        std::env::var("MTG_REAL_DATA_DIR").expect("set MTG_REAL_DATA_DIR"),
    );
    let mut files: Vec<_> = std::fs::read_dir(dir)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| {
            let name = path.file_name().unwrap().to_string_lossy();
            name.starts_with("oracle-cards-") && name.ends_with(".jsonl.gz")
        })
        .collect();
    files.sort();
    let file = std::fs::File::open(files.pop().expect("an oracle-cards-*.jsonl.gz file")).unwrap();
    let library = library();
    let ours: BTreeSet<_> = library.cards().map(|card| card.oracle_id.clone()).collect();
    let fresh: Vec<_> =
        mtg_import::scryfall::cards_jsonl(flate2::read::GzDecoder::new(BufReader::new(file)))
            .filter_map(Result::ok)
            .filter(|record| ours.contains(record.oracle_id.as_str()))
            .map(|record| common::printed_of(&record))
            .collect();
    assert_eq!(
        fresh.len(),
        ours.len(),
        "every corpus card is in the download"
    );
    let errors = library.check_printed(&fresh);
    assert!(
        errors.is_empty(),
        "errata since the rules were written:\n{}",
        errors
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join("\n")
    );
}

#[test]
fn clause_spans_cut_out_the_sentences_they_name() {
    let library = library();
    let mut found = Vec::new();
    for shown in printed() {
        let card = library.card(&shown.oracle_id).unwrap();
        for (index, face) in card.faces.iter().enumerate() {
            for ability in &face.abilities {
                if let Ability::Manual { span, .. } = ability {
                    let clause = span.slice(&shown.faces[index].oracle_text).unwrap();
                    found.push((card.name.clone(), clause.to_owned()));
                }
            }
        }
    }
    let expected = [
        (
            "Aven Mindcensor",
            "If an opponent would search a library, that player searches the top four cards of that library instead.",
        ),
        (
            "Courser of Kruphix",
            "You may play lands from the top of your library.",
        ),
    ];
    assert_eq!(
        found,
        expected.map(|(name, clause)| (name.to_owned(), clause.to_owned()))
    );
}
