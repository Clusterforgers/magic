//! Goal: a place in an ability's effects marks its own clause of the writer's words, across spells, triggered and
//! activated abilities: the modes whole, a mode's bullet and an effect inside it, a "you may" with what depends on it,
//! the choice of a look, a search or a return, and a step a sentence joins to its neighbours; a place that is not there
//! marks nothing.
use rstest::rstest;

use super::{AskedPlace, asked_clause};
use crate::compile::{ClauseKind, render_ability};

const TUTOR: &str =
    "Search your library for a basic land card, put it onto the battlefield tapped, then shuffle.";
const CHARM: &str = "Choose one —\n• Target player draws two cards.\n• Destroy target artifact.\n• Return target creature card from your graveyard to your hand.";
const SEER_MODES: &str = "When ~ enters, choose one —\n• Draw a card.\n• Search your library for a basic land card, reveal it, put it into your hand, then shuffle.";
const LOOK: &str = "When ~ enters, look at the top four cards of your library. You may reveal a creature card from among them and put it into your hand. Put the rest on the bottom of your library in a random order.";

/// The words `place` marks in the writer's words of `text` read as `kind`, the card named Seer.
fn marked(text: &str, kind: ClauseKind, place: AskedPlace<'_>) -> Option<String> {
    let ability = crate::compile::read::clause(text, kind, false).expect("read");
    let words = render_ability(&ability)
        .expect("written")
        .replace('~', "Seer");
    let span = asked_clause(&ability, place, "Seer", &words)?;
    words.get(span).map(str::to_owned)
}

#[rstest]
#[case::a_search_and_the_steps_its_sentence_joins(TUTOR, ClauseKind::Spell, AskedPlace::Effect(&[0]), Some(TUTOR))]
#[case::the_modes_whole(CHARM, ClauseKind::Spell, AskedPlace::Modes(&[0]), Some(CHARM))]
#[case::the_second_mode(CHARM, ClauseKind::Spell, AskedPlace::Effect(&[0, 1, 0]), Some("Destroy target artifact."))]
#[case::the_third_mode(
    CHARM,
    ClauseKind::Spell,
    AskedPlace::Effect(&[0, 2, 0]),
    Some("Return target creature card from your graveyard to your hand.")
)]
#[case::the_modes_of_a_trigger(
    SEER_MODES,
    ClauseKind::Triggered,
    AskedPlace::Modes(&[0]),
    Some("choose one —\n• Draw a card.\n• Search your library for a basic land card, reveal it, put it into your hand, then shuffle.")
)]
#[case::a_search_inside_a_mode(
    SEER_MODES,
    ClauseKind::Triggered,
    AskedPlace::Effect(&[0, 1, 0]),
    Some("Search your library for a basic land card, reveal it, put it into your hand, then shuffle.")
)]
#[case::one_of_both(
    "Choose one or both —\n• Destroy target artifact.\n• Destroy target enchantment.",
    ClauseKind::Spell,
    AskedPlace::Effect(&[0, 1, 0]),
    Some("Destroy target enchantment.")
)]
#[case::a_payment_and_what_it_buys(
    "When ~ enters, you may pay {2}. If you do, draw a card.",
    ClauseKind::Triggered,
    AskedPlace::Effect(&[0]),
    Some("you may pay {2}. If you do, draw a card.")
)]
#[case::an_x_paid_and_its_reflexive_trigger(
    "When ~ enters, you may pay {X}. When you do, ~ deals X damage to any target.",
    ClauseKind::Triggered,
    AskedPlace::Effect(&[0]),
    Some("you may pay {X}. When you do, Seer deals X damage to any target.")
)]
#[case::a_sacrifice_asked(
    "You may sacrifice a creature. If you do, draw two cards.",
    ClauseKind::Spell,
    AskedPlace::Effect(&[0]),
    Some("You may sacrifice a creature. If you do, draw two cards.")
)]
#[case::a_choice_of_cards_to_discard(
    "Target opponent reveals their hand. You choose a nonland card from it. That player discards that card.",
    ClauseKind::Spell,
    AskedPlace::Effect(&[1]),
    Some("Target opponent reveals their hand. You choose a nonland card from it. That player discards that card.")
)]
#[case::a_return_after_a_mill(
    "Target player mills three cards. Return a creature card from your graveyard to your hand.",
    ClauseKind::Spell,
    AskedPlace::Effect(&[1]),
    Some("return a creature card from your graveyard to your hand.")
)]
#[case::a_choice_among_cards_looked_at(
    LOOK,
    ClauseKind::Triggered,
    AskedPlace::Effect(&[1]),
    Some("You may reveal a creature card from among them and put it into your hand. Put the rest on the bottom of your library in a random order.")
)]
#[case::a_scry("Scry 2, then draw a card.", ClauseKind::Spell, AskedPlace::Effect(&[0]), Some("Scry 2."))]
#[case::a_discard("Draw two cards, then discard a card.", ClauseKind::Spell, AskedPlace::Effect(&[1]), Some("Discard a card."))]
#[case::a_may_of_a_dies_trigger(
    "When ~ dies, you may return target creature card from your graveyard to your hand.",
    ClauseKind::Triggered,
    AskedPlace::Effect(&[0]),
    Some("you may return target creature card from your graveyard to your hand.")
)]
#[case::the_second_step_of_an_activation(
    "{2}, {T}: Draw a card, then discard a card.",
    ClauseKind::Activated,
    AskedPlace::Effect(&[1]),
    Some("Discard a card.")
)]
#[case::a_step_inside_a_may(
    "When ~ enters, you may sacrifice a creature. If you do, target player discards two cards.",
    ClauseKind::Triggered,
    AskedPlace::Effect(&[0, 0, 0]),
    Some("target player discards two cards.")
)]
#[case::no_such_effect(TUTOR, ClauseKind::Spell, AskedPlace::Effect(&[7]), None)]
#[case::no_such_mode(CHARM, ClauseKind::Spell, AskedPlace::Effect(&[0, 5, 0]), None)]
fn a_place_marks_its_own_clause(
    #[case] text: &str,
    #[case] kind: ClauseKind,
    #[case] place: AskedPlace<'_>,
    #[case] expected: Option<&str>,
) {
    assert_eq!(marked(text, kind, place).as_deref(), expected);
}

/// A colour chosen by a trigger, which only the written wordings word: "~ becomes the color of your choice."
const EYE: &str = r#"{"Triggered":{"when":{"ZoneChange":{"what":["This"],"to":"Battlefield"}},"effects":[{"Draw":{}},{"ChooseValue":{"kind":"Color","as":"chosen"}},{"Continuous":{"affected":"This","mods":[{"SetColorsChosen":"chosen"}],"duration":"Forever"}}]}}"#;

#[rstest]
#[case::the_choice_and_what_it_changes(&[1], Some("Seer becomes the color of your choice."))]
#[case::the_draw_before_it(&[0], Some("draw a card."))]
fn a_place_in_words_only_written_marks_its_clause(
    #[case] path: &[usize],
    #[case] expected: Option<&str>,
) {
    let ability: crate::ability::Ability = serde_json::from_str(EYE).expect("json");
    let words = super::rules_words(&ability)
        .expect("written")
        .replace('~', "Seer");
    assert_eq!(
        words,
        "When Seer enters, draw a card. Seer becomes the color of your choice."
    );
    let span = asked_clause(&ability, AskedPlace::Effect(path), "Seer", &words);
    assert_eq!(span.and_then(|span| words.get(span)), expected);
}
