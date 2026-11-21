//! Goal: in a spell, "other" after a creation reads as other than the tokens the spell created (`Not(Is(Bound))` with the creation bound), under "If X is N or more" too, and writes back as the same text; the shape whose `Other` means the spell itself has no wording, and "other" with no creation before it stays as it was.
use super::*;

const SOLDIERS: &str = r#"{"CreateToken":{"token":{"Custom":{"name":"Soldier","colors":["White"],"types":["Creature"],"subtypes":["Soldier"],"power":1,"toughness":1}},"count":"X","as":"tokens"}}"#;

const OTHERS: &str = r#"{"Destroy":{"what":{"Each":[{"Type":"Creature"},{"Not":{"Is":{"Bound":"tokens"}}}]}}}"#;

#[rstest]
#[case::martial_coup(
    "Create X 1/1 white Soldier creature tokens. If X is 5 or more, destroy all other creatures.",
    format!(r#"{{"Spell":{{"effects":[{SOLDIERS},{{"If":{{"condition":{{"Compare":{{"left":"X","op":"Ge","right":5}}}},"then":[{OTHERS}]}}}}]}}}}"#)
)]
#[case::in_words(
    "Create X 1/1 white Soldier creature tokens. If X is three or less, destroy all other creatures.",
    format!(r#"{{"Spell":{{"effects":[{SOLDIERS},{{"If":{{"condition":{{"Compare":{{"left":"X","op":"Le","right":3}}}},"then":[{OTHERS}]}}}}]}}}}"#)
)]
#[case::always(
    "Create X 1/1 white Soldier creature tokens. Destroy all other creatures.",
    format!(r#"{{"Spell":{{"effects":[{SOLDIERS},{OTHERS}]}}}}"#)
)]
fn other_after_a_creation_spares_the_tokens(#[case] text: &str, #[case] json: String) {
    reads_as(text, ClauseKind::Spell, true, &json);
}

#[test]
fn other_meaning_the_spell_after_a_creation_has_no_wording() {
    let wrong = ability(&format!(
        r#"{{"Spell":{{"effects":[{},{{"Destroy":{{"what":{{"Each":[{{"Type":"Creature"}},"Other"]}}}}}}]}}}}"#,
        SOLDIERS.replace(r#","as":"tokens""#, "")
    ));
    assert_eq!(render_variants(&wrong), Vec::<String>::new());
}

#[test]
fn other_without_a_creation_stays_other_than_the_source() {
    reads_as(
        "Destroy all other creatures.",
        ClauseKind::Spell,
        false,
        r#"{"Spell":{"effects":[{"Destroy":{"what":{"Each":[{"Type":"Creature"},"Other"]}}}]}}"#,
    );
}

#[rstest]
#[case::no_x_announced("Create a 1/1 white Soldier creature token. If X is 5 or more, destroy all other creatures.", false)]
#[case::no_bound("Create X 1/1 white Soldier creature tokens. If X is 5, destroy all other creatures.", true)]
fn near_misses_stay_with_the_table(#[case] text: &str, #[case] x_free: bool) {
    assert_eq!(clause(text, ClauseKind::Spell, x_free), None, "{text}");
}
