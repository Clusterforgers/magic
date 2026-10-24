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

#[rstest]
#[case::clone_legion(
    "For each creature target player controls, create a token that's a copy of that creature.",
    r#"{"Spell":{"targets":[{"what":"Player"}],"effects":[{"CreateToken":{"token":{"CopyOf":{"of":{"Each":[{"Type":"Creature"},{"ControlledBy":{"Target":0}}]}}}}}]}}"#
)]
#[case::artifacts(
    "For each artifact target player controls, create a token that's a copy of that artifact.",
    r#"{"Spell":{"targets":[{"what":"Player"}],"effects":[{"CreateToken":{"token":{"CopyOf":{"of":{"Each":[{"Type":"Artifact"},{"ControlledBy":{"Target":0}}]}}}}}]}}"#
)]
#[case::rite_of_replication(
    "Create a token that's a copy of target creature. If this spell was kicked, create five of those tokens instead.",
    r#"{"Spell":{"targets":[{"what":{"Object":[{"Type":"Creature"}]}}],"effects":[{"If":{"condition":"Kicked","then":[{"CreateToken":{"token":{"CopyOf":{"of":{"Target":0}}},"count":5}}],"otherwise":[{"CreateToken":{"token":{"CopyOf":{"of":{"Target":0}}}}}]}}]}}"#
)]
#[case::three_tokens(
    "Create a token that's a copy of target artifact. If this spell was kicked, create three of those tokens instead.",
    r#"{"Spell":{"targets":[{"what":{"Object":[{"Type":"Artifact"}]}}],"effects":[{"If":{"condition":"Kicked","then":[{"CreateToken":{"token":{"CopyOf":{"of":{"Target":0}}},"count":3}}],"otherwise":[{"CreateToken":{"token":{"CopyOf":{"of":{"Target":0}}}}}]}}]}}"#
)]
fn tokens_are_made_as_copies_for_each_permanent_of_a_target_player_or_more_when_kicked(
    #[case] text: &str,
    #[case] json: &str,
) {
    reads_as(text, ClauseKind::Spell, false, json);
}

#[rstest]
#[case::body_double(
    "You may have this creature enter as a copy of any creature card in a graveyard.",
    r#"{"Replacement":{"when":{"Enters":{"what":["This"]}},"by":{"AsCopy":{"of":[{"Type":"Creature"},{"In":"Graveyard"}]}}}}"#
)]
#[case::vesuva(
    "You may have this land enter tapped as a copy of any land on the battlefield.",
    r#"{"Replacement":{"when":{"Enters":{"what":["This"]}},"by":{"AsCopy":{"of":[{"Type":"Land"}],"tapped":true}}}}"#
)]
#[case::cursed_mirror(
    "As this artifact enters, you may have it become a copy of any creature on the battlefield until end of turn, except it has haste.",
    r#"{"Replacement":{"when":{"Enters":{"what":["This"]}},"by":{"AsCopy":{"of":[{"Type":"Creature"}],"except":[{"AddKeyword":"Haste"}],"until_end_of_turn":true}}}}"#
)]
#[case::an_artifact_card(
    "You may have this creature enter as a copy of any artifact card in a graveyard, except it's a Golem in addition to its other types.",
    r#"{"Replacement":{"when":{"Enters":{"what":["This"]}},"by":{"AsCopy":{"of":[{"Type":"Artifact"},{"In":"Graveyard"}],"except":[{"AddSubtype":"Golem"}]}}}}"#
)]
fn a_permanent_enters_as_a_copy_of_a_card_tapped_or_for_a_turn(
    #[case] text: &str,
    #[case] json: &str,
) {
    reads_as(text, ClauseKind::Static, false, json);
}

#[rstest]
#[case::tapped_until_end_of_turn(
    "As this artifact enters, you may have it become a copy of any creature on the battlefield until end of turn, except it has haste and enters tapped."
)]
#[case::no_end(
    "As this artifact enters, you may have it become a copy of any creature on the battlefield."
)]
#[case::a_graveyard_of_your_own(
    "You may have this creature enter as a copy of any creature card in your graveyard."
)]
#[case::an_opponent_graveyard(
    "You may have this creature enter as a copy of any creature card in an opponent's graveyard."
)]
fn an_entering_copy_that_is_not_the_shape_stays_unread(#[case] text: &str) {
    assert_eq!(clause(text, ClauseKind::Static, false), None, "{text}");
}

#[rstest]
#[case::one_more(
    "Create a token that's a copy of target creature. If this spell was kicked, create one of those tokens instead."
)]
#[case::not_kicked(
    "Create a token that's a copy of target creature. If this spell wasn't kicked, create five of those tokens instead."
)]
#[case::other_tokens(
    "Create a token that's a copy of target creature. If this spell was kicked, create five of those creatures instead."
)]
#[case::no_player("For each creature, create a token that's a copy of that creature.")]
#[case::another_noun(
    "For each creature target player controls, create a token that's a copy of that permanent."
)]
fn a_copy_wording_that_is_not_the_shape_stays_unread(#[case] text: &str) {
    assert_eq!(spell(text), None, "{text}");
}
