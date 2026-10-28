//! Goal: "This ability costs {N} less (or more) to activate" after the effects of an activated ability ("for each" a count, "if" a condition; before "Activate only ...") reads into a `CostItem::Changed` at the end of the cost, writes back as the same text, and every near-miss stays with the table.
use super::*;
use crate::ability::Timing;
use crate::cost::CostItem;

fn activated(text: &str) -> Option<Ability> {
    clause(text, ClauseKind::Activated, false)
}

/// The last part of the cost of the activated ability read from `text`, as JSON.
fn less_of(text: &str) -> String {
    let Ability::Activated { cost, .. } = golden(text, ClauseKind::Activated, false) else {
        panic!("not an activated ability: {text}");
    };
    serde_json::to_string(cost.0.last().unwrap()).unwrap()
}

#[rstest]
#[case::cards_in_your_graveyard(
    "{7}{U}: Draw a card. This ability costs {1} less to activate for each instant and sorcery card in your graveyard.",
    r#"{"Changed":{"change":"Reduce","mana":"{1}","per":{"Count":[{"AnyOf":[{"Type":"Instant"},{"Type":"Sorcery"}]},{"In":"Graveyard"},{"OwnedBy":"You"}]}}}"#
)]
#[case::other_artifacts(
    "{5}{U}, {T}: Draw a card. This ability costs {1} less to activate for each other artifact you control.",
    r#"{"Changed":{"change":"Reduce","mana":"{1}","per":{"Count":[{"Type":"Artifact"},"Other",{"ControlledBy":"You"}]}}}"#
)]
#[case::a_subtype(
    "{5}, {T}: Create a Blood token. This ability costs {1} less to activate for each Vampire you control.",
    r#"{"Changed":{"change":"Reduce","mana":"{1}","per":{"Count":[{"Subtype":"Vampire"},{"ControlledBy":"You"}]}}}"#
)]
#[case::legendary_creatures(
    "{5}, {T}: Scry 1, then draw a card. This ability costs {1} less to activate for each legendary creature you control.",
    r#"{"Changed":{"change":"Reduce","mana":"{1}","per":{"Count":[{"Type":"Creature"},{"Supertype":"Legendary"},{"ControlledBy":"You"}]}}}"#
)]
#[case::counters_on_it_before_a_restriction(
    "{8}, {T}: Draw a card. This ability costs {1} less to activate for each charge counter on ~. Activate only as a sorcery.",
    r#"{"Changed":{"change":"Reduce","mana":"{1}","per":{"Counters":{"on":"This","counter":"charge"}}}}"#
)]
#[case::a_condition(
    "{4}{B}{B}: Draw a card. This ability costs {4} less to activate if you control an artifact.",
    r#"{"Changed":{"change":"Reduce","mana":"{4}","condition":{"Compare":{"left":{"Count":[{"Type":"Artifact"},{"ControlledBy":"You"}]},"op":"Ge","right":1}}}}"#
)]
#[case::more_for_each_card_in_your_hand(
    "{3}, {T}: Draw three cards. This ability costs {1} more to activate for each card in your hand.",
    r#"{"Changed":{"change":"Increase","mana":"{1}","per":{"Count":[{"In":"Hand"},{"OwnedBy":"You"}]}}}"#
)]
fn a_change_of_the_ability_itself(#[case] text: &str, #[case] json: &str) {
    assert_eq!(less_of(text), json, "{text}");
}

#[test]
fn the_reduction_is_the_last_part_and_the_paid_parts_stay() {
    let Some(Ability::Activated { cost, timing, .. }) = activated(
        "{8}, {T}: Draw a card. This ability costs {1} less to activate for each charge counter on ~. Activate only as a sorcery.",
    ) else {
        panic!("not read");
    };
    assert_eq!(cost.0.len(), 3);
    assert_eq!(cost.0[1], CostItem::Tap);
    assert_eq!(timing, Timing::Sorcery);
}

#[rstest]
#[case::neither_more_nor_less(
    "{3}: Draw a card. This ability costs {1} fewer to activate for each artifact you control."
)]
#[case::to_cast(
    "{3}: Draw a card. This ability costs {1} less to cast for each artifact you control."
)]
#[case::an_x_reduction("{3}: Draw a card. This ability costs {X} less to activate.")]
#[case::no_period_then_more_words(
    "{3}: Draw a card. This ability costs {1} less to activate for each artifact you control and draws a card."
)]
#[case::two_reductions(
    "{3}: Draw a card. This ability costs {1} less to activate. This ability costs {1} less to activate."
)]
#[case::before_the_effects("{3}: This ability costs {1} less to activate. Draw a card.")]
fn a_near_miss_stays_with_the_table(#[case] text: &str) {
    assert_eq!(activated(text), None, "{text}");
}
