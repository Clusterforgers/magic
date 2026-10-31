//! Goal: the Forge hint finds a card's script by the file-name rule or by the `Name:` lines of its folder (back faces, apostrophes, commas, typographic quotes), keeps only the lines that say what the card does, cuts at the limits on a line boundary, gives a card with no script nothing, and reads each folder only when a card needs it.
use std::fs;

use mtg_cards::PrintedFace;
use rstest::rstest;

use super::*;

const FIXTURE: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/forge_hint");

fn hints() -> ForgeHints {
    ForgeHints::open(Path::new(FIXTURE)).unwrap_or_else(|error| panic!("{error}"))
}

fn card(name: &str, faces: &[&str]) -> PrintedCard {
    PrintedCard {
        oracle_id: String::new(),
        name: name.to_owned(),
        keywords: Vec::new(),
        layout: "normal".into(),
        faces: faces
            .iter()
            .map(|face| PrintedFace {
                name: (*face).to_owned(),
                ..PrintedFace::default()
            })
            .collect(),
    }
}

fn one(name: &str) -> PrintedCard {
    card(name, &[name])
}

#[rstest]
#[case::plain("Fenwick Scribe", "fenwick_scribe")]
#[case::apostrophe("Kael's Lantern", "kaels_lantern")]
#[case::typographic_apostrophe("Kael\u{2019}s Lantern", "kaels_lantern")]
#[case::comma("Mirabel, the Wanderer", "mirabel_the_wanderer")]
#[case::split("Spark // Frost", "spark_frost")]
#[case::hyphens("Will-o'-Wisp Vagrant", "willowisp_vagrant")]
#[case::accents("Élan Vital", "elan_vital")]
#[case::ligature("Æther Gust", "aether_gust")]
#[case::spaces("  Odd   Spaces ", "odd_spaces")]
#[case::digits("Rule of 3", "rule_of_3")]
fn the_file_name_is_the_lowercase_name_with_underscores_and_no_punctuation(
    #[case] name: &str,
    #[case] stem: &str,
) {
    assert_eq!(file_stem(name), stem);
}

const FENWICK: &str = "Name:Fenwick Scribe\nManaCost:1 U\nTypes:Creature Human Scribe\nPT:1/3\nK:Flying\n\
    T:Mode$ ChangesZone | Execute$ TrigInk\nSVar:TrigInk:DB$ Ink | Amount$ 2\nSVar:AIPreference:InkHoldBack\n\
    SVar:AILogic:Never\nSVar:PlayMain1:TRUE\nDeckHas:Ability$Ink\nA:AB$ Glow | Cost$ T\nS:Mode$ Continuous | AddKeyword$ Shimmer\n\
    R:Event$ Drain | ReplaceWith$ Spare\nFlavorName:The Long Road\nImage:fenwick.jpg\nText:Flying\nOracle:Flying\n";

#[test]
fn only_the_lines_that_say_what_the_card_does_are_kept() {
    let hint = hint_text(FENWICK).unwrap_or_default();
    assert_eq!(
        hint.lines().collect::<Vec<_>>(),
        [
            "K:Flying",
            "T:Mode$ ChangesZone | Execute$ TrigInk",
            "SVar:TrigInk:DB$ Ink | Amount$ 2",
            "A:AB$ Glow | Cost$ T",
            "S:Mode$ Continuous | AddKeyword$ Shimmer",
            "R:Event$ Drain | ReplaceWith$ Spare",
        ]
    );
}

#[rstest]
#[case::nothing_but_the_card("Name:Plain Ox\nManaCost:2\nTypes:Creature Ox\nPT:2/2\nOracle:\n")]
#[case::empty("")]
#[case::only_hints_for_the_computer("Name:X\nSVar:AIPreference:Y\nSVar:PlayMain1:TRUE\n")]
fn a_script_with_no_line_to_show_gives_no_hint(#[case] script: &str) {
    assert_eq!(hint_text(script), None);
}

#[test]
fn the_faces_of_a_card_with_several_are_headed_by_name_and_a_single_face_is_not() {
    let text = fs::read_to_string(Path::new(FIXTURE).join("h/hollow_larva.txt")).unwrap();
    let hint = hint_text(&text).unwrap_or_default();
    let lines: Vec<&str> = hint.lines().collect();
    assert_eq!(lines[0], "Face 1 of 2: Hollow Larva");
    assert!(lines[1].starts_with("K:Defender"), "{hint}");
    assert!(lines[2].starts_with("A:AB$ Molt"), "{hint}");
    assert_eq!(lines[3], "Face 2 of 2: Hollow Moth");
    assert!(lines[4].starts_with("K:Flying"), "{hint}");
    assert_eq!(lines.len(), 6, "{hint}");
    assert!(!hint_text(FENWICK).unwrap_or_default().contains("Face 1 of"));
}

/// A script of `count` ability lines of `length` characters each.
fn script_of(count: usize, length: usize) -> String {
    let line = format!("A:{}", "x".repeat(length - 2));
    format!("Name:Long\n{}\n", vec![line; count].join("\n"))
}

#[rstest]
#[case::fits(10, 50, 10, false)]
#[case::exactly_forty_lines(40, 20, 40, false)]
#[case::one_line_over(41, 20, 40, true)]
#[case::long_lines_end_at_the_characters(30, 99, 25, true)]
#[case::exactly_the_characters(25, 99, 25, false)]
#[case::one_character_over(25, 100, 24, true)]
fn a_hint_is_cut_at_a_line_boundary_at_the_limits_and_says_so(
    #[case] count: usize,
    #[case] length: usize,
    #[case] kept: usize,
    #[case] cut: bool,
) {
    let hint = hint_text(&script_of(count, length)).unwrap_or_default();
    let lines: Vec<&str> = hint.lines().collect();
    assert_eq!(lines.len(), kept + usize::from(cut), "{hint}");
    assert_eq!(lines.last() == Some(&TRUNCATED), cut, "{hint}");
    assert!(lines.iter().take(kept).all(|line| line.len() == length));
    let script_chars: usize = lines.iter().take(kept).map(|line| line.len() + 1).sum();
    assert!(script_chars <= MAX_CHARS, "{script_chars}");
}

#[test]
fn a_first_line_over_the_limit_leaves_no_hint() {
    assert_eq!(hint_text(&script_of(1, MAX_CHARS + 5)), None);
}

#[test]
fn the_limit_counts_all_faces_and_a_face_header_is_not_left_without_its_lines() {
    let first = vec!["A:aaaaaaaaaaaaaaaaaaaa"; 40].join("\n");
    let script = format!("Name:One\n{first}\nALTERNATE\nName:Two\nA:bbbb\n");
    let hint = hint_text(&script).unwrap_or_default();
    assert!(hint.ends_with(TRUNCATED), "{hint}");
    assert!(!hint.contains("Face 2"), "{hint}");
    assert_eq!(
        hint.lines().filter(|line| line.starts_with("A:")).count(),
        40
    );
}

#[rstest]
#[case::by_the_file_name("Fenwick Scribe", "K:Flying")]
#[case::apostrophe("Kael's Lantern", "A:AB$ Glow")]
#[case::typographic_apostrophe("Kael\u{2019}s Lantern", "A:AB$ Glow")]
#[case::comma("Mirabel, the Wanderer", "R:Event$ Drain")]
#[case::case_and_accents("FENWICK SCRIBE", "K:Flying")]
#[case::a_file_named_by_another_rule("Will-o'-Wisp Vagrant", "K:Hexproof")]
fn a_card_is_found_by_its_name(#[case] name: &str, #[case] line: &str) {
    let hint = hints().hint(&one(name)).unwrap_or_default();
    assert!(hint.contains(line), "{name}: {hint}");
}

#[test]
fn a_back_face_is_found_in_the_file_of_its_card_by_the_names_of_the_faces() {
    let hints = hints();
    // The index holds the name of the back face too, so a card named by it alone finds the file.
    assert!(
        hints
            .script("Hollow Moth")
            .is_some_and(|text| text.contains("Name:Hollow Larva"))
    );
    for card in [
        card(
            "Hollow Larva // Hollow Moth",
            &["Hollow Larva", "Hollow Moth"],
        ),
        card("Spark // Frost", &["Spark", "Frost"]),
    ] {
        let hint = hints.hint(&card).unwrap_or_default();
        assert!(hint.contains("Face 2 of 2"), "{}: {hint}", card.name);
    }
}

#[rstest]
#[case::no_script("Unwritten Wisp")]
#[case::no_folder("Quixotic Quokka")]
#[case::a_script_with_nothing_to_show("Plain Ox")]
#[case::an_empty_name("")]
fn a_card_with_no_script_has_no_hint(#[case] name: &str) {
    assert_eq!(hints().hint(&one(name)), None);
}

#[test]
fn a_folder_is_read_only_when_a_card_needs_its_index() {
    let hints = hints();
    let indexed = |hints: &ForgeHints| {
        let letters = hints.letters.lock().unwrap_or_else(PoisonError::into_inner);
        letters.keys().copied().collect::<String>()
    };
    assert_eq!(indexed(&hints), "");
    // Found by the file name: no index.
    assert!(hints.hint(&one("Fenwick Scribe")).is_some());
    assert_eq!(indexed(&hints), "");
    // Found by the index of its folder only; the folder of another letter stays unread.
    assert!(hints.hint(&one("Will-o'-Wisp Vagrant")).is_some());
    assert_eq!(indexed(&hints), "w");
    assert!(hints.hint(&one("Unwritten Wisp")).is_none());
    assert_eq!(indexed(&hints), "uw");
}

#[test]
fn the_first_script_of_a_name_two_files_share_is_the_one_used() {
    let dir = tempfile::tempdir().unwrap();
    let folder = dir.path().join("t");
    fs::create_dir_all(&folder).unwrap();
    fs::write(folder.join("twin_a.txt"), "Name:Twin\nK:Flying\n").unwrap();
    fs::write(folder.join("twin_b.txt"), "Name:Twin\nK:Haste\n").unwrap();
    fs::write(folder.join("notes.md"), "Name:Twin\nK:Trample\n").unwrap();
    let hints = ForgeHints::open(dir.path()).unwrap_or_else(|error| panic!("{error}"));
    assert_eq!(hints.hint(&one("Twin")).as_deref(), Some("K:Flying"));
}

#[test]
fn the_folder_may_be_the_checkout_that_holds_the_scripts() {
    let dir = tempfile::tempdir().unwrap();
    let folder = dir.path().join("forge-gui/res/cardsfolder/t");
    fs::create_dir_all(&folder).unwrap();
    fs::write(folder.join("tiny_owl.txt"), "Name:Tiny Owl\nK:Flying\n").unwrap();
    let hints = ForgeHints::open(dir.path()).unwrap_or_else(|error| panic!("{error}"));
    assert_eq!(hints.hint(&one("Tiny Owl")).as_deref(), Some("K:Flying"));
}

#[test]
fn a_folder_that_is_not_a_scripts_folder_is_an_error_before_any_card() {
    let dir = tempfile::tempdir().unwrap();
    let empty = ForgeHints::open(dir.path()).unwrap_err().to_string();
    assert!(empty.contains("no letter folders"), "{empty}");
    let missing = ForgeHints::open(&dir.path().join("nowhere"));
    assert!(missing.is_err());
}
