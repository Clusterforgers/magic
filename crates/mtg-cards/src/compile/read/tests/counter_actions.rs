//! Goal: the keyword actions that put +1/+1 counters (monstrosity, adapt, bolster), the triggers on them and "as long as ~ is monstrous" read into the DSL in the shapes of the cards, write back as the card's own text, and the near-misses stay with the table.
use super::*;

#[rstest]
#[case::monstrosity(
    "{5}{R}{R}: Monstrosity 3.",
    ClauseKind::Activated,
    r#"{"Activated":{"cost":"{5}{R}{R}","effects":[{"Monstrosity":{"count":3}}]}}"#
)]
#[case::monstrosity_x(
    "{X}{X}{G}: Monstrosity X.",
    ClauseKind::Activated,
    r#"{"Activated":{"cost":"{X}{X}{G}","effects":[{"Monstrosity":{"count":"X"}}]}}"#
)]
#[case::adapt(
    "{2}{G}: Adapt 2.",
    ClauseKind::Activated,
    r#"{"Activated":{"cost":"{2}{G}","effects":[{"Adapt":{"count":2}}]}}"#
)]
#[case::adapt_for_two_colors(
    "{2}{G}{U}: Adapt 1.",
    ClauseKind::Activated,
    r#"{"Activated":{"cost":"{2}{G}{U}","effects":[{"Adapt":{"count":1}}]}}"#
)]
#[case::monstrosity_for_two_colors(
    "{5}{G}{U}: Monstrosity 3.",
    ClauseKind::Activated,
    r#"{"Activated":{"cost":"{5}{G}{U}","effects":[{"Monstrosity":{"count":3}}]}}"#
)]
#[case::bolster_by_an_enchantment(
    "{2}{W}: Bolster 1.",
    ClauseKind::Activated,
    r#"{"Activated":{"cost":"{2}{W}","effects":[{"Bolster":{"count":1}}]}}"#
)]
#[case::bolster_when_it_enters(
    "When ~ enters, bolster 2.",
    ClauseKind::Triggered,
    r#"{"Triggered":{"when":{"ZoneChange":{"what":["This"],"to":"Battlefield"}},"effects":[{"Bolster":{"count":2}}]}}"#
)]
#[case::bolster_when_it_attacks(
    "Whenever ~ attacks, bolster 1.",
    ClauseKind::Triggered,
    r#"{"Triggered":{"when":{"Attacks":{"what":["This"]}},"effects":[{"Bolster":{"count":1}}]}}"#
)]
#[case::bolster_when_it_dies(
    "When ~ dies, bolster 2.",
    ClauseKind::Triggered,
    r#"{"Triggered":{"when":{"ZoneChange":{"what":["This"],"from":"Battlefield","to":"Graveyard"}},"effects":[{"Bolster":{"count":2}}]}}"#
)]
#[case::a_bolster_spell(
    "Bolster 3.",
    ClauseKind::Spell,
    r#"{"Spell":{"effects":[{"Bolster":{"count":3}}]}}"#
)]
#[case::bolster_after_another_effect(
    "You gain 4 life. Bolster 2.",
    ClauseKind::Spell,
    r#"{"Spell":{"effects":[{"GainLife":{"amount":4}},{"Bolster":{"count":2}}]}}"#
)]
fn counter_action_wordings_read_into_the_dsl_and_write_back(
    #[case] text: &str,
    #[case] kind: ClauseKind,
    #[case] json: &str,
) {
    reads_as(text, kind, false, json);
}

#[rstest]
#[case::the_card_becomes_monstrous(
    "When ~ becomes monstrous, you gain 2 life.",
    r#"{"Triggered":{"when":{"BecomesMonstrous":{"what":["This"]}},"effects":[{"GainLife":{"amount":2}}]}}"#
)]
#[case::it_enters_or_becomes_monstrous(
    "When ~ enters or becomes monstrous, you gain 2 life.",
    r#"{"Triggered":{"when":{"AnyOf":[{"ZoneChange":{"what":["This"],"to":"Battlefield"}},{"BecomesMonstrous":{"what":["This"]}}]},"effects":[{"GainLife":{"amount":2}}]}}"#
)]
#[case::a_creature_adapts(
    "Whenever a creature you control adapts, you gain 1 life.",
    r#"{"Triggered":{"when":{"Adapted":{"what":[{"Type":"Creature"},{"ControlledBy":"You"}]}},"effects":[{"GainLife":{"amount":1}}]}}"#
)]
#[case::you_bolster(
    "Whenever you bolster, you gain 1 life.",
    r#"{"Triggered":{"when":{"Bolstered":{"who":"You"}},"effects":[{"GainLife":{"amount":1}}]}}"#
)]
fn triggers_on_the_counter_actions_read_into_the_dsl_and_write_back(
    #[case] text: &str,
    #[case] json: &str,
) {
    reads_as(text, ClauseKind::Triggered, false, json);
}

#[rstest]
#[case::menace_while_monstrous(
    "As long as ~ is monstrous, it has menace.",
    r#"{"Static":{"affected":"This","mods":[{"AddKeyword":"Menace"}],"condition":{"Matches":{"subject":"This","selector":["Monstrous"]}}}}"#
)]
fn a_static_while_monstrous_reads_into_the_dsl_and_writes_back(
    #[case] text: &str,
    #[case] json: &str,
) {
    reads_as(text, ClauseKind::Static, false, json);
}

#[rstest]
#[case::monstrosity_without_a_number("{5}{R}{R}: Monstrosity.")]
#[case::adapt_without_a_number("{2}{G}: Adapt.")]
#[case::bolster_a_creature("Bolster target creature 2.")]
#[case::monstrous_as_a_target("Target monstrous creature gains haste until end of turn.")]
fn near_misses_of_the_counter_actions_stay_with_the_table(#[case] text: &str) {
    for kind in [
        ClauseKind::Spell,
        ClauseKind::Activated,
        ClauseKind::Triggered,
    ] {
        assert_eq!(clause(text, kind, false), None, "{text}");
    }
}
