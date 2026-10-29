//! Goal: the pairs are the `Verified` non-token cards in the proposal format with a stable split, the retrieval masks
//! what is not mechanics, ranks by shared wording, keywords and types, never returns the card itself or a held-out card
//! when asked not to, breaks ties by name, and the prompt shows the retrieved cards in a fixed layout.
use std::collections::{BTreeMap, BTreeSet};

use mtg_cards::compile::compile;
use mtg_cards::load::Library;
use mtg_cards::{PrintedCard, PrintedFace};
use rstest::rstest;

use super::measure::w19b_measure;
use super::retrieve::{W19bIndex, W19bQuery, w19b_mask, w19b_terms};
use super::*;
use crate::propose::prompt::card_prompt;
use crate::propose::w19b_solved::W19bSolvedExamples;

fn printed(name: &str, type_line: &str, text: &str, keywords: &[&str]) -> PrintedCard {
    PrintedCard {
        oracle_id: format!("id-{name}"),
        name: name.to_owned(),
        keywords: keywords.iter().map(|k| (*k).to_owned()).collect(),
        layout: "normal".into(),
        faces: vec![PrintedFace {
            name: name.to_owned(),
            type_line: type_line.to_owned(),
            oracle_text: text.to_owned(),
            ..PrintedFace::default()
        }],
    }
}

fn pair(card: &PrintedCard, split: W19bSplit) -> W19bPair {
    W19bPair {
        name: card.name.clone(),
        oracle_text: card.faces[0].oracle_text.clone(),
        type_line: card.faces[0].type_line.clone(),
        mana_cost: String::new(),
        rules: serde_json::Value::Null,
        source: W19bSource::Corpus,
        split,
        oracle_id: card.oracle_id.clone(),
        names: vec![card.name.clone()],
        keywords: card.keywords.clone(),
        rules_text: format!("{{\"card\":\"{}\"}}", card.name),
        vocabulary: BTreeSet::new(),
    }
}

fn names(
    index: &W19bIndex,
    pairs: &[W19bPair],
    card: &PrintedCard,
    k: usize,
    train: bool,
) -> Vec<String> {
    index
        .nearest(&W19bQuery::of_card(card), k, train)
        .into_iter()
        .map(|at| pairs[at].name.clone())
        .collect()
}

#[rstest]
#[case::reminder_text("Flying (This creature can't be blocked.)", &[], "flying ")]
#[case::own_name("When Serra Angel enters, draw a card.", &["Serra Angel"], "when ~ enters, draw a card.")]
#[case::legendary_short_name("Whenever Roon attacks, exile it.", &["Roon, Wanderer"], "whenever ~ attacks, exile it.")]
#[case::mana_symbols("{2}{W}, {T}: Untap {Q}.", &[], " {m}  {m} ,  {t} : untap  {q} .")]
#[case::short_names_stay("Ox gets +1/+1.", &["Ox"], "ox gets +1/+1.")]
fn the_mask_drops_reminder_text_names_and_mana(
    #[case] text: &str,
    #[case] own: &[&str],
    #[case] expected: &str,
) {
    let own: Vec<String> = own.iter().map(|name| (*name).to_owned()).collect();
    assert_eq!(w19b_mask(text, &own), expected);
}

#[rstest]
#[case::digits("deals 3 damage", &["deals", "#", "damage", "deals #", "# damage"])]
#[case::number_words("draw two cards", &["draw", "#", "cards", "draw #", "# cards"])]
#[case::counters("put a +1/+1 counter", &["put", "a", "+#/+#", "counter", "put a", "a +#/+#", "+#/+# counter"])]
fn numbers_are_masked_in_the_terms(#[case] masked: &str, #[case] expected: &[&str]) {
    let wanted: BTreeSet<String> = expected.iter().map(|term| (*term).to_owned()).collect();
    assert_eq!(w19b_terms(masked), wanted);
}

#[test]
fn the_same_text_with_other_numbers_and_names_is_the_same_query() {
    let a = W19bQuery::of_card(&printed(
        "Shock",
        "Instant",
        "Shock deals 2 damage to any target.",
        &[],
    ));
    let b = W19bQuery::of_card(&printed(
        "Jolt",
        "Instant",
        "Jolt deals 4 damage to any target. (Reminder.)",
        &[],
    ));
    assert_eq!(a.terms, b.terms);
}

/// The cards of a ranking table: a card asked about and the solved cards.
fn table() -> (PrintedCard, Vec<W19bPair>) {
    let wanted = printed(
        "Wanted",
        "Creature — Bird",
        "Flying\nWhenever this creature attacks, draw a card.",
        &["Flying"],
    );
    let cards = [
        (
            printed("Far", "Sorcery", "Destroy target land.", &[]),
            W19bSplit::Train,
        ),
        (
            printed(
                "Near",
                "Creature — Drake",
                "Flying\nWhenever this creature attacks, you gain 1 life.",
                &["Flying"],
            ),
            W19bSplit::Train,
        ),
        (
            printed(
                "Held",
                "Creature — Bird",
                "Flying\nWhenever this creature attacks, draw a card.",
                &["Flying"],
            ),
            W19bSplit::HeldOut,
        ),
        (
            printed(
                "Wanted",
                "Creature — Bird",
                "Flying\nWhenever this creature attacks, draw a card.",
                &["Flying"],
            ),
            W19bSplit::Train,
        ),
        (
            printed("Bravo", "Instant", "Draw a card.", &[]),
            W19bSplit::Train,
        ),
        (
            printed("Alpha", "Instant", "Draw a card.", &[]),
            W19bSplit::Train,
        ),
    ];
    (
        wanted,
        cards
            .iter()
            .map(|(card, split)| pair(card, *split))
            .collect(),
    )
}

#[rstest]
#[case::all(5, false, &["Held", "Near", "Alpha", "Bravo", "Far"])]
#[case::train_only(5, true, &["Near", "Alpha", "Bravo", "Far"])]
#[case::top_two(2, true, &["Near", "Alpha"])]
fn the_nearest_skip_the_card_itself_and_held_out_cards_and_ties_go_by_name(
    #[case] k: usize,
    #[case] train_only: bool,
    #[case] expected: &[&str],
) {
    let (wanted, pairs) = table();
    let index = W19bIndex::build(&pairs);
    assert_eq!(names(&index, &pairs, &wanted, k, train_only), expected);
}

#[test]
fn the_card_is_excluded_by_its_oracle_id_even_under_another_name() {
    let (wanted, mut pairs) = table();
    pairs[3].name = "Renamed".into();
    let index = W19bIndex::build(&pairs);
    assert!(!names(&index, &pairs, &wanted, 9, false).contains(&"Renamed".to_owned()));
}

#[test]
fn cards_with_the_same_rules_are_one_example() {
    let (wanted, mut pairs) = table();
    pairs[4].rules_text = pairs[5].rules_text.clone();
    let index = W19bIndex::build(&pairs);
    assert_eq!(
        names(&index, &pairs, &wanted, 9, true),
        ["Near", "Alpha", "Far"]
    );
}

#[test]
fn the_split_is_stable_and_holds_out_about_the_percent_asked() {
    let ids: Vec<String> = (0..4000).map(|n| format!("oracle-{n}")).collect();
    let held = |splitter: W19bSplitter| {
        ids.iter()
            .filter(|id| splitter.split(id) == W19bSplit::HeldOut)
            .count()
    };
    let default = W19bSplitter::default();
    assert!((140..=260).contains(&held(default)), "{}", held(default));
    assert_eq!(
        held(W19bSplitter {
            percent: 0,
            seed: 0
        }),
        0
    );
    assert_eq!(
        held(W19bSplitter {
            percent: 100,
            seed: 0
        }),
        ids.len()
    );
    // The same id lands in the same part on every run; another seed holds out other cards.
    assert_eq!(default.split("oracle-17"), default.split("oracle-17"));
    let other = W19bSplitter {
        percent: 5,
        seed: 9,
    };
    assert!(ids.iter().any(|id| default.split(id) != other.split(id)));
    assert_eq!(
        default.split("ab0c1d2e-0000-4000-8000-000000000000"),
        W19bSplit::HeldOut
    );
}

/// Cards the compiler reads whole, with their compiled rules.
fn compiled(cards: &[PrintedCard]) -> BTreeMap<String, mtg_cards::card::Card> {
    cards
        .iter()
        .filter_map(|card| Some((card.oracle_id.clone(), compile(card).card?)))
        .collect()
}

#[test]
fn the_pairs_are_the_verified_non_token_cards_as_json_lines() {
    let draw = printed("Divination", "Sorcery", "Draw two cards.", &[]);
    let mut token = printed("Drawing Token", "Token Sorcery", "Draw two cards.", &[]);
    token.layout = "token".into();
    let mut stale = printed("Stale", "Sorcery", "Draw two cards.", &[]);
    let rules = compiled(&[draw.clone(), token.clone(), stale.clone()]);
    stale.faces[0].oracle_text = "Draw three cards.".into();
    let pairs = w19b_solved_pairs(
        &[stale, token, draw],
        &Library::default(),
        &rules,
        &mtg_engine::caps(),
        W19bSplitter::default(),
    );
    let text = w19b_jsonl(&pairs).unwrap();
    let lines: Vec<serde_json::Value> = text
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect();
    assert_eq!(lines.len(), 1, "{text}");
    let line = &lines[0];
    let keys: BTreeSet<&str> = line
        .as_object()
        .unwrap()
        .keys()
        .map(String::as_str)
        .collect();
    assert_eq!(
        keys,
        BTreeSet::from([
            "name",
            "oracle_text",
            "type_line",
            "mana_cost",
            "rules",
            "source",
            "split"
        ])
    );
    assert_eq!(line["name"], "Divination");
    assert_eq!(line["oracle_text"], "Draw two cards.");
    assert_eq!(line["source"], "compiled");
    assert_eq!(line["split"], "train");
    assert_eq!(
        line["rules"]["faces"][0]["abilities"][0]["Spell"]["effects"][0]["Draw"]["count"],
        2
    );
    assert_eq!(
        w19b_summary(&pairs),
        "1 pairs: corpus 0 train + 0 held out; compiled 1 train + 0 held out"
    );
}

#[test]
fn the_prompt_shows_the_nearest_solved_cards_after_the_card() {
    let (wanted, pairs) = table();
    let solved = W19bSolvedExamples::new(&pairs, 2, true);
    let text = card_prompt(&wanted, &solved.nearest(&wanted));
    let section = text
        .split_once("\nSolved examples")
        .map(|(_, rest)| rest)
        .unwrap();
    assert_eq!(
        section,
        " with close wording (oracle text, then the rules that are accepted for it):\n\
         \nExample: Near (Creature — Drake)\nOracle text:\nFlying\nWhenever this creature attacks, you gain 1 life.\nRules: {\"card\":\"Near\"}\n\
         \nExample: Alpha (Instant)\nOracle text:\nDraw a card.\nRules: {\"card\":\"Alpha\"}\n\
         \nWrite the rules of Wanted now. Reply with the JSON object only."
    );
}

#[test]
fn the_measure_counts_the_kinds_the_nearest_examples_cover() {
    let (_, mut pairs) = table();
    for pair in &mut pairs {
        pair.vocabulary =
            BTreeSet::from([format!("Effect.{}", pair.name), "Keyword.Flying".to_owned()]);
    }
    pairs[2].vocabulary = BTreeSet::from(["Effect.Wanted".to_owned()]);
    let measure = w19b_measure(&pairs, &[1]);
    assert_eq!((measure.cards, measure.train), (1, 5));
    assert!((measure.rows[0].1.full - 1.0).abs() < 1e-9, "{measure:?}");
    assert!(measure.render().contains("K=1: nearest full 100.0%"));
}

fn parse(line: &[&str]) -> Option<crate::args::Command> {
    use clap::Parser as _;
    let line = std::iter::once("mtg-cli").chain(line.iter().copied());
    crate::args::Cli::try_parse_from(line)
        .ok()
        .map(|cli| cli.command)
}

#[rstest]
#[case::default(&["propose", "--cards", "Opt"], Some((0, false)))]
#[case::solved(&["propose", "--cards", "Opt", "--solved-examples", "5", "--examples-train-only"], Some((5, true)))]
#[case::train_only_alone(&["propose", "--cards", "Opt", "--examples-train-only"], None)]
fn the_solved_example_flags_parse(#[case] line: &[&str], #[case] expected: Option<(usize, bool)>) {
    let got = parse(line).and_then(|command| match command {
        crate::args::Command::Propose(args) => {
            Some((args.solved_examples, args.examples_train_only))
        }
        _ => None,
    });
    assert_eq!(got, expected);
}

#[test]
fn export_pairs_defaults_to_five_percent_held_out() {
    let Some(crate::args::Command::ExportPairs(args)) =
        parse(&["export-pairs", "--out", "p.jsonl"])
    else {
        panic!("expected export-pairs");
    };
    assert_eq!(
        (args.held_out_percent, args.seed, args.measure),
        (5, 0, false)
    );
    assert!(parse(&["export-pairs", "--out", "p", "--held-out-percent", "101"]).is_none());
}
