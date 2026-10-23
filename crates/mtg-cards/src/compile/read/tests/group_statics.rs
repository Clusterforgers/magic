//! Goal: the statics about groups read into the shapes the engine runs and write back as the same text: permanents that enter tapped, a group picked by a counter on it, granted ward, skulk and horsemanship, and the skipped draw step.
use super::*;

fn stat(text: &str) -> Option<Ability> {
    clause(text, ClauseKind::Static, false)
}

#[rstest]
#[case::opponents_creatures(
    "Creatures your opponents control enter tapped.",
    r#"{"Replacement":{"when":{"Enters":{"what":[{"Type":"Creature"},{"ControlledBy":"Opponents"}]}},"by":"Tapped"}}"#
)]
#[case::artifacts_and_creatures(
    "Artifacts and creatures your opponents control enter tapped.",
    r#"{"Replacement":{"when":{"Enters":{"what":[{"AnyOf":[{"Type":"Artifact"},{"Type":"Creature"}]},{"ControlledBy":"Opponents"}]}},"by":"Tapped"}}"#
)]
#[case::artifacts_alone(
    "Artifacts your opponents control enter tapped.",
    r#"{"Replacement":{"when":{"Enters":{"what":[{"Type":"Artifact"},{"ControlledBy":"Opponents"}]}},"by":"Tapped"}}"#
)]
#[case::your_own(
    "Creatures you control enter tapped.",
    r#"{"Replacement":{"when":{"Enters":{"what":[{"Type":"Creature"},{"ControlledBy":"You"}]}},"by":"Tapped"}}"#
)]
fn a_group_enters_tapped(#[case] text: &str, #[case] json: &str) {
    reads_as(text, ClauseKind::Static, false, json);
}

#[rstest]
#[case::trample(
    "Each creature you control with a +1/+1 counter on it has trample.",
    r#"{"Static":{"affected":{"Each":[{"Type":"Creature"},{"ControlledBy":"You"},{"HasCounter":"+1/+1"}]},"mods":[{"AddKeyword":"Trample"}]}}"#
)]
#[case::another(
    "Each other creature you control with a +1/+1 counter on it has deathtouch.",
    r#"{"Static":{"affected":{"Each":[{"Type":"Creature"},"Other",{"ControlledBy":"You"},{"HasCounter":"+1/+1"}]},"mods":[{"AddKeyword":"Deathtouch"}]}}"#
)]
#[case::a_named_counter(
    "Each creature you control with a quest counter on it gets +1/+1.",
    r#"{"Static":{"affected":{"Each":[{"Type":"Creature"},{"ControlledBy":"You"},{"HasCounter":{"Other":"quest"}}]},"mods":[{"ModifyPT":{"power":1,"toughness":1}}]}}"#
)]
#[case::each_creature(
    "Each creature you control has haste.",
    r#"{"Static":{"affected":{"Each":[{"Type":"Creature"},{"ControlledBy":"You"}]},"mods":[{"AddKeyword":"Haste"}]}}"#
)]
fn a_group_with_a_counter_is_given_a_keyword(#[case] text: &str, #[case] json: &str) {
    reads_as(text, ClauseKind::Static, false, json);
}

#[rstest]
#[case::ward_one(
    "Other creatures you control have ward {1}.",
    r#"{"Static":{"affected":{"Each":[{"Type":"Creature"},"Other",{"ControlledBy":"You"}]},"mods":[{"AddKeyword":{"Ward":"{1}"}}]}}"#
)]
#[case::ward_two_colours(
    "Creatures you control have ward {2}{U}.",
    r#"{"Static":{"affected":{"Each":[{"Type":"Creature"},{"ControlledBy":"You"}]},"mods":[{"AddKeyword":{"Ward":"{2}{U}"}}]}}"#
)]
#[case::equipped_ward(
    "Equipped creature gets +1/+1 and has ward {2}.",
    r#"{"Static":{"affected":"Attached","mods":[{"ModifyPT":{"power":1,"toughness":1}},{"AddKeyword":{"Ward":"{2}"}}]}}"#
)]
#[case::skulk(
    "Creatures you control have skulk.",
    r#"{"Static":{"affected":{"Each":[{"Type":"Creature"},{"ControlledBy":"You"}]},"mods":[{"AddKeyword":"Skulk"}]}}"#
)]
#[case::horsemanship_for_a_subtype(
    "Other Knights you control have horsemanship.",
    r#"{"Static":{"affected":{"Each":[{"Subtype":"Knight"},"Other",{"ControlledBy":"You"}]},"mods":[{"AddKeyword":"Horsemanship"}]}}"#
)]
#[case::horsemanship_for_all(
    "Creatures you control have horsemanship.",
    r#"{"Static":{"affected":{"Each":[{"Type":"Creature"},{"ControlledBy":"You"}]},"mods":[{"AddKeyword":"Horsemanship"}]}}"#
)]
fn ward_and_skulk_are_granted(#[case] text: &str, #[case] json: &str) {
    reads_as(text, ClauseKind::Static, false, json);
}

#[test]
fn ward_is_granted_for_a_turn() {
    reads_as(
        "Target creature gains ward {2} until end of turn.",
        ClauseKind::Spell,
        false,
        r#"{"Spell":{"targets":[{"what":{"Object":[{"Type":"Creature"}]}}],"effects":[{"Continuous":{"affected":{"Target":0},"mods":[{"AddKeyword":{"Ward":"{2}"}}],"duration":"UntilEndOfTurn"}}]}}"#,
    );
}

#[test]
fn the_draw_step_is_skipped() {
    reads_as(
        "Skip your draw step.",
        ClauseKind::Static,
        false,
        r#"{"Rule":{"rule":{"SkipDrawStep":"You"}}}"#,
    );
}

#[rstest]
#[case::ward_pays_life("Creatures you control have ward\u{2014}Pay 2 life.")]
#[case::ward_without_a_cost("Creatures you control have ward.")]
#[case::ward_with_x("Creatures you control have ward {X}.")]
#[case::skip_another_players("Skip target player's draw step.")]
#[case::skip_another_step("Skip your untap step.")]
#[case::a_counter_of_any_kind("Each creature you control with a counter on it has trample.")]
#[case::counters_it_does_not_have("Each creature you control with a +1/+1 counter has trample.")]
fn near_misses_stay_with_the_table(#[case] text: &str) {
    assert_eq!(stat(text), None, "{text}");
}
