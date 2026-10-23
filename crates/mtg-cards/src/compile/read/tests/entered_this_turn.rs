//! Goal: the permanents that entered the battlefield this turn ("creature token that entered the battlefield this turn", "creatures that entered this turn") read into `Pred::EnteredThisTurn` as a target and as a group, a single token is named as a target, and wordings about another turn or another controller stay unread.
use super::*;

#[rstest]
#[case::trigger_octomancer(
    "At the beginning of each end step, create a token that's a copy of target creature token that entered the battlefield this turn.",
    ClauseKind::Triggered,
    r#"{"Triggered":{"when":{"StepBegins":{"step":"End"}},"targets":[{"what":{"Object":[{"Type":"Creature"},"Token","EnteredThisTurn"]}}],"effects":[{"CreateToken":{"token":{"CopyOf":{"of":{"Target":0}}}}}]}}"#
)]
#[case::tapped_copy_romana(
    "{T}: Create a tapped token that's a copy of target token that entered this turn.",
    ClauseKind::Activated,
    r#"{"Activated":{"cost":["Tap"],"targets":[{"what":{"Object":["Token","EnteredThisTurn"]}}],"effects":[{"CreateToken":{"token":{"CopyOf":{"of":{"Target":0}}},"tapped":true}}]}}"#
)]
#[case::a_token_you_control(
    "Create a token that's a copy of target creature token you control.",
    ClauseKind::Spell,
    r#"{"Spell":{"targets":[{"what":{"Object":[{"Type":"Creature"},"Token",{"ControlledBy":"You"}]}}],"effects":[{"CreateToken":{"token":{"CopyOf":{"of":{"Target":0}}}}}]}}"#
)]
#[case::counter_on_one(
    "Put a +1/+1 counter on target creature that entered this turn.",
    ClauseKind::Spell,
    r#"{"Spell":{"targets":[{"what":{"Object":[{"Type":"Creature"},"EnteredThisTurn"]}}],"effects":[{"PutCounters":{"on":{"Target":0},"counter":"+1/+1"}}]}}"#
)]
#[case::counters_on_each(
    "Put a +1/+1 counter on each creature that entered this turn.",
    ClauseKind::Spell,
    r#"{"Spell":{"effects":[{"PutCounters":{"on":{"Each":[{"Type":"Creature"},"EnteredThisTurn"]},"counter":"+1/+1"}}]}}"#
)]
fn the_permanents_that_entered_this_turn_read_and_write_back(
    #[case] text: &str,
    #[case] kind: ClauseKind,
    #[case] written: &str,
) {
    reads_as(text, kind, false, written);
}

#[rstest]
#[case::last_turn(
    "Put a +1/+1 counter on target creature that entered last turn.",
    ClauseKind::Spell
)]
#[case::under_your_control(
    "Put a +1/+1 counter on target creature that entered the battlefield under your control this turn.",
    ClauseKind::Spell
)]
#[case::a_plural_token_as_one_target(
    "Create a token that's a copy of target creature tokens.",
    ClauseKind::Spell
)]
#[case::two_tapped_copies(
    "Create two tapped tokens that are copies of target token.",
    ClauseKind::Spell
)]
fn other_wordings_about_what_entered_stay_unread(#[case] text: &str, #[case] kind: ClauseKind) {
    assert_eq!(clause(text, kind, false), None, "{text}");
}
