//! Goal: "transform ~", "transform it" and "transform target ..." read as `Effect::Transform` in a spell, a trigger and an activated ability (with its timing and limit), write back as the same text, and a wording the template does not model stays with the table.
use super::*;

fn activated(text: &str) -> Option<Ability> {
    clause(text, ClauseKind::Activated, false)
}

fn trigger(text: &str) -> Option<Ability> {
    clause(text, ClauseKind::Triggered, false)
}

#[rstest]
#[case::the_card_itself(
    "At the beginning of your upkeep, transform ~.",
    r#"{"Triggered":{"when":{"StepBegins":{"step":"Upkeep","whose":"You"}},"effects":[{"Transform":{"what":"This"}}]}}"#
)]
#[case::this_creature(
    "At the beginning of your upkeep, transform this creature.",
    r#"{"Triggered":{"when":{"StepBegins":{"step":"Upkeep","whose":"You"}},"effects":[{"Transform":{"what":"This"}}]}}"#
)]
#[case::you_may(
    "At the beginning of your upkeep, you may transform ~.",
    r#"{"Triggered":{"when":{"StepBegins":{"step":"Upkeep","whose":"You"}},"effects":[{"May":{"effects":[{"Transform":{"what":"This"}}]}}]}}"#
)]
#[case::after_counters_it(
    "Whenever ~ attacks, put a time counter on ~. Transform it.",
    r#"{"Triggered":{"when":{"Attacks":{"what":["This"]}},"effects":[{"PutCounters":{"on":"This","counter":{"Other":"time"}}},{"Transform":{"what":"This"}}]}}"#
)]
fn transform_in_a_trigger_reads_in_the_shape_of_the_hand_written_files(
    #[case] text: &str,
    #[case] json: &str,
) {
    reads_as(text, ClauseKind::Triggered, false, json);
}

#[rstest]
#[case::mana(
    "{1}{G}: Transform ~.",
    r#"{"Activated":{"cost":"{1}{G}","effects":[{"Transform":{"what":"This"}}]}}"#
)]
#[case::tap_and_mana(
    "{2}{R}, {T}: Transform ~.",
    r#"{"Activated":{"cost":[{"Mana":"{2}{R}"},"Tap"],"effects":[{"Transform":{"what":"This"}}]}}"#
)]
#[case::at_sorcery_speed(
    "{3}: Transform ~. Activate only as a sorcery.",
    r#"{"Activated":{"cost":"{3}","timing":"Sorcery","effects":[{"Transform":{"what":"This"}}]}}"#
)]
#[case::once_each_turn(
    "{1}: Transform ~. Activate only once each turn.",
    r#"{"Activated":{"cost":"{1}","limit":"OncePerTurn","effects":[{"Transform":{"what":"This"}}]}}"#
)]
#[case::after_counters(
    "{1}{U}: Put a hatchling counter on ~. Transform it.",
    r#"{"Activated":{"cost":"{1}{U}","effects":[{"PutCounters":{"on":"This","counter":{"Other":"hatchling"}}},{"Transform":{"what":"This"}}]}}"#
)]
fn transform_in_an_activated_ability_reads_with_its_restrictions(
    #[case] text: &str,
    #[case] json: &str,
) {
    reads_as(text, ClauseKind::Activated, false, json);
}

#[rstest]
#[case::a_creature_you_control(
    "Transform target creature you control.",
    r#"{"Spell":{"targets":[{"what":{"Object":[{"Type":"Creature"},{"ControlledBy":"You"}]}}],"effects":[{"Transform":{"what":{"Target":0}}}]}}"#
)]
#[case::a_permanent(
    "Transform target permanent.",
    r#"{"Spell":{"targets":[{"what":{"Object":[]}}],"effects":[{"Transform":{"what":{"Target":0}}}]}}"#
)]
fn transform_of_a_target_reads_as_a_spell(#[case] text: &str, #[case] json: &str) {
    reads_as(text, ClauseKind::Spell, false, json);
}

#[rstest]
#[case::no_object("Transform.")]
#[case::all("Transform all creatures you control.")]
#[case::a_card_in_a_graveyard("Transform target card in your graveyard.")]
#[case::a_spell("Transform target spell.")]
#[case::two_targets("Transform up to two target creatures.")]
#[case::a_player("Transform target player.")]
#[case::a_name("Transform Delver of Secrets.")]
#[case::it_with_nothing_before("Transform it.")]
#[case::untransform("Untransform ~.")]
#[case::if_it_is_night("Transform ~ if it's night.")]
#[case::the_other_face("Transform ~ into its night side.")]
fn a_transform_the_template_does_not_model_stays_unread(#[case] text: &str) {
    assert_eq!(spell(text), None, "{text}");
    assert_eq!(
        trigger(&format!(
            "At the beginning of your upkeep, {}",
            lower_first(text)
        )),
        None,
        "{text}"
    );
}

#[rstest]
#[case::only_if("{1}{G}: Transform ~. Activate only if an opponent controls a Wolf.")]
#[case::only_during("{1}{G}: Transform ~. Activate only during your turn.")]
#[case::two_sentences_of_restriction(
    "{1}{G}: Transform ~. Activate only as a sorcery. Draw a card."
)]
#[case::another_object("{1}{G}: Transform another creature you control.")]
fn an_activation_with_a_condition_or_another_object_stays_unread(#[case] text: &str) {
    assert_eq!(activated(text), None, "{text}");
}

fn lower_first(text: &str) -> String {
    let mut chars = text.chars();
    chars
        .next()
        .map(|first| format!("{}{}", first.to_lowercase(), chars.as_str()))
        .unwrap_or_default()
}
