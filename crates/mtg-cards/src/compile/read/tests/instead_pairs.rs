//! Goal: an effect that another replaces under a condition reads and writes back as the card words it (the first sentence, then "if C, instead S2" or "if C, S2 instead"), a search puts one card of the cards found in one place and the rest in another, and a list of lands names the basic lands and/or other land types; a replacement that names a target, lacks its second sentence or states no condition stays with the table.
use super::*;
use crate::effect::Effect;
use crate::value::Condition;

#[rstest]
#[case::kicked_search(
    "Search your library for a basic land card, put that card onto the battlefield, then shuffle. If this spell was kicked, instead search your library for up to two basic land cards, put them onto the battlefield, then shuffle."
)]
#[case::threshold(
    "Search your library for a basic land card, put that card onto the battlefield tapped, then shuffle. If there are seven or more cards in your graveyard, instead search your library for up to three basic land cards, put them onto the battlefield tapped, then shuffle."
)]
#[case::kicked_draw("Draw a card. If this spell was kicked, instead draw three cards.")]
#[case::instead_last("Draw a card. If this spell was kicked, draw three cards instead.")]
#[case::lands_controlled(
    "Draw a card. If you control eight or more lands, draw two cards instead."
)]
fn a_second_sentence_replaces_the_first_under_a_condition(#[case] text: &str) {
    let read = golden(text, ClauseKind::Spell, false);
    let Ability::Spell { effects, targets } = read else {
        panic!("a spell ability");
    };
    assert_eq!(targets.len(), 0);
    let [
        Effect::If {
            then, otherwise, ..
        },
    ] = effects.as_slice()
    else {
        panic!("one If over both sentences: {effects:?}");
    };
    assert!(!then.is_empty() && !otherwise.is_empty());
}

#[test]
fn the_first_sentence_is_the_otherwise_and_the_condition_a_kicker() {
    let Ability::Spell { effects, .. } = golden(
        "Draw a card. If this spell was kicked, draw three cards instead.",
        ClauseKind::Spell,
        false,
    ) else {
        panic!("a spell ability");
    };
    let [
        Effect::If {
            condition,
            then,
            otherwise,
        },
    ] = effects.as_slice()
    else {
        panic!("one If");
    };
    assert_eq!(*condition, Condition::Kicked);
    assert!(matches!(then.as_slice(), [Effect::Draw { .. }]));
    assert!(matches!(otherwise.as_slice(), [Effect::Draw { .. }]));
    assert_ne!(then, otherwise);
}

#[rstest]
#[case::a_target(
    "Destroy target creature. If this spell was kicked, instead destroy target artifact."
)]
#[case::no_second_sentence("Draw a card. If this spell was kicked, instead.")]
#[case::instead_twice("Draw a card. If this spell was kicked, instead draw three cards instead.")]
#[case::an_unread_condition("Draw a card. If the moon is full, instead draw three cards.")]
#[case::an_unread_second_sentence("Draw a card. If this spell was kicked, instead frobnicate.")]
#[case::no_first_sentence("If this spell was kicked, instead draw three cards.")]
fn a_replacement_without_both_sentences_or_with_a_target_is_not_read(#[case] text: &str) {
    assert!(clause(text, ClauseKind::Spell, false).is_none(), "{text}");
}

#[test]
fn a_condition_without_instead_adds_an_effect_and_replaces_none() {
    let Ability::Spell { effects, .. } = golden(
        "Draw a card. If this spell was kicked, draw three cards.",
        ClauseKind::Spell,
        false,
    ) else {
        panic!("a spell ability");
    };
    assert!(matches!(
        effects.as_slice(),
        [Effect::Draw { .. }, Effect::If { otherwise, .. }] if otherwise.is_empty()
    ));
}

#[test]
fn a_kicked_sentence_alone_is_a_condition_of_one_effect() {
    let Ability::Spell { effects, .. } = golden(
        "If this spell was kicked, draw three cards.",
        ClauseKind::Spell,
        false,
    ) else {
        panic!("a spell ability");
    };
    assert!(matches!(
        effects.as_slice(),
        [Effect::If { condition: Condition::Kicked, otherwise, .. }] if otherwise.is_empty()
    ));
}

#[rstest]
#[case::the_other(
    "Search your library for up to two basic land cards, reveal those cards, put one onto the battlefield tapped and the other into your hand, then shuffle."
)]
#[case::the_rest_and_a_sentence_of_the_shuffle(
    "Search your library for up to two basic Forest cards, reveal those cards, and put one onto the battlefield tapped and the rest into your hand. Then shuffle."
)]
#[case::the_rest_in_a_comma(
    "Search your library for up to three basic land cards, put one onto the battlefield and the rest into your hand, then shuffle."
)]
fn one_card_found_goes_to_one_place_and_the_rest_to_another(#[case] text: &str) {
    let read = golden(text, ClauseKind::Spell, false);
    let Ability::Spell { effects, .. } = read else {
        panic!("a spell ability");
    };
    assert!(matches!(
        effects.as_slice(),
        [
            Effect::Search { .. },
            Effect::Choose {
                rest_as: Some(_),
                ..
            },
            Effect::Move { .. },
            Effect::Move { .. },
            Effect::Shuffle { .. },
        ]
    ));
}

#[rstest]
#[case::two_for_one_count(
    "Search your library for a basic land card, reveal it, put one onto the battlefield and the other into your hand, then shuffle."
)]
#[case::three_places(
    "Search your library for up to two basic land cards, put one onto the battlefield and the other onto the battlefield and the rest into your hand, then shuffle."
)]
#[case::no_second_place(
    "Search your library for up to two basic land cards, put one onto the battlefield and the other, then shuffle."
)]
#[case::another_player_searches(
    "That creature's controller may search their library for up to two basic land cards, put one onto the battlefield and the other into your hand, then shuffle."
)]
fn a_split_of_the_cards_found_needs_several_cards_and_two_places(#[case] text: &str) {
    assert!(clause(text, ClauseKind::Spell, false).is_none(), "{text}");
}

#[rstest]
#[case::desert(
    "Search your library for up to two basic land cards and/or Desert cards, put them onto the battlefield tapped, then shuffle."
)]
#[case::gate(
    "Search your library for up to two basic land cards and/or Gate cards, put them onto the battlefield tapped, then shuffle."
)]
#[case::town(
    "Search your library for up to two basic land cards and/or Town cards, put them onto the battlefield tapped, then shuffle."
)]
fn basic_lands_and_other_land_types_are_one_list(#[case] text: &str) {
    golden(text, ClauseKind::Spell, false);
}

#[rstest]
#[case::two_lists_of_types(
    "Search your library for an artifact card and/or creature card, reveal it, put it into your hand, then shuffle."
)]
#[case::not_a_land_type(
    "Search your library for up to two basic land cards and/or Wizard cards, put them onto the battlefield tapped, then shuffle."
)]
#[case::not_basic(
    "Search your library for up to two land cards and/or Gate cards, put them onto the battlefield tapped, then shuffle."
)]
#[case::singular(
    "Search your library for a basic land card and/or Gate card, put it onto the battlefield tapped, then shuffle."
)]
fn a_list_of_lands_names_basic_lands_and_land_types_only(#[case] text: &str) {
    assert!(clause(text, ClauseKind::Spell, false).is_none(), "{text}");
}

#[rstest]
#[case::two_types(
    "Search your library for up to two artifact and/or creature cards, reveal them, put them into your hand, then shuffle."
)]
#[case::three_types(
    "Search your library for up to two artifact, creature, and/or enchantment cards, reveal them, put them into your hand, then shuffle."
)]
#[case::two_subtypes(
    "Search your library for up to two Plains and/or Island cards, put them onto the battlefield tapped, then shuffle."
)]
fn and_or_stands_for_or_in_a_list_of_cards(#[case] text: &str) {
    golden(text, ClauseKind::Spell, false);
}
