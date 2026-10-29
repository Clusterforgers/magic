//! Goal: the reflexive "When you do," reads into the steps that exist (an `If` of `Performed` after a sacrifice, the next steps after a surveil or a scry, the `paid` steps of a payment that choose a target), a counter and a fight read as one sentence, each reads back as the printed text, and a link with no step to depend on, a follow-up that is no sentence or a fight of another creature stays with the table.
use super::*;

/// The sentences of the reflexive family, each written back as the card prints it.
#[rstest]
#[case::sacrifice_then_search(
    "When ~ enters, sacrifice it. When you do, search your library for a basic Mountain, Forest, or Plains card, put it onto the battlefield tapped, then shuffle and you gain 1 life.",
    r#""If":{"condition":"Performed""#
)]
#[case::sacrifice_if_you_do(
    "When ~ enters, sacrifice it. If you do, search your library for a basic Mountain, Forest, or Plains card, put it onto the battlefield tapped, then shuffle and you gain 1 life.",
    r#""If":{"condition":"Performed""#
)]
#[case::sacrifice_then_draw(
    "Whenever ~ attacks, sacrifice it. When you do, draw a card.",
    r#""If":{"condition":"Performed""#
)]
#[case::surveil_then_return(
    "When ~ enters, surveil 4. When you do, you may return target nonland permanent an opponent controls to its owner's hand.",
    r#""Surveil":{"count":4}"#
)]
#[case::surveil_then_draw(
    "When ~ enters, surveil 2. When you do, draw a card.",
    r#""Surveil":{"count":2}"#
)]
#[case::scry_then_damage(
    "When ~ enters, scry 2. When you do, ~ deals 3 damage to any target.",
    r#""Scry":{"count":2}"#
)]
#[case::payment_then_damage(
    "Whenever ~ attacks, you may pay {2}. When you do, ~ deals 3 damage to any target.",
    r#""Pay":{"cost":"{2}""#
)]
#[case::sacrifice_cost_then_damage(
    "Whenever ~ attacks, you may sacrifice an artifact. When you do, ~ deals 3 damage to any target.",
    r#""Sacrifice":{"filter":[{"Type":"Artifact"}]}"#
)]
#[case::counter_then_fight_after_a_cost(
    "Whenever ~ attacks, you may sacrifice an artifact. When you do, put a +1/+1 counter on ~, then it fights up to one target creature defending player controls.",
    r#""Fight":{"what":"This","with":{"Target":0}}"#
)]
#[case::counter_then_fight(
    "Whenever ~ attacks, put a +1/+1 counter on ~, then it fights up to one target creature defending player controls.",
    r#""Fight":{"what":"This","with":{"Target":0}}"#
)]
fn w20b_reflexive_sentences_read_and_write_back(#[case] text: &str, #[case] part: &str) {
    let read = golden(text, ClauseKind::Triggered, false);
    let json = serde_json::to_string(&read).unwrap();
    assert!(json.contains(part), "{text}: {json}");
}

/// The payment with a reflexive target still reads with "If you do", and the link keeps its steps in order.
#[rstest]
#[case::if_you_do_after_a_payment(
    "Whenever ~ attacks, you may pay {2}. If you do, ~ deals 3 damage to any target."
)]
#[case::if_you_do_without_a_target(
    "Whenever ~ attacks, you may sacrifice an artifact. If you do, draw a card."
)]
fn w20b_existing_links_still_read(#[case] text: &str) {
    golden(text, ClauseKind::Triggered, false);
}

/// A link with nothing to depend on, a follow-up with no reading, or a counter and a fight of another creature.
#[rstest]
#[case::no_target_after_a_cost(
    "Whenever ~ attacks, you may sacrifice an artifact. When you do, draw a card."
)]
#[case::no_target_after_a_payment("Whenever ~ attacks, you may pay {2}. When you do, draw a card.")]
#[case::after_a_token("When ~ enters, create a Treasure token. When you do, draw a card.")]
#[case::after_a_draw("When ~ enters, draw a card. When you do, draw a card.")]
#[case::if_you_do_after_a_surveil("When ~ enters, surveil 2. If you do, draw a card.")]
#[case::follow_up_unknown("When ~ enters, sacrifice it. When you do, frobnicate your library.")]
#[case::nothing_before_the_link("Whenever ~ attacks, when you do, draw a card.")]
#[case::counter_on_another(
    "Whenever ~ attacks, put a +1/+1 counter on target creature you control, then it fights up to one target creature defending player controls."
)]
#[case::sacrifice_then_when_without_comma("When ~ enters, sacrifice it. When you do draw a card.")]
fn w20b_other_links_stay_with_the_table(#[case] text: &str) {
    assert_eq!(clause(text, ClauseKind::Triggered, false), None, "{text}");
}

/// An ability word with a digit after its hyphen is flavor like any other; a digit anywhere else is not a word.
#[rstest]
#[case::digit_after_a_hyphen("Nitro-9 \u{2014} Whenever ~ attacks, draw a card.", Some(12))]
#[case::letters_only("Landfall \u{2014} Whenever ~ attacks, draw a card.", Some(13))]
#[case::digit_in_the_word("Nitro9 \u{2014} Whenever ~ attacks, draw a card.", None)]
#[case::digit_first("9 \u{2014} Whenever ~ attacks, draw a card.", None)]
fn w20b_ability_words_may_hold_a_digit_after_a_hyphen(
    #[case] line: &str,
    #[case] length: Option<usize>,
) {
    assert_eq!(
        crate::compile::classify::ability_word_len(line),
        length,
        "{line}"
    );
}
