//! Goal: a card added to a copy of the card directory and the fixture edits no count or table (the pairing, the fingerprints, the compiler's status and sources and the loaded library follow from the files, and the ratchet asks for one name only when the compiler reproduces the card), and a missing half or a stale fingerprint is reported with the card, the command and the checklist.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common::{ADDING, Corpus, UPDATE};
use crate::compile_corpus::{list_text, ratchet_problems, reproduced};

use std::collections::BTreeSet;
use std::path::Path;

use mtg_cards::load::slug;
use rstest::rstest;
use serde_json::{Value, json};

const TWIN_ORACLE_ID: &str = "00000000-0000-4000-8000-0000000000a1";
const TWIN_PRINTING_ID: &str = "00000000-0000-4000-8000-0000000000b1";

fn copy_dir(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).unwrap();
    for entry in std::fs::read_dir(from).unwrap().flatten() {
        let target = to.join(entry.file_name());
        if entry.path().is_dir() {
            copy_dir(&entry.path(), &target);
        } else {
            std::fs::copy(entry.path(), target).unwrap();
        }
    }
}

/// The repository's corpus copied under `root`.
fn copy_of_repo(root: &Path) -> Corpus {
    let repo = Corpus::repo();
    let (dir, fixture) = (root.join("cards"), root.join("corpus"));
    copy_dir(&repo.dir, &dir);
    copy_dir(&repo.fixture, &fixture);
    Corpus { dir, fixture }
}

fn read_record(corpus: &Corpus, name: &str) -> Value {
    let path = corpus.fixture.join(format!("{}.json", slug(name)));
    serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()
}

/// Writes the record as the file of the card `name`: pretty-printed with a trailing newline.
fn write_record(corpus: &Corpus, name: &str, record: &Value) {
    let path = corpus.fixture.join(format!("{}.json", slug(name)));
    let text = serde_json::to_string_pretty(record).unwrap() + "\n";
    std::fs::write(path, text).unwrap();
}

fn read_card(corpus: &Corpus, name: &str) -> Value {
    let path = corpus.dir.join(format!("{}.json", slug(name)));
    serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()
}

fn write_card(corpus: &Corpus, name: &str, card: &Value) {
    let path = corpus.dir.join(format!("{}.json", slug(name)));
    std::fs::write(path, serde_json::to_string_pretty(card).unwrap()).unwrap();
}

/// Adds a card that is `name` under another name: its file, and its record in the fixture.
fn add_twin(corpus: &Corpus, name: &str, twin: &str) {
    let mut card = read_card(corpus, name);
    card["oracle_id"] = json!(TWIN_ORACLE_ID);
    card["name"] = json!(twin);
    card["faces"][0]["name"] = json!(twin);
    write_card(corpus, twin, &card);

    let mut record = read_record(corpus, name);
    record["id"] = json!(TWIN_PRINTING_ID);
    record["oracle_id"] = json!(TWIN_ORACLE_ID);
    record["name"] = json!(twin);
    write_record(corpus, twin, &record);
}

fn assert_clean(problems: &[String]) {
    assert!(problems.is_empty(), "{problems:#?}");
}

#[rstest]
#[case::a_spell_the_compiler_reproduces("Giant Growth", true)]
#[case::a_land_with_types_the_compiler_reproduces("Steam Vents", true)]
#[case::a_creature_without_rules_text("Grizzly Bears", false)]
#[case::a_basic_land("Forest", false)]
#[case::a_card_part_of_which_stays_to_the_table("Aven Mindcensor", false)]
fn a_new_card_needs_its_file_its_record_and_at_most_one_name_in_the_ratchet(
    #[case] name: &str,
    #[case] reproduced_by_the_compiler: bool,
) {
    let temp = tempfile::tempdir().unwrap();
    let corpus = copy_of_repo(temp.path());
    let twin = format!("{name} Twin");
    let files_before = corpus.card_files().len();
    let reproduced_before = reproduced(&corpus);
    let list = list_text(&reproduced_before);
    assert_clean(&ratchet_problems(&reproduced_before, &list));

    add_twin(&corpus, name, &twin);

    // Every count follows from the files.
    assert_eq!(corpus.card_files().len(), files_before + 1);
    assert_eq!(corpus.fixture_files().len(), files_before + 1);
    assert_eq!(corpus.records().len(), files_before + 1);
    assert_eq!(corpus.printed().len(), files_before + 1);
    assert_eq!(corpus.library().cards().count(), files_before + 1);
    // Every invariant a count stood for holds without an edit.
    assert_clean(&corpus.fixture_problems());
    assert_clean(&corpus.pairing_problems());
    assert_clean(&corpus.fingerprint_problems());
    assert_clean(&corpus.status_problems());
    assert_clean(&corpus.compiled_problems());

    // The ratchet asks for the new card's name when the compiler reproduces it, and for nothing else.
    let found = reproduced(&corpus);
    assert!(found.is_superset(&reproduced_before));
    assert_eq!(found.contains(&twin), reproduced_by_the_compiler);
    let problems = ratchet_problems(&found, &list);
    if reproduced_by_the_compiler {
        assert_eq!(problems.len(), 1, "{problems:#?}");
        assert!(problems[0].contains(&twin) && problems[0].contains(UPDATE));
        let mut listed = reproduced_before;
        listed.insert(twin);
        assert_clean(&ratchet_problems(&found, &list_text(&listed)));
    } else {
        assert_clean(&problems);
    }
}

#[test]
fn a_record_without_a_file_and_a_file_without_a_record_are_reported_with_the_card_and_the_checklist()
 {
    let temp = tempfile::tempdir().unwrap();
    let corpus = copy_of_repo(temp.path());
    assert_clean(&corpus.pairing_problems());

    std::fs::remove_file(corpus.dir.join("giant_growth.json")).unwrap();
    std::fs::remove_file(corpus.fixture.join("lightning_bolt.json")).unwrap();

    let problems = corpus.pairing_problems();
    assert_eq!(problems.len(), 2, "{problems:#?}");
    let of = |card: &str| problems.iter().find(|p| p.contains(card)).unwrap();
    assert!(
        of("Giant Growth").contains("add cards/giant_growth.json")
            && of("Giant Growth").contains(UPDATE)
            && of("Giant Growth").contains(ADDING)
    );
    assert!(
        of("Lightning Bolt").contains("tests/fixtures/corpus/lightning_bolt.json")
            && of("Lightning Bolt").contains(ADDING)
    );
}

#[test]
fn a_fixture_file_must_be_named_after_its_card_and_hold_one_card_once() {
    let temp = tempfile::tempdir().unwrap();
    let corpus = copy_of_repo(temp.path());
    assert_clean(&corpus.fixture_problems());

    // A misnamed file, a file without its final newline and a second file with the oracle id of Lightning Bolt.
    let from = corpus.fixture.join("giant_growth.json");
    std::fs::rename(&from, corpus.fixture.join("growth.json")).unwrap();
    let bolt = corpus.fixture.join("lightning_bolt.json");
    let text = std::fs::read_to_string(&bolt).unwrap();
    std::fs::write(&bolt, text.trim_end()).unwrap();
    std::fs::write(corpus.fixture.join("bolt_again.json"), &text).unwrap();

    let problems = corpus.fixture_problems();
    assert_eq!(problems.len(), 3, "{problems:#?}");
    let of = |file: &str| {
        let file = format!("tests/fixtures/corpus/{file} ");
        problems.iter().find(|p| p.contains(&file)).unwrap()
    };
    assert!(of("growth.json").contains("giant_growth.json") && of("growth.json").contains(ADDING));
    assert!(of("bolt_again.json").contains("lightning_bolt.json"));
    let newline = problems.iter().find(|p| p.contains("newline")).unwrap();
    assert!(newline.contains("lightning_bolt.json") && newline.contains(ADDING));

    let pairing = corpus.pairing_problems();
    assert_eq!(pairing.len(), 1, "{pairing:#?}");
    assert!(pairing[0].contains("Lightning Bolt") && pairing[0].contains("one oracle id"));
}

#[test]
fn a_missing_or_stale_fingerprint_is_reported_with_the_card_and_the_command() {
    let temp = tempfile::tempdir().unwrap();
    let corpus = copy_of_repo(temp.path());
    assert_clean(&corpus.fingerprint_problems());

    let mut card = read_card(&corpus, "Giant Growth");
    card["faces"][0]["oracle_fingerprint"] = json!("");
    write_card(&corpus, "Giant Growth", &card);
    let mut record = read_record(&corpus, "Lightning Bolt");
    record["oracle_text"] = json!("Lightning Bolt deals 4 damage to any target.");
    write_record(&corpus, "Lightning Bolt", &record);

    let problems = corpus.fingerprint_problems();
    assert_eq!(problems.len(), 2, "{problems:#?}");
    for (card, state) in [("Giant Growth", "missing"), ("Lightning Bolt", "stale")] {
        let line = problems.iter().find(|p| p.starts_with(card)).unwrap();
        assert!(
            line.contains(state) && line.contains(UPDATE) && line.contains(ADDING),
            "{line}"
        );
    }
}

#[test]
fn the_ratchet_keeps_a_listed_card_and_wants_a_reproduced_one_listed_in_sorted_order() {
    let found: BTreeSet<String> = ["Alpha", "Beta"].map(str::to_owned).into();
    assert_clean(&ratchet_problems(&found, "Alpha\nBeta\n"));

    let lost = ratchet_problems(&found, "Alpha\nBeta\nGamma\n");
    assert_eq!(lost.len(), 1, "{lost:#?}");
    assert!(lost[0].contains("no longer reproduced") && lost[0].contains("Gamma"));

    let new = ratchet_problems(&found, "Alpha\n");
    assert_eq!(new.len(), 1, "{new:#?}");
    assert!(new[0].contains("Beta") && new[0].contains(UPDATE));

    for text in ["Beta\nAlpha\n", "Alpha\nAlpha\nBeta\n", "Alpha\nBeta"] {
        let problems = ratchet_problems(&found, text);
        assert_eq!(problems.len(), 1, "{text:?}: {problems:#?}");
        assert!(problems[0].contains("sorted"), "{text:?}");
    }
}
