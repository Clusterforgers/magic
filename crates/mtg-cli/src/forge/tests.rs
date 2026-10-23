//! Goal: the survey finds the script of each stored card by the name of the card or of a face, whatever the case, accents and quotes, sets the items of the script against the rules of the card, marks a card with an unread clause, and names the cards Forge has no script for.
use std::collections::BTreeMap;

use mtg_cards::PrintedFace;
use rstest::rstest;

use super::*;
use compare::Status;

fn printed(n: u32, name: &str, type_line: &str, text: &str) -> PrintedCard {
    PrintedCard {
        layout: String::new(),
        oracle_id: format!("00000000-0000-4000-8000-{n:012}"),
        name: name.into(),
        keywords: vec![],
        faces: vec![PrintedFace {
            name: name.into(),
            type_line: type_line.into(),
            oracle_text: text.into(),
            ..PrintedFace::default()
        }],
    }
}

fn in_deck(printed: PrintedCard) -> PreconCard {
    PreconCard {
        printed,
        copies: BTreeMap::from([("deck".to_owned(), 1)]),
    }
}

fn run(cards: &[PreconCard], scripts: &[&str]) -> Survey {
    let scripts: Vec<Script> = scripts.iter().map(|text| Script::parse(text)).collect();
    let map = Map::standard().unwrap_or_else(|error| panic!("{error}"));
    survey(
        cards,
        &scripts,
        map,
        &Library::default(),
        &Caps::everything(),
    )
}

fn check<'a>(survey: &'a Survey, name: &str) -> &'a CardCheck {
    survey
        .checks
        .iter()
        .find(|check| check.name == name)
        .unwrap_or_else(|| panic!("no check for {name}"))
}

fn statuses(check: &CardCheck) -> Vec<(&str, Status)> {
    check
        .findings
        .iter()
        .map(|finding| (finding.name.as_str(), finding.status))
        .collect()
}

#[test]
fn a_card_the_compiler_reads_is_read_and_a_card_with_an_unread_clause_is_in_the_tail() {
    let cards = [
        in_deck(printed(
            1,
            "Quill Sprite",
            "Creature \u{2014} Faerie",
            "Flying",
        )),
        in_deck(printed(2, "Brisk Study", "Instant", "Draw a card.")),
        in_deck(printed(
            3,
            "Odd Rite",
            "Sorcery",
            "Rotate the widget until the gizmo glows.",
        )),
        in_deck(printed(4, "Lost Card", "Instant", "Draw a card.")),
    ];
    let survey = run(
        &cards,
        &[
            "Name:Quill Sprite\nK:Flying\n",
            "Name:Brisk Study\nA:SP$ Draw | NumCards$ 1\n",
            "Name:Odd Rite\nA:SP$ Spin | Defined$ You | SubAbility$ DBGlow\nSVar:DBGlow:DB$ Draw | NumCards$ 2\n",
        ],
    );

    assert_eq!(survey.stored, 4);
    assert_eq!(survey.absent, ["Lost Card"]);
    let names: Vec<&str> = survey.checks.iter().map(|c| c.name.as_str()).collect();
    assert_eq!(names, ["Brisk Study", "Odd Rite", "Quill Sprite"]);

    let sprite = check(&survey, "Quill Sprite");
    assert!(!sprite.unread);
    assert_eq!(statuses(sprite), [("Flying", Status::Read)]);
    assert_eq!(sprite.ours, ["keyword.flying"]);

    let study = check(&survey, "Brisk Study");
    assert!(!study.unread);
    assert_eq!(statuses(study), [("Draw", Status::Read)]);

    let rite = check(&survey, "Odd Rite");
    assert!(rite.unread);
    assert_eq!(
        statuses(rite),
        [("Spin", Status::Unmapped), ("Draw", Status::Missing)]
    );
    assert_eq!(rite.ours.len(), 0);
}

#[test]
fn a_hand_written_file_is_read_before_the_compiler() {
    let card = printed(7, "Brisk Study", "Instant", "Draw a card.");
    let mut file = mtg_cards::compile::compile(&card)
        .card
        .unwrap_or_else(|| panic!("compiles"));
    file.faces[0].abilities.clear();
    file.impl_status = mtg_cards::card::ImplStatus::Manual;
    let library = Library::from_sources(&[mtg_cards::load::Source {
        path: "cards/brisk_study.json".to_owned(),
        text: mtg_cards::load::canonical(&file),
    }])
    .unwrap_or_else(|errors| panic!("{errors}"));
    let scripts = [Script::parse(
        "Name:Brisk Study\nA:SP$ Draw | NumCards$ 1\n",
    )];
    let map = Map::standard().unwrap_or_else(|error| panic!("{error}"));

    let survey = survey(
        &[in_deck(card)],
        &scripts,
        map,
        &library,
        &Caps::everything(),
    );

    // The file models nothing, so the Draw of the script is not read, though the compiler would read it.
    assert_eq!(statuses(&survey.checks[0]), [("Draw", Status::Missing)]);
}

fn index_of(scripts: &[&str]) -> BTreeMap<String, usize> {
    let scripts: Vec<Script> = scripts.iter().map(|text| Script::parse(text)).collect();
    script_index(&scripts)
}

fn named(name: &str, faces: &[&str]) -> PrintedCard {
    let mut card = printed(1, name, "Instant", "");
    card.faces = faces
        .iter()
        .map(|face| PrintedFace {
            name: (*face).to_owned(),
            ..PrintedFace::default()
        })
        .collect();
    card
}

#[rstest]
#[case::the_name("Zap Bolt", &["Zap Bolt"], Some(0))]
#[case::another_case("zap BOLT", &["Zap Bolt"], Some(0))]
#[case::a_typographic_quote("Cat\u{2019}s Paw", &["Cat's Paw"], Some(0))]
#[case::an_accent("L\u{00fb}minous Dream", &["Luminous Dream"], Some(0))]
#[case::no_script("Other Card", &["Zap Bolt"], None)]
fn a_card_is_found_by_its_name_whatever_the_case_accents_and_quotes(
    #[case] name: &str,
    #[case] scripted: &[&str],
    #[case] expected: Option<usize>,
) {
    let texts: Vec<String> = scripted.iter().map(|n| format!("Name:{n}\n")).collect();
    let texts: Vec<&str> = texts.iter().map(String::as_str).collect();
    let found = script_of(&named(name, &[name]), &index_of(&texts));
    assert_eq!(found, expected);
}

#[test]
fn a_card_with_faces_is_found_by_the_name_of_either_face_or_by_all_of_them() {
    let both = "Name:Front Half\nALTERNATE\nName:Back Half\n";
    let other = "Name:Other\n";
    let index = index_of(&[other, both]);
    for card in [
        named("Front Half // Back Half", &["Front Half", "Back Half"]),
        named("Unrelated", &["Back Half"]),
        named("Front Half", &["Front Half"]),
    ] {
        assert_eq!(script_of(&card, &index), Some(1), "{}", card.name);
    }
    assert_eq!(
        script_of(&named("Front Half // Back Half", &[]), &index_of(&[both])),
        Some(0)
    );
}

#[test]
fn the_first_script_of_a_name_wins() {
    let index = index_of(&["Name:Twin\nK:Flying\n", "Name:Twin\nK:Reach\n"]);
    assert_eq!(script_of(&named("Twin", &["Twin"]), &index), Some(0));
}
