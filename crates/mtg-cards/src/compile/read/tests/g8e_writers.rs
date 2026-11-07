//! Goal: a shield for the turn of the damage dealt to, by or to and by one creature, and the damage an Aura or
//! Equipment prevents for its creature, are written in their Oracle wordings, and a rule of other damage, another side,
//! another creature or another duration does not read back.
use super::*;

fn wordings(json: &str) -> Vec<String> {
    written_variants(&ability(json))
}

fn written(json: &str, clause: &str) -> bool {
    wordings(json)
        .iter()
        .any(|wording| same_text(wording, clause))
}

const ONE: &str = r#"{"what":{"Object":[{"Type":"Creature"}]}}"#;

fn spell(effects: &str) -> String {
    format!(r#"{{"Spell":{{"effects":[{effects}],"targets":[{ONE}]}}}}"#)
}

fn shield(damage: &str) -> String {
    format!(
        r#"{{"Shield":{{"by":"Prevent","duration":"UntilEndOfTurn","when":{{"Damage":{damage}}}}}}}"#
    )
}

fn pump_then(damage: &str) -> String {
    spell(&format!(
        r#"{{"Continuous":{{"affected":{{"Target":0}},"duration":"UntilEndOfTurn","mods":[{{"AddKeyword":"Flying"}}]}}}},{}"#,
        shield(damage)
    ))
}

fn activated(effects: &str) -> String {
    format!(r#"{{"Activated":{{"cost":[{{"Mana":"{{U}}"}}],"effects":[{effects}]}}}}"#)
}

fn aura(damage: &str) -> String {
    format!(r#"{{"Replacement":{{"by":"Prevent","when":{{"Damage":{damage}}}}}}}"#)
}

const TO_COMBAT: &str = r#"{"to":{"Target":0},"combat":true}"#;
const BY_COMBAT: &str = r#"{"source":[{"Is":{"Target":0}}],"combat":true}"#;
const BY_ALL: &str = r#"{"source":[{"Is":{"Target":0}}]}"#;
const SELF_TO: &str = r#"{"to":"This","combat":true}"#;
const SELF_BY: &str = r#"{"source":[{"Is":"This"}],"combat":true}"#;

#[rstest]
#[case::to_it(
    pump_then(TO_COMBAT),
    "Target creature gains flying until end of turn. Prevent all combat damage that would be dealt to it this turn."
)]
#[case::by_target(
    spell(&shield(BY_COMBAT)),
    "Prevent all combat damage that would be dealt by target creature this turn."
)]
#[case::by_this_turn_first(
    spell(&shield(BY_COMBAT)),
    "Prevent all combat damage that would be dealt this turn by target creature."
)]
#[case::by_would_deal(
    spell(&shield(BY_ALL)),
    "Prevent all damage that would be dealt this turn by target creature."
)]
#[case::to_and_by(
    activated(&format!("{},{}", shield(SELF_TO), shield(SELF_BY))),
    "{U}: Prevent all combat damage that would be dealt to and dealt by ~ this turn."
)]
#[case::by_then_to_that_creature(
    spell(&format!(
        r#"{{"Untap":{{"what":{{"Target":0}}}}}},{},{}"#,
        shield(BY_COMBAT),
        shield(TO_COMBAT)
    )),
    "Untap target creature. Prevent all combat damage that would be dealt to and dealt by that creature this turn."
)]
#[case::this_turn_to(
    spell(&shield(r#"{"to":{"Target":0}}"#)),
    "Prevent all damage that would be dealt this turn to target creature."
)]
#[case::it_for_this(
    format!(
        r#"{{"Triggered":{{"when":{{"Attacks":{{"what":["This"]}}}},"effects":[{{"PutCounters":{{"counter":"+1/+1","on":"This"}}}},{}]}}}}"#,
        shield(r#"{"to":"This"}"#)
    ),
    "Whenever ~ attacks, put a +1/+1 counter on ~ and prevent all damage that would be dealt to it this turn."
)]
#[case::aura_by(
    aura(r#"{"source":[{"Is":"Attached"}]}"#),
    "Prevent all damage that would be dealt by enchanted creature."
)]
#[case::equipment_to(
    aura(r#"{"to":"Attached","combat":true}"#),
    "Prevent all combat damage that would be dealt to equipped creature."
)]
fn writes_the_oracle_wording(#[case] json: String, #[case] clause: &str) {
    assert!(written(&json, clause), "{clause}: {:?}", wordings(&json));
}

#[rstest]
#[case::all_for_combat(
    spell(&shield(BY_COMBAT)),
    "Prevent all damage that would be dealt by target creature this turn."
)]
#[case::to_for_by(
    spell(&shield(BY_COMBAT)),
    "Prevent all combat damage that would be dealt to target creature this turn."
)]
#[case::by_for_to(
    pump_then(TO_COMBAT),
    "Target creature gains flying until end of turn. Prevent all combat damage that would be dealt by it this turn."
)]
#[case::other_creature(
    spell(&shield(BY_COMBAT)),
    "Prevent all combat damage that would be dealt by target creature you control this turn."
)]
#[case::longer(
    spell(&shield(BY_COMBAT)),
    "Prevent all combat damage that would be dealt by target creature until your next turn."
)]
#[case::one_side_for_both(
    activated(&shield(SELF_TO)),
    "{U}: Prevent all combat damage that would be dealt to and dealt by ~ this turn."
)]
#[case::it_after_a_target(
    format!(
        r#"{{"Activated":{{"cost":["Tap"],"effects":[{{"DealDamage":{{"amount":1,"to":{{"Target":0}}}}}},{}],"targets":[{ONE}]}}}}"#,
        shield(r#"{"to":"This"}"#)
    ),
    "{T}: ~ deals 1 damage to target creature. Prevent all damage that would be dealt to it this turn."
)]
#[case::pair_of_two_kinds(
    activated(&format!(
        "{},{}",
        shield(SELF_TO),
        shield(r#"{"source":[{"Is":"This"}]}"#)
    )),
    "{U}: Prevent all combat damage that would be dealt to and dealt by ~ this turn."
)]
#[case::aura_to_for_by(
    aura(r#"{"to":"Attached"}"#),
    "Prevent all damage that would be dealt by enchanted creature."
)]
#[case::aura_combat_for_all(
    aura(r#"{"to":"Attached","combat":true}"#),
    "Prevent all damage that would be dealt to equipped creature."
)]
#[case::aura_for_this(
    aura(r#"{"to":"This","combat":true}"#),
    "Prevent all combat damage that would be dealt to equipped creature."
)]
fn a_changed_clause_differs(#[case] json: String, #[case] clause: &str) {
    assert!(!written(&json, clause), "{clause}: {:?}", wordings(&json));
}
