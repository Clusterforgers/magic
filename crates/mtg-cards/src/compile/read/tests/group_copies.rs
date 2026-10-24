//! Goal: a group that becomes a copy of one target (Mirrorweave, Nanogene Conversion, with a sentence of its own that chooses the target or not) and a permanent that becomes a copy with no end or with exceptions (The Mycosynth Gardens) read into `BecomeCopy`, are written back as the card's text, and every near-miss stays with the table.
use super::*;

const OTHERS: &str = r#"{"Each":[{"Type":"Creature"},{"Not":{"Is":{"Target":0}}}]}"#;

#[rstest]
#[case::in_one_sentence(
    "Each other creature becomes a copy of target nonlegendary creature until end of turn.",
    r#"[{"Type":"Creature"},{"Not":{"Supertype":"Legendary"}}]"#,
    ""
)]
#[case::the_target_chosen_first(
    "Choose target creature you control. Each other creature becomes a copy of that creature until end of turn.",
    r#"[{"Type":"Creature"},{"ControlledBy":"You"}]"#,
    ""
)]
#[case::with_an_exception(
    "Choose target creature you control. Each other creature becomes a copy of that creature until end of turn, except it isn't legendary.",
    r#"[{"Type":"Creature"},{"ControlledBy":"You"}]"#,
    r#","except":[{"RemoveSupertype":"Legendary"}]"#
)]
#[case::the_target_named_in_place_with_an_exception(
    "Each other creature becomes a copy of target creature you control until end of turn, except it isn't legendary.",
    r#"[{"Type":"Creature"},{"ControlledBy":"You"}]"#,
    r#","except":[{"RemoveSupertype":"Legendary"}]"#
)]
fn a_group_becomes_a_copy_of_the_target(
    #[case] text: &str,
    #[case] target: &str,
    #[case] except: &str,
) {
    reads_as(
        text,
        ClauseKind::Spell,
        false,
        &format!(
            r#"{{"Spell":{{"targets":[{{"what":{{"Object":{target}}}}}],"effects":[{{"BecomeCopy":{{"what":{OTHERS},"of":{{"Target":0}}{except},"duration":"UntilEndOfTurn"}}}}]}}}}"#
        ),
    );
}

#[rstest]
#[case::a_permanent("Each other creature becomes a copy of that creature until end of turn.")]
#[case::another_noun(
    "Choose target creature you control. Each other creature becomes a copy of that permanent until end of turn."
)]
#[case::no_end(
    "Choose target creature you control. Each other creature becomes a copy of that creature."
)]
#[case::every_creature("Each creature becomes a copy of target creature until end of turn.")]
#[case::all_of_them("All other creatures become copies of target creature until end of turn.")]
#[case::two_targets(
    "Choose target creature you control. Each other creature becomes a copy of target creature until end of turn."
)]
fn a_group_copy_that_is_not_the_shape_stays_unread(#[case] text: &str) {
    assert_eq!(spell(text), None, "{text}");
}

#[rstest]
#[case::the_land_itself(
    "{X}, {T}: This land becomes a copy of target nontoken artifact you control with mana value X.",
    r#"{"Activated":{"cost":[{"Mana":"{X}"},"Tap"],"targets":[{"what":{"Object":[{"Type":"Artifact"},{"Not":"Token"},{"ControlledBy":"You"},{"Compare":{"stat":"ManaValue","op":"Eq","to":"X"}}]}}],"effects":[{"BecomeCopy":{"what":"This","of":{"Target":0},"duration":"Forever"}}]}}"#
)]
fn a_permanent_becomes_a_copy_with_no_end(#[case] text: &str, #[case] json: &str) {
    reads_as(text, ClauseKind::Activated, false, json);
}
