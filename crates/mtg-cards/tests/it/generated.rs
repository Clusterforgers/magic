//! Goal: the rules the compiler generates for the corpus pass the checks of a rules file and join a library as stored rows would, a hand-written file wins over a row for the same oracle id, and a row that does not load (unparsable, invalid, or stored under another id) is left out and reported without stopping the others.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::collections::BTreeSet;

use crate::common;

use mtg_cards::Caps;
use mtg_cards::builtin;
use mtg_cards::generated::{Generation, generate};
use mtg_cards::load::{GeneratedRules, Library};
use mtg_cards::support::{PrintedCard, Support};
use serde_json::json;

use common::{printed, token_sources};

fn names(cards: &[PrintedCard]) -> BTreeSet<String> {
    cards
        .iter()
        .flat_map(|card| std::iter::once(&card.name).chain(card.faces.iter().map(|f| &f.name)))
        .map(|name| name.to_lowercase())
        .collect()
}

/// The predefined tokens as a library: what the compiler's output is checked against.
fn tokens() -> Library {
    Library::from_sources(&token_sources()).expect("the token files load")
}

fn card_named(name: &str) -> PrintedCard {
    printed()
        .into_iter()
        .find(|card| card.name == name)
        .unwrap_or_else(|| panic!("the corpus has {name}"))
}

fn row_of(card: &PrintedCard, hand_written: &Library, all: &BTreeSet<String>) -> GeneratedRules {
    let Generation::Rules(rules) = generate(card, hand_written, all) else {
        panic!("{} generates rules", card.name);
    };
    GeneratedRules {
        oracle_id: card.oracle_id.clone(),
        rules: serde_json::to_value(&*rules).unwrap(),
    }
}

#[test]
fn every_corpus_card_the_compiler_reads_generates_rules_that_load_as_rows() {
    let cards = printed();
    let (all, hand_written) = (names(&cards), tokens());
    let mut rows = Vec::new();
    for card in &cards {
        match generate(card, &hand_written, &all) {
            Generation::Rules(rules) => rows.push(GeneratedRules {
                oracle_id: card.oracle_id.clone(),
                rules: serde_json::to_value(&*rules).unwrap(),
            }),
            Generation::Nothing => {}
            Generation::Invalid(problems) => panic!("{}: {problems:?}", card.name),
        }
    }

    let (library, left_out) = hand_written.with_generated(&rows);

    assert!(left_out.is_empty(), "{left_out:?}");
    assert_eq!(library.cards().count(), rows.len());
    assert!(rows.len() > 30, "{} rows", rows.len());
}

#[test]
fn a_generated_card_is_played_by_its_rules() {
    let (bolt, all) = (card_named("Lightning Bolt"), names(&printed()));
    let hand_written = tokens();
    let row = row_of(&bolt, &hand_written, &all);

    let (library, left_out) = hand_written.with_generated(&[row]);

    assert!(left_out.is_empty(), "{left_out:?}");
    assert!(library.card(&bolt.oracle_id).is_some());
    assert_eq!(
        library.support(&bolt, &Caps::everything()).support,
        Support::Verified
    );
    assert_eq!(
        hand_written.support(&bolt, &Caps::everything()).support,
        Support::Manual,
        "the library it joined is left as it was"
    );
}

#[test]
fn a_hand_written_file_wins_over_a_row_for_the_same_card() {
    let (bolt, all) = (card_named("Lightning Bolt"), names(&printed()));
    let hand_written = builtin::library().expect("the embedded rules load");
    let mut row = row_of(&bolt, hand_written, &all);
    row.rules["faces"][0]["abilities"][0]["Spell"]["effects"][0]["DealDamage"]["amount"] =
        json!(99);

    let (library, left_out) = hand_written.with_generated(&[row]);

    assert!(left_out.is_empty(), "{left_out:?}");
    assert_eq!(
        library.card(&bolt.oracle_id),
        hand_written.card(&bolt.oracle_id)
    );
    assert_eq!(
        library.file_of(&bolt.oracle_id),
        hand_written.file_of(&bolt.oracle_id)
    );
}

#[test]
fn a_row_that_does_not_load_is_left_out_and_reported_and_the_others_load() {
    let (bolt, all) = (card_named("Lightning Bolt"), names(&printed()));
    let hand_written = tokens();
    let good = row_of(&bolt, &hand_written, &all);
    let unknown_field = GeneratedRules {
        oracle_id: "00000000-0000-0000-0000-000000000001".into(),
        rules: json!({"oracle_id": "00000000-0000-0000-0000-000000000001", "nope": 1}),
    };
    let mut out_of_range = row_of(&card_named("Brainstorm"), &hand_written, &all);
    out_of_range.rules["faces"][0]["abilities"][0]["Spell"]["effects"][0] =
        json!({"DealDamage": {"to": {"Target": 4}, "amount": 1}});
    let mut elsewhere = row_of(&card_named("Black Knight"), &hand_written, &all);
    elsewhere.oracle_id = "00000000-0000-0000-0000-000000000002".into();

    let (library, left_out) =
        hand_written.with_generated(&[unknown_field, out_of_range, elsewhere, good]);

    assert_eq!(library.cards().count(), 1);
    assert!(library.card(&bolt.oracle_id).is_some());
    let files: BTreeSet<&str> = left_out.iter().map(|e| e.file.as_str()).collect();
    assert_eq!(
        files,
        BTreeSet::from([
            "card_rules/00000000-0000-0000-0000-000000000001",
            "card_rules/00000000-0000-0000-0000-000000000002",
            format!("card_rules/{}", card_named("Brainstorm").oracle_id).as_str(),
        ])
    );
    assert!(
        left_out
            .iter()
            .any(|e| e.message.contains("unknown field `nope`")),
        "{left_out:?}"
    );
}

#[test]
fn rules_that_fail_a_check_against_the_imported_card_are_invalid() {
    let (bolt, all) = (card_named("Lightning Bolt"), names(&printed()));
    let mut changed = bolt.clone();
    // The compiler read the old text; the check finds the card as imported now says something else.
    changed.faces[0].oracle_text = "Lightning Bolt deals 4 damage to any target.".into();
    let outcome = generate(&changed, &tokens(), &all);
    // Compiled from the changed text, the rules still match it.
    assert!(matches!(outcome, Generation::Rules(_)), "{outcome:?}");

    let Generation::Rules(rules) = generate(&bolt, &tokens(), &all) else {
        panic!("rules");
    };
    let problems = tokens().check_generated(&rules, &changed, &all);
    assert!(
        problems
            .iter()
            .any(|p| p.message.contains("oracle text changed")),
        "{problems:?}"
    );
}

#[test]
fn a_compiled_mode_renders_as_its_bullet() {
    use mtg_cards::ability::Ability;
    use mtg_cards::compile::{compile, render_mode};
    use mtg_cards::effect::Effect;
    use mtg_cards::support::PrintedFace;

    let printed = PrintedCard {
        layout: String::new(),
        oracle_id: "00000000-0000-0000-0000-000000000003".into(),
        name: "Test Charm".into(),
        keywords: Vec::new(),
        faces: vec![PrintedFace {
            name: "Test Charm".into(),
            mana_cost: "{1}{U}".into(),
            type_line: "Instant".into(),
            oracle_text: "Choose one \u{2014}\n\u{2022} Draw a card.\n\u{2022} Target player gains 3 life and draws a card.\n\u{2022} Destroy target creature.".into(),
            ..PrintedFace::default()
        }],
    };
    let card = compile(&printed).card.expect("the charm compiles");
    let Some(Ability::Spell { effects, .. }) = card.faces[0].abilities.first() else {
        panic!("a spell ability, got {:?}", card.faces[0].abilities);
    };
    let [Effect::Modal { modes, .. }] = effects.as_slice() else {
        panic!("one modal effect, got {effects:?}");
    };

    let rendered: Vec<Option<String>> = modes.iter().map(render_mode).collect();

    assert_eq!(
        rendered,
        [
            Some("Draw a card".to_owned()),
            Some("Target player gains 3 life and draws a card".to_owned()),
            Some("Destroy target creature".to_owned()),
        ]
    );
}
