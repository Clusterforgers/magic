//! Goal: no text a card can show a player as the reason it is played by hand names the tooling or the engine (the compiler, a proposal, a primitive's dotted name, "the engine cannot ..."), for every kind of clause, every printed gap (each tells the player what to do by hand) and every card of the corpus.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common::{Corpus, printed_of, records};
use mtg_cards::ability::Ability;
use mtg_cards::assemble::assemble;
use mtg_cards::compile::compile;
use mtg_cards::support::{Caps, PrintedCard, PrintedFace};
use rstest::rstest;
use serde_json::Value;

/// Words of the tooling a player has no use for.
const TOOLING: [&str; 9] = [
    "engine",
    "compiler",
    "parser",
    "proposal",
    "proposed",
    "primitive",
    "capabilit",
    " ast ",
    "dsl",
];

/// Why `text` is no wording for a player, or `None` when it is fine.
fn tooling_in(text: &str) -> Option<String> {
    let lower = format!(" {} ", text.to_lowercase());
    if let Some(word) = TOOLING.iter().find(|word| lower.contains(**word)) {
        return Some(format!("names the tooling ({word:?})"));
    }
    let chars: Vec<char> = text.chars().collect();
    let dotted = chars
        .windows(3)
        .any(|w| w[0].is_alphanumeric() && w[1] == '.' && w[2].is_uppercase());
    dotted.then(|| "holds a dotted name of the engine".to_owned())
}

/// Every `reason` string anywhere in a JSON value.
fn reasons_in(value: &Value, found: &mut Vec<String>) {
    match value {
        Value::Object(map) => {
            for (key, inner) in map {
                match inner {
                    Value::String(text) if key == "reason" => found.push(text.clone()),
                    _ => reasons_in(inner, found),
                }
            }
        }
        Value::Array(items) => items.iter().for_each(|item| reasons_in(item, found)),
        _ => {}
    }
}

#[rstest]
#[case::the_compiler("a triggered ability the compiler does not read yet", true)]
#[case::a_primitive("the engine does not run Effect.DealDamage yet", true)]
#[case::a_proposal("the proposed rules do not cover this clause", true)]
#[case::the_engine_cannot_pay("the engine cannot pay {X} in a mana cost yet", true)]
#[case::the_engine_does_not_know(
    "the mana cost has the symbol {S}, which the engine does not know yet",
    true
)]
#[case::the_engine_plays_a_face("the engine plays only the front face of a card", true)]
#[case::the_player_pays("you choose and pay the {X} in the mana cost by hand", false)]
#[case::the_game_does_not_run(
    "you work out the printed power by hand: it is not a number, so it comes from an ability the game does not run yet",
    false
)]
#[case::plain_words("you apply this triggered ability by hand", false)]
#[case::a_gap_in_words(
    "you apply this by hand; the game does not run yet: deal damage",
    false
)]
#[case::a_sentence_with_a_stop("you play this card's text by hand. Then go on.", false)]
fn the_scan_knows_tooling_from_plain_words(#[case] text: &str, #[case] flagged: bool) {
    assert_eq!(tooling_in(text).is_some(), flagged, "{text}");
}

/// The reasons the compiler gives, the state of each corpus card under an engine that runs nothing and under one that
/// runs everything, and the gaps of the printed characteristics: every text the by-hand list and the deck preview show.
fn reasons_of_the_corpus() -> Vec<(String, String)> {
    let mut found = Vec::new();
    for record in records() {
        let printed = printed_of(&record);
        let compiled = compile(&printed);
        if let Some(card) = &compiled.card {
            for (_, ability) in card.abilities() {
                if let Ability::Manual { reason, .. } = ability {
                    found.push((record.name.clone(), reason.clone()));
                }
            }
        }
        for caps in [Caps::none(), Caps::everything()] {
            let assembled = assemble(&printed, compiled.card.as_ref(), &caps);
            for clause in &assembled.support.clauses {
                found.push((record.name.clone(), clause.reason.clone()));
            }
        }
    }
    found
}

#[test]
fn the_reasons_of_every_corpus_card_are_plain_words() {
    let found = reasons_of_the_corpus();
    assert!(
        found.len() > 100,
        "the corpus gives {} reasons",
        found.len()
    );
    let bad: Vec<String> = found
        .iter()
        .filter_map(|(card, reason)| {
            tooling_in(reason).map(|why| format!("{card}: {reason:?} {why}"))
        })
        .collect();
    assert!(bad.is_empty(), "{}", bad.join("\n"));
}

#[test]
fn the_reasons_written_into_card_files_are_plain_words() {
    let mut found = Vec::new();
    for path in Corpus::repo().card_files() {
        let value: Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        reasons_in(&value, &mut found);
    }
    let bad: Vec<String> = found
        .iter()
        .filter_map(|reason| tooling_in(reason).map(|why| format!("{reason:?} {why}")))
        .collect();
    assert!(bad.is_empty(), "{}", bad.join("\n"));
}

fn face(name: &str, cost: &str, type_line: &str) -> PrintedFace {
    PrintedFace {
        name: name.to_owned(),
        mana_cost: cost.to_owned(),
        type_line: type_line.to_owned(),
        oracle_text: String::new(),
        power: None,
        toughness: None,
        loyalty: None,
        defense: None,
        colors: Vec::new(),
    }
}

fn stats(power: &str, toughness: &str) -> PrintedFace {
    PrintedFace {
        power: Some(power.to_owned()),
        toughness: Some(toughness.to_owned()),
        ..face("Star", "{1}{G}", "Creature \u{2014} Elf")
    }
}

fn printed(layout: &str, faces: Vec<PrintedFace>) -> PrintedCard {
    PrintedCard {
        oracle_id: "00000000-0000-0000-0000-000000000000".to_owned(),
        name: "Test Card".to_owned(),
        keywords: Vec::new(),
        layout: layout.to_owned(),
        faces,
    }
}

#[rstest]
#[case::x_in_the_cost(
    printed("normal", vec![face("Fireball", "{X}{R}", "Sorcery")]),
    "you choose and pay the {X} in the mana cost by hand"
)]
#[case::a_symbol_the_game_lacks(
    printed("normal", vec![face("Chill", "{1}{S}", "Instant")]),
    "you pay the {S} in the mana cost by hand: the game does not know the symbol"
)]
#[case::a_cost_nobody_can_read(
    printed("normal", vec![face("Garbled", "1G", "Instant")]),
    "you pay the mana cost \"1G\" by hand: the game cannot read it"
)]
#[case::a_type_word_the_game_lacks(
    printed("normal", vec![face("Plane", "", "Plane \u{2014} Dominaria")]),
    "you apply the type line by hand: the game does not know Plane"
)]
#[case::a_star_power(
    printed("normal", vec![stats("*", "2")]),
    "you work out the printed power by hand: it is not a number, so it comes from an ability the game does not run yet"
)]
#[case::star_stats(
    printed("normal", vec![stats("*", "1+*")]),
    "you work out the printed power and toughness by hand: they are not numbers, so they come from an ability the game does not run yet"
)]
#[case::a_face_the_game_does_not_play(
    printed(
        "transform",
        vec![
            face("Front", "{1}{G}", "Creature \u{2014} Elf"),
            face("Back", "", "Creature \u{2014} Werewolf"),
        ]
    ),
    "you cast or turn over Back by hand: the game plays only the front face of a card"
)]
fn a_printed_gap_tells_the_player_what_to_do_by_hand(
    #[case] card: PrintedCard,
    #[case] reason: &str,
) {
    let assembled = assemble(&card, None, &Caps::none());
    let reasons: Vec<&str> = assembled
        .support
        .clauses
        .iter()
        .map(|clause| clause.reason.as_str())
        .collect();
    assert!(reasons.contains(&reason), "{reasons:?}");
    for text in reasons {
        assert_eq!(tooling_in(text), None, "{text}");
    }
}
