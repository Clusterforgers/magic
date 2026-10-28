//! Goal: a cost change that counts what happened this turn ("for each 1 life your opponents have lost this turn", "for each creature that attacked this turn", "for each creature that died this turn", "for each 1 life you gained this turn", "where X is the amount of life you gained this turn") reads into a tally of the turn, "The first creature spell you cast each turn costs {2} less to cast." into a modifier of the spells of that kind while none was cast, each writes back as the same text, also in an activated ability's own reduction, and every near-miss stays with the table.
use super::*;

fn stat(text: &str) -> Option<Ability> {
    clause(text, ClauseKind::Static, false)
}

/// A cost modifier as JSON.
fn modifier(applies_to: &str, mana: &str, rest: &str) -> String {
    format!(
        r#"{{"CostModifier":{{"applies_to":{applies_to},"change":"Reduce","mana":"{mana}",{rest}}}}}"#
    )
}

/// A tally of the turn as JSON.
fn tallied(who: &str, what: &str) -> String {
    format!(r#""per":{{"Tallied":{{"who":"{who}","what":"{what}"}}}}"#)
}

const CREATURES_YOU_CAST: &str = r#"["Spell",{"Type":"Creature"},{"ControlledBy":"You"}]"#;
const ITSELF: &str = r#"["This"]"#;

#[rstest]
#[case::life_your_opponents_lost(
    "Creature spells you cast cost {1} less to cast for each 1 life your opponents have lost this turn.",
    CREATURES_YOU_CAST,
    "{1}",
    "Opponents",
    "LifeLost"
)]
#[case::creatures_that_attacked(
    "This spell costs {1} less to cast for each creature that attacked this turn.",
    ITSELF,
    "{1}",
    "EachPlayer",
    "CreaturesAttacked"
)]
#[case::creatures_that_died(
    "This spell costs {1} less to cast for each creature that died this turn.",
    ITSELF,
    "{1}",
    "EachPlayer",
    "CreaturesDied"
)]
#[case::each_life_you_gained(
    "This spell costs {1} less to cast for each 1 life you gained this turn.",
    ITSELF,
    "{1}",
    "You",
    "LifeGainedTotal"
)]
#[case::x_is_the_life_you_gained(
    "This spell costs {X} less to cast, where X is the amount of life you gained this turn.",
    ITSELF,
    "{1}",
    "You",
    "LifeGainedTotal"
)]
#[case::x_for_your_creature_spells(
    "Creature spells you cast cost {X} less to cast, where X is the amount of life you gained this turn.",
    CREATURES_YOU_CAST,
    "{1}",
    "You",
    "LifeGainedTotal"
)]
fn a_cost_change_counts_a_tally_of_the_turn(
    #[case] text: &str,
    #[case] applies_to: &str,
    #[case] mana: &str,
    #[case] who: &str,
    #[case] what: &str,
) {
    reads_as(
        text,
        ClauseKind::Static,
        false,
        &modifier(applies_to, mana, &tallied(who, what)),
    );
}

#[rstest]
#[case::first_creature(
    "The first creature spell you cast each turn costs {2} less to cast.",
    CREATURES_YOU_CAST,
    "{2}",
    "CreatureSpellsCast",
    0
)]
#[case::first_artifact(
    "The first artifact spell you cast each turn costs {1} less to cast.",
    r#"["Spell",{"Type":"Artifact"},{"ControlledBy":"You"}]"#,
    "{1}",
    "ArtifactSpellsCast",
    0
)]
#[case::second_spell(
    "The second spell you cast each turn costs {2} less to cast.",
    r#"["Spell",{"ControlledBy":"You"}]"#,
    "{2}",
    "SpellsCast",
    1
)]
fn the_first_spell_of_a_kind_each_turn(
    #[case] text: &str,
    #[case] applies_to: &str,
    #[case] mana: &str,
    #[case] what: &str,
    #[case] before: u32,
) {
    let condition = format!(
        r#""condition":{{"Compare":{{"left":{{"Tallied":{{"who":"You","what":"{what}"}}}},"op":"Eq","right":{before}}}}}"#
    );
    reads_as(
        text,
        ClauseKind::Static,
        false,
        &modifier(applies_to, mana, &condition),
    );
}

#[test]
fn an_activated_ability_counts_a_tally_of_the_turn_for_its_own_cost() {
    let read = golden(
        "{5}, {T}: Draw a card. This ability costs {1} less to activate for each creature that died this turn.",
        ClauseKind::Activated,
        false,
    );
    assert!(
        serde_json::to_string(&read)
            .unwrap()
            .contains(r#""Tallied":{"who":"EachPlayer","what":"CreaturesDied"}"#)
    );
}

#[rstest]
#[case::an_opponent_you_attacked(
    "This spell costs {1} less to cast for each opponent you attacked this turn."
)]
#[case::life_lost_by_you(
    "This spell costs {1} less to cast for each 1 life you have lost this turn."
)]
#[case::two_life("This spell costs {1} less to cast for each 2 life you gained this turn.")]
#[case::x_without_a_where(
    "This spell costs {X} less to cast for each creature that died this turn."
)]
#[case::the_third_spell("The third spell you cast each turn costs {1} less to cast.")]
#[case::each_turn_missing("The first creature spell you cast costs {2} less to cast.")]
#[case::an_enchantment_spell(
    "The first enchantment spell you cast each turn costs {1} less to cast."
)]
#[case::an_x_reduction_of_the_first(
    "The first creature spell you cast each turn costs {X} less to cast."
)]
fn a_near_miss_stays_with_the_table(#[case] text: &str) {
    assert_eq!(stat(text), None, "{text}");
}
