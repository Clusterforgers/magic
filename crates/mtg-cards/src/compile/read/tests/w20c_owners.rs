//! Goal: "each player gains control of all creatures they own" reads as a pass for each player that gives them, for good, what they own and writes back as the card words it, and a near-miss stays with the table.
use super::*;

#[rstest]
#[case::creatures_in_a_trigger(
    "At the beginning of your end step, each player gains control of all creatures they own.",
    ClauseKind::Triggered
)]
#[case::creatures_in_an_activation(
    "{T}: Each player gains control of all creatures they own.",
    ClauseKind::Activated
)]
#[case::artifacts_in_a_spell(
    "Each player gains control of all artifacts they own.",
    ClauseKind::Spell
)]
#[case::nontoken_creatures(
    "Each player gains control of all nontoken creatures they own.",
    ClauseKind::Spell
)]
fn w20c_each_player_takes_back_what_they_own(#[case] text: &str, #[case] kind: ClauseKind) {
    golden(text, kind, false);
}

#[test]
fn w20c_the_pass_reads_into_the_dsl() {
    reads_as(
        "{T}: Each player gains control of all creatures they own.",
        ClauseKind::Activated,
        false,
        r#"{"Activated":{"cost":["Tap"],"effects":[{"RepeatEach":{"over":"EachPlayer","as":"player","effects":[{"Continuous":{"affected":{"Each":[{"Type":"Creature"},{"OwnedBy":{"Bound":"player"}}]},"mods":[{"SetController":{"Bound":"player"}}],"duration":"Forever"}}]}}]}}"#,
    );
}

#[rstest]
#[case::controls_not_owns(
    "Each player gains control of all creatures they control.",
    ClauseKind::Spell
)]
#[case::an_opponent(
    "Each opponent gains control of all creatures they own.",
    ClauseKind::Spell
)]
#[case::you_not_each("You gain control of all creatures you own.", ClauseKind::Spell)]
#[case::for_the_turn(
    "Each player gains control of all creatures they own until end of turn.",
    ClauseKind::Spell
)]
#[case::one_target(
    "Each player gains control of target creature they own.",
    ClauseKind::Spell
)]
#[case::no_owner("Each player gains control of all creatures.", ClauseKind::Spell)]
fn w20c_an_owner_near_miss_stays_unread(#[case] text: &str, #[case] kind: ClauseKind) {
    assert_eq!(clause(text, kind, false), None, "{text}");
}
