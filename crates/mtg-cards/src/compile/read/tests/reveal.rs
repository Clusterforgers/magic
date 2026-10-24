//! Goal: "Look at the top card of your library. You may reveal that card. If <kind> card is revealed this way, <effects>." reads as a `LookAtTop` and a `May` of the `Reveal` and an `If` on the kind of the card, writes back as the same three sentences, and any other look at the top card, reveal or condition stays with the table.
use super::*;

fn trigger(text: &str) -> Option<Ability> {
    clause(text, ClauseKind::Triggered, false)
}

const DELVER: &str = r#"{"Triggered":{"when":{"StepBegins":{"step":"Upkeep","whose":"You"}},"effects":[{"LookAtTop":{"as":"top"}},{"May":{"effects":[{"Reveal":{"what":{"Bound":"top"}}},{"If":{"condition":{"Matches":{"subject":{"Bound":"top"},"selector":[{"AnyOf":[{"Type":"Instant"},{"Type":"Sorcery"}]}]}},"then":[{"Transform":{"what":"This"}}]}}]}}]}}"#;

#[rstest]
#[case::this_creature(
    "At the beginning of your upkeep, look at the top card of your library. You may reveal that card. If an instant or sorcery card is revealed this way, transform this creature."
)]
#[case::the_card(
    "At the beginning of your upkeep, look at the top card of your library. You may reveal that card. If an instant or sorcery card is revealed this way, transform ~."
)]
#[case::it(
    "At the beginning of your upkeep, look at the top card of your library. You may reveal it. If an instant or sorcery card is revealed this way, transform ~."
)]
fn delver_of_secrets_reads_in_the_shape_of_the_hand_written_file(#[case] text: &str) {
    reads_as(text, ClauseKind::Triggered, false, DELVER);
}

#[rstest]
#[case::a_land(
    "At the beginning of your upkeep, look at the top card of your library. You may reveal that card. If a land card is revealed this way, you gain 2 life.",
    r#"{"Triggered":{"when":{"StepBegins":{"step":"Upkeep","whose":"You"}},"effects":[{"LookAtTop":{"as":"top"}},{"May":{"effects":[{"Reveal":{"what":{"Bound":"top"}}},{"If":{"condition":{"Matches":{"subject":{"Bound":"top"},"selector":[{"Type":"Land"}]}},"then":[{"GainLife":{"amount":2}}]}}]}}]}}"#
)]
#[case::a_creature_with_a_mana_value(
    "Whenever ~ attacks, look at the top card of your library. You may reveal that card. If a creature card with mana value 3 or less is revealed this way, draw a card.",
    r#"{"Triggered":{"when":{"Attacks":{"what":["This"]}},"effects":[{"LookAtTop":{"as":"top"}},{"May":{"effects":[{"Reveal":{"what":{"Bound":"top"}}},{"If":{"condition":{"Matches":{"subject":{"Bound":"top"},"selector":[{"Type":"Creature"},{"Compare":{"stat":"ManaValue","op":"Le","to":3}}]}},"then":[{"Draw":{}}]}}]}}]}}"#
)]
#[case::two_steps(
    "Whenever ~ attacks, look at the top card of your library. You may reveal that card. If a Wolf card is revealed this way, draw a card and scry 1.",
    r#"{"Triggered":{"when":{"Attacks":{"what":["This"]}},"effects":[{"LookAtTop":{"as":"top"}},{"May":{"effects":[{"Reveal":{"what":{"Bound":"top"}}},{"If":{"condition":{"Matches":{"subject":{"Bound":"top"},"selector":[{"Subtype":"Wolf"}]}},"then":[{"Draw":{}},{"Scry":{}}]}}]}}]}}"#
)]
fn the_kind_of_the_revealed_card_and_the_effects_after_it_vary(
    #[case] text: &str,
    #[case] json: &str,
) {
    reads_as(text, ClauseKind::Triggered, false, json);
}

#[rstest]
#[case::no_reveal(
    "At the beginning of your upkeep, look at the top card of your library. If an instant or sorcery card is revealed this way, transform ~."
)]
#[case::reveal_without_may(
    "At the beginning of your upkeep, look at the top card of your library. Reveal that card. If an instant or sorcery card is revealed this way, transform ~."
)]
#[case::no_condition(
    "At the beginning of your upkeep, look at the top card of your library. You may reveal that card."
)]
#[case::a_pronoun_condition(
    "At the beginning of your upkeep, look at the top card of your library. You may reveal that card. If it's an instant or sorcery card, transform ~."
)]
#[case::two_cards(
    "At the beginning of your upkeep, look at the top two cards of your library. You may reveal that card. If an instant or sorcery card is revealed this way, transform ~."
)]
#[case::another_library(
    "At the beginning of your upkeep, look at the top card of target player's library. You may reveal that card. If an instant or sorcery card is revealed this way, transform ~."
)]
#[case::an_unread_effect(
    "At the beginning of your upkeep, look at the top card of your library. You may reveal that card. If an instant or sorcery card is revealed this way, flip two coins."
)]
#[case::an_otherwise(
    "At the beginning of your upkeep, look at the top card of your library. You may reveal that card. If an instant or sorcery card is revealed this way, transform ~. Otherwise, draw a card."
)]
#[case::a_name(
    "At the beginning of your upkeep, look at the top card of your library. You may reveal that card. If a Delver of Secrets card is revealed this way, transform ~."
)]
fn another_look_reveal_or_condition_stays_unread(#[case] text: &str) {
    assert_eq!(trigger(text), None, "{text}");
}

#[rstest]
#[case::alone("At the beginning of your upkeep, look at the top card of your library.")]
#[case::three("At the beginning of your upkeep, look at the top three cards of your library.")]
fn a_look_alone_is_still_read(#[case] text: &str) {
    golden(text, ClauseKind::Triggered, false);
}
