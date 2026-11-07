//! Goal: damage that can't be prevented reads back with "The damage can't be prevented." and, after "Damage can't be
//! prevented this turn.", without it; plain damage with the sentence, the flag without it, and a rule for combat damage
//! only that does not cover the damage do not read back; a shield of the next X damage reads back with its X and its
//! count, and another count, a number or another recipient does not; a redirection reads back as the next time or as all
//! of it, and the other one, another recipient or a part does not.
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

fn counted(amount: &str, to: &str) -> String {
    format!(
        r#"{{"Activated":{{"cost":["Tap"],"effects":[{{"Shield":{{"by":{{"PreventCounted":{amount}}},"duration":"UntilEndOfTurn","when":{{"Damage":{{"to":{to}}}}}}}}}],"targets":[{{"what":{{"Object":[{{"Type":"Creature"}}]}}}}]}}}}"#
    )
}

const CLERICS: &str = r#"{"Count":[{"Subtype":"Cleric"}]}"#;
const MEDIC: &str = "{T}: Prevent the next X damage that would be dealt to target creature this turn, where X is the number of Clerics on the battlefield.";

#[rstest]
#[case::the_count(CLERICS, MEDIC)]
#[case::the_x_announced(
    r#""X""#,
    "{T}: Prevent the next X damage that would be dealt to target creature this turn."
)]
fn a_counted_shield_reads_back(#[case] amount: &str, #[case] clause: &str) {
    let json = counted(amount, r#"{"Target":0}"#);
    assert!(
        written(&json, clause),
        "{:?}",
        written_variants(&ability(&json))
    );
}

#[rstest]
#[case::another_count(r#"{"Count":[{"Subtype":"Elf"}]}"#, r#"{"Target":0}"#, MEDIC)]
#[case::a_number_in_the_words(
    r#""X""#,
    r#"{"Target":0}"#,
    "{T}: Prevent the next 3 damage that would be dealt to target creature this turn."
)]
#[case::the_count_dropped(
    CLERICS,
    r#"{"Target":0}"#,
    "{T}: Prevent the next X damage that would be dealt to target creature this turn."
)]
#[case::another_recipient(CLERICS, r#""You""#, MEDIC)]
fn another_counted_shield_does_not_read_back(
    #[case] amount: &str,
    #[case] to: &str,
    #[case] clause: &str,
) {
    assert!(!written(&counted(amount, to), clause));
}

fn redirect(by: &str, dealt: &str) -> String {
    format!(
        r#"{{"Activated":{{"cost":["Tap"],"effects":[{{"Shield":{{"by":{by},"duration":"UntilEndOfTurn","when":{{"Damage":{{"to":{dealt}}}}}}}}}],"targets":[{{"what":"AnyTarget"}}]}}}}"#
    )
}

const NEXT: &str = r#"{"Redirect":{"to":{"Target":0},"next":true}}"#;
const ALL: &str = r#"{"Redirect":{"to":{"Target":0}}}"#;
const NEXT_TEXT: &str = "{T}: The next time damage would be dealt to ~ this turn, that damage is dealt to any target instead.";
const ALL_TEXT: &str =
    "{T}: All damage that would be dealt to ~ this turn is dealt to any target instead.";

#[rstest]
#[case::the_next_time(NEXT, NEXT_TEXT)]
#[case::all_of_it(ALL, ALL_TEXT)]
fn a_redirection_reads_back(#[case] by: &str, #[case] clause: &str) {
    let json = redirect(by, r#""This""#);
    assert!(
        written(&json, clause),
        "{:?}",
        written_variants(&ability(&json))
    );
}

#[rstest]
#[case::all_as_the_next_time(ALL, r#""This""#, NEXT_TEXT)]
#[case::the_next_time_as_all(NEXT, r#""This""#, ALL_TEXT)]
#[case::another_recipient(NEXT, r#""You""#, NEXT_TEXT)]
#[case::a_part_of_it(
    NEXT,
    r#""This""#,
    "{T}: The next 2 damage that would be dealt to ~ this turn is dealt to any target instead."
)]
fn another_redirection_does_not_read_back(
    #[case] by: &str,
    #[case] dealt: &str,
    #[case] clause: &str,
) {
    assert!(!written(&redirect(by, dealt), clause));
}
