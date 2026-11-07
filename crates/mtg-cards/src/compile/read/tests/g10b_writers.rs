//! Goal: a counter that names the spell it countered reads back as "counter target spell" with the exile instead of the
//! graveyard, as the unnamed counter does, and another zone, another kind of object or a missing shield does not read back
//! as it.
use super::*;

fn written(json: &str, clause: &str) -> bool {
    written_variants(&ability(json))
        .iter()
        .any(|wording| same_text(wording, clause))
}

const SHIELD: &str = r#"{"Shield":{"by":{"MoveTo":"Exile"},"duration":"UntilEndOfTurn","when":{"Countered":{"what":[{"Is":{"Target":0}},"Spell"]}}}}"#;
const NAMED: &str = r#"{"Counter":{"as":"countered","what":{"Target":0}}}"#;
const EXILED: &str = "Counter target spell. If that spell is countered this way, exile it instead of putting it into its owner's graveyard.";

fn spell(effects: &[&str]) -> String {
    format!(
        r#"{{"Spell":{{"effects":[{}],"targets":[{{"what":{{"Object":["Spell"]}}}}]}}}}"#,
        effects.join(",")
    )
}

#[test]
fn a_named_counter_with_the_exile_shield_reads_back() {
    let json = spell(&[SHIELD, NAMED]);
    assert!(
        written(&json, EXILED),
        "{:?}",
        written_variants(&ability(&json))
    );
}

#[rstest]
#[case::to_the_hand(&[&SHIELD.replace(r#""MoveTo":"Exile""#, r#""MoveTo":"Hand""#), NAMED], EXILED)]
#[case::no_shield(&[NAMED], EXILED)]
#[case::a_creature_shield(&[&SHIELD.replace(r#""Spell""#, r#"{"Type":"Creature"}"#), NAMED], EXILED)]
#[case::the_shield_dropped_from_the_words(&[SHIELD, NAMED], "Counter target spell.")]
fn another_counter_does_not_read_back_as_it(#[case] effects: &[&str], #[case] clause: &str) {
    assert!(!written(&spell(effects), clause));
}
