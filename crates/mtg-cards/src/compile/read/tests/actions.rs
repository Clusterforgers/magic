//! Goal: the keyword actions of creatures (explore, connive) read into the DSL in the shapes of the cards, write back as the card's own text, and the near-misses stay with the table.
use super::*;

#[rstest]
#[case::the_card_explores_when_it_enters(
    "When ~ enters, it explores.",
    ClauseKind::Triggered,
    r#"{"Triggered":{"when":{"ZoneChange":{"what":["This"],"to":"Battlefield"}},"effects":[{"Explore":{"what":"This"}}]}}"#
)]
#[case::the_card_explores_by_name_when_it_enters(
    "When ~ enters, ~ explores.",
    ClauseKind::Triggered,
    r#"{"Triggered":{"when":{"ZoneChange":{"what":["This"],"to":"Battlefield"}},"effects":[{"Explore":{"what":"This"}}]}}"#
)]
#[case::twice_by_name(
    "When ~ enters, ~ explores, then it explores again.",
    ClauseKind::Triggered,
    r#"{"Triggered":{"when":{"ZoneChange":{"what":["This"],"to":"Battlefield"}},"effects":[{"Explore":{"what":"This"}},{"Explore":{"what":"This"}}]}}"#
)]
#[case::the_card_explores_when_it_attacks(
    "Whenever ~ attacks, it explores.",
    ClauseKind::Triggered,
    r#"{"Triggered":{"when":{"Attacks":{"what":["This"]}},"effects":[{"Explore":{"what":"This"}}]}}"#
)]
#[case::the_card_explores_by_name(
    "Whenever ~ deals combat damage to a player, ~ explores.",
    ClauseKind::Triggered,
    r#"{"Triggered":{"when":{"DealsDamage":{"source":["This"],"to":"EachPlayer","combat":true}},"effects":[{"Explore":{"what":"This"}}]}}"#
)]
#[case::the_card_explores_twice(
    "When ~ enters, it explores, then it explores again.",
    ClauseKind::Triggered,
    r#"{"Triggered":{"when":{"ZoneChange":{"what":["This"],"to":"Battlefield"}},"effects":[{"Explore":{"what":"This"}},{"Explore":{"what":"This"}}]}}"#
)]
#[case::an_explore_a_creature_you_control_causes(
    "Whenever a creature you control explores, you gain 1 life.",
    ClauseKind::Triggered,
    r#"{"Triggered":{"when":{"Explored":{"what":[{"Type":"Creature"},{"ControlledBy":"You"}]}},"effects":[{"GainLife":{"amount":1}}]}}"#
)]
#[case::a_target_explores(
    "Target creature you control explores.",
    ClauseKind::Spell,
    r#"{"Spell":{"targets":[{"what":{"Object":[{"Type":"Creature"},{"ControlledBy":"You"}]}}],"effects":[{"Explore":{"what":{"Target":0}}}]}}"#
)]
fn the_wordings_read_into_the_dsl_and_write_back(
    #[case] text: &str,
    #[case] kind: ClauseKind,
    #[case] json: &str,
) {
    reads_as(text, kind, false, json);
}

#[rstest]
#[case::each_creature_explores("Each creature you control explores.")]
#[case::explores_a_number_of_times("When ~ enters, it explores X times.")]
#[case::another_creature_by_name("Target creature explores twice.")]
#[case::it_without_a_card_that_acts("Draw a card. It explores.")]
fn near_misses_stay_with_the_table(#[case] text: &str) {
    for kind in [ClauseKind::Spell, ClauseKind::Triggered] {
        assert_eq!(clause(text, kind, false), None, "{text}");
    }
}

#[rstest]
#[case::the_card_connives_when_it_enters(
    "When ~ enters, it connives.",
    ClauseKind::Triggered,
    r#"{"Triggered":{"when":{"ZoneChange":{"what":["This"],"to":"Battlefield"}},"effects":[{"Connive":{"what":"This"}}]}}"#
)]
#[case::the_card_connives_when_it_attacks(
    "Whenever ~ attacks, it connives.",
    ClauseKind::Triggered,
    r#"{"Triggered":{"when":{"Attacks":{"what":["This"]}},"effects":[{"Connive":{"what":"This"}}]}}"#
)]
#[case::a_connive_a_creature_you_control_causes(
    "Whenever a creature you control connives, you gain 1 life.",
    ClauseKind::Triggered,
    r#"{"Triggered":{"when":{"Connived":{"what":[{"Type":"Creature"},{"ControlledBy":"You"}]}},"effects":[{"GainLife":{"amount":1}}]}}"#
)]
#[case::a_target_connives(
    "Target creature you control connives.",
    ClauseKind::Spell,
    r#"{"Spell":{"targets":[{"what":{"Object":[{"Type":"Creature"},{"ControlledBy":"You"}]}}],"effects":[{"Connive":{"what":{"Target":0}}}]}}"#
)]
fn connive_wordings_read_into_the_dsl_and_write_back(
    #[case] text: &str,
    #[case] kind: ClauseKind,
    #[case] json: &str,
) {
    reads_as(text, kind, false, json);
}

#[rstest]
#[case::connives_a_number_of_times("When ~ enters, it connives X.")]
#[case::each_creature_connives("Each creature you control connives.")]
#[case::up_to_one_target("Up to one target creature you control connives.")]
fn connive_near_misses_stay_with_the_table(#[case] text: &str) {
    for kind in [ClauseKind::Spell, ClauseKind::Triggered] {
        assert_eq!(clause(text, kind, false), None, "{text}");
    }
}
