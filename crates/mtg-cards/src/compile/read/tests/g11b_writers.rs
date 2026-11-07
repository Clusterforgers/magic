//! Goal: damage that can't be prevented reads back with "The damage can't be prevented." and, after "Damage can't be
//! prevented this turn.", without it; plain damage with the sentence, the flag without it, and a rule for combat damage
//! only that does not cover the damage do not read back.
use super::*;

fn written(json: &str, clause: &str) -> bool {
    written_variants(&ability(json))
        .iter()
        .any(|wording| same_text(wording, clause))
}

fn spell(effects: &[&str]) -> String {
    format!(
        r#"{{"Spell":{{"effects":[{}],"targets":[{{"what":{{"Object":[{{"Type":"Creature"}}]}}}}]}}}}"#,
        effects.join(",")
    )
}

const HARD: &str = r#"{"DealDamage":{"amount":4,"to":{"Target":0},"unpreventable":true}}"#;
const SOFT: &str = r#"{"DealDamage":{"amount":4,"to":{"Target":0}}}"#;
const TURN: &str = r#"{"Rule":{"duration":"UntilEndOfTurn","rule":{"DamageCantBePrevented":{}}}}"#;
const COMBAT_TURN: &str =
    r#"{"Rule":{"duration":"UntilEndOfTurn","rule":{"DamageCantBePrevented":{"combat":true}}}}"#;
const ONE: &str = "~ deals 4 damage to target creature. The damage can't be prevented.";
const TURN_ONE: &str = "Damage can't be prevented this turn. ~ deals 4 damage to target creature.";

#[rstest]
#[case::the_sentence(&[HARD], ONE)]
#[case::after_the_turn_rule(&[TURN, HARD], TURN_ONE)]
#[case::plain_after_the_turn_rule(&[TURN, SOFT], TURN_ONE)]
fn damage_that_cant_be_prevented_reads_back(#[case] effects: &[&str], #[case] clause: &str) {
    let json = spell(effects);
    assert!(
        written(&json, clause),
        "{:?}",
        written_variants(&ability(&json))
    );
}

#[rstest]
#[case::plain_damage_with_the_sentence(&[SOFT], ONE)]
#[case::the_flag_without_the_sentence(&[HARD], "~ deals 4 damage to target creature.")]
#[case::other_amount(&[HARD], "~ deals 3 damage to target creature. The damage can't be prevented.")]
#[case::a_combat_rule_does_not_cover_it(&[COMBAT_TURN, HARD], "Combat damage can't be prevented this turn. ~ deals 4 damage to target creature.")]
#[case::the_rule_after_the_damage(&[HARD, TURN], "~ deals 4 damage to target creature. Damage can't be prevented this turn.")]
fn other_damage_does_not_read_back_as_it(#[case] effects: &[&str], #[case] clause: &str) {
    assert!(!written(&spell(effects), clause));
}
