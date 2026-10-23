//! Goal: "for each ..., create a token that's a copy of that ..." and the exceptions of a permanent that enters as a copy (a list that names its subject once, a type that replaces the others with its subtypes and keywords, a quoted ability, a creature an opponent controls) read into `CopyOf` over a group and `AsCopy`, write back as the card's text, and the near-misses stay with the table.
use super::*;

fn each_token(group: &str) -> String {
    format!(r#"{{"CreateToken":{{"token":{{"CopyOf":{{"of":{{"Each":{group}}}}}}}}}}}"#)
}

#[rstest]
#[case::every_token(
    "For each token you control, create a token that's a copy of that permanent.",
    r#"["Token",{"ControlledBy":"You"}]"#
)]
#[case::every_creature_token(
    "For each creature token you control, create a token that's a copy of that creature.",
    r#"[{"Type":"Creature"},"Token",{"ControlledBy":"You"}]"#
)]
#[case::every_artifact(
    "For each artifact you control, create a token that's a copy of that artifact.",
    r#"[{"Type":"Artifact"},{"ControlledBy":"You"}]"#
)]
fn a_token_is_made_as_a_copy_of_each_permanent_of_a_group(#[case] text: &str, #[case] group: &str) {
    reads_as(
        text,
        ClauseKind::Spell,
        false,
        &format!(r#"{{"Spell":{{"effects":[{}]}}}}"#, each_token(group)),
    );
}

#[test]
fn an_activated_ability_makes_a_token_as_a_copy_of_each_creature_token() {
    reads_as(
        "{4}{G/W}{G/W}, {T}: For each creature token you control, create a token that's a copy of that creature.",
        ClauseKind::Activated,
        false,
        &format!(
            r#"{{"Activated":{{"cost":[{{"Mana":"{{4}}{{G/W}}{{G/W}}"}},"Tap"],"effects":[{}]}}}}"#,
            each_token(r#"[{"Type":"Creature"},"Token",{"ControlledBy":"You"}]"#)
        ),
    );
}

#[rstest]
#[case::no_holder("For each token, create a token that's a copy of that permanent.")]
#[case::another_noun(
    "For each creature you control, create a token that's a copy of that permanent."
)]
#[case::the_wrong_noun(
    "For each creature token you control, create a token that's a copy of that artifact."
)]
#[case::several_tokens(
    "For each token you control, create two tokens that are copies of that permanent."
)]
#[case::the_target_instead(
    "For each token you control, create a token that's a copy of target permanent."
)]
fn near_misses_of_a_copy_for_each_stay_with_the_table(#[case] text: &str) {
    assert_eq!(spell(text), None, "{text}");
}

fn enters(of: &str, except: &str) -> String {
    format!(
        r#"{{"Replacement":{{"when":{{"Enters":{{"what":["This"]}}}},"by":{{"AsCopy":{{"of":{of},"except":[{except}]}}}}}}}}"#
    )
}

const ANY_CREATURE: &str = r#"[{"Type":"Creature"}]"#;
const OPPONENTS_CREATURE: &str = r#"[{"Type":"Creature"},{"ControlledBy":"Opponents"}]"#;

#[rstest]
#[case::a_list_that_names_its_subject_once(
    "You may have this creature enter as a copy of any creature on the battlefield, except it isn't legendary, is an artifact in addition to its other types, and has myriad.",
    ANY_CREATURE,
    r#"{"RemoveSupertype":"Legendary"},{"AddType":"Artifact"},{"AddKeyword":"Myriad"}"#
)]
#[case::the_same_parts_joined_by_and(
    "You may have this creature enter as a copy of any creature on the battlefield, except it isn't legendary and it's an artifact in addition to its other types and it has myriad.",
    ANY_CREATURE,
    r#"{"RemoveSupertype":"Legendary"},{"AddType":"Artifact"},{"AddKeyword":"Myriad"}"#
)]
#[case::a_creature_an_opponent_controls(
    "You may have this creature enter as a copy of a creature an opponent controls, except it's a Faerie Shapeshifter in addition to its other types and it has flying.",
    OPPONENTS_CREATURE,
    r#"{"AddSubtype":"Faerie"},{"AddSubtype":"Shapeshifter"},{"AddKeyword":"Flying"}"#
)]
#[case::a_type_alone_and_a_quoted_mana_ability(
    "You may have this artifact enter as a copy of any creature on the battlefield, except it's an artifact and it has \"{T}: Add {U}.\"",
    ANY_CREATURE,
    r#"{"SetTypes":["Artifact"]},{"GrantAbility":{"Activated":{"cost":["Tap"],"effects":[{"AddMana":{"mana":{"Fixed":["Blue"]}}}]}}}"#
)]
#[case::a_type_alone_with_the_others_lost(
    "You may have this artifact enter as a copy of any creature on the battlefield, except it's an artifact and it loses all other card types.",
    ANY_CREATURE,
    r#"{"SetTypes":["Artifact"]}"#
)]
#[case::a_type_alone_terse(
    "You may have this artifact enter as a copy of any creature on the battlefield, except it's an artifact.",
    ANY_CREATURE,
    r#"{"SetTypes":["Artifact"]}"#
)]
#[case::a_subtype_and_keyword_with_the_type(
    "You may have this creature enter as a copy of a creature an opponent controls, except it's a Vehicle artifact with crew 3 and it loses all other card types.",
    OPPONENTS_CREATURE,
    r#"{"SetTypes":["Artifact"]},{"AddSubtype":"Vehicle"},{"AddKeyword":{"Crew":3}}"#
)]
fn a_permanent_may_enter_as_a_copy_with_these_exceptions(
    #[case] text: &str,
    #[case] of: &str,
    #[case] except: &str,
) {
    reads_as(text, ClauseKind::Static, false, &enters(of, except));
}

#[test]
fn the_list_and_the_and_form_of_one_set_of_exceptions_are_the_same_reading() {
    let list = clause(
        "You may have this creature enter as a copy of any creature on the battlefield, except it isn't legendary, has haste, and is a Zombie in addition to its other types.",
        ClauseKind::Static,
        false,
    );
    let joined = clause(
        "You may have this creature enter as a copy of any creature on the battlefield, except it isn't legendary and it has haste and it's a Zombie in addition to its other types.",
        ClauseKind::Static,
        false,
    );
    assert!(list.is_some());
    assert_eq!(list, joined);
}

#[rstest]
#[case::a_part_without_a_verb(
    "You may have this creature enter as a copy of any creature on the battlefield, except it isn't legendary, is an artifact in addition to its other types, and myriad."
)]
#[case::a_list_of_a_missing_word(
    "You may have this creature enter as a copy of any creature on the battlefield, except it isn't legendary, and."
)]
#[case::a_type_with_a_size(
    "You may have this artifact enter as a copy of any creature on the battlefield, except it's a 2/2 artifact and it loses all other card types."
)]
#[case::keywords_without_a_subtype_ahead_of_the_loss(
    "You may have this artifact enter as a copy of any creature on the battlefield, except it's an artifact with flying and it loses all other card types."
)]
#[case::a_name(
    "You may have this creature enter as a copy of any creature on the battlefield, except its name is Hero."
)]
fn near_misses_of_the_exceptions_of_a_copy_stay_with_the_table(#[case] text: &str) {
    assert_eq!(clause(text, ClauseKind::Static, false), None, "{text}");
}

#[rstest]
#[case::power_after_the_control_words(
    "You may have this creature enter as a copy of a creature you control with power 4 or greater, except it's a Dragon in addition to its other types and it has flying."
)]
#[case::an_opponents_creature_alone(
    "You may have this creature enter as a copy of a creature an opponent controls."
)]
fn the_choice_of_what_to_copy_writes_back_as_the_card_words_it(#[case] text: &str) {
    golden(text, ClauseKind::Static, false);
}
