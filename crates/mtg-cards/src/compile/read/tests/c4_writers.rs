//! Goal: the writers of the gaps of the model-assisted reading give the oracle wording of a gain of control for as long as the player controls the source or the source stays, of a control taken from a group and from a target permanent, and a wrong duration, source, group, noun or mode never reads back.
use super::*;

/// Whether some wording the checker accepts for the rules is the clause.
fn written(json: &str, clause: &str) -> bool {
    written_variants(&ability(json))
        .iter()
        .any(|wording| same_text(wording, clause))
}

fn taking(target: &str, duration: &str) -> String {
    format!(
        r#"{{"Triggered":{{"when":{{"ZoneChange":{{"what":["This"],"to":"Battlefield"}}}},"targets":[{{"what":{{"Object":[{{"Type":"{target}"}}]}}}}],"effects":[{{"Continuous":{{"affected":{{"Target":0}},"mods":[{{"SetController":"You"}}],"duration":"{duration}"}}}}]}}}}"#
    )
}

#[rstest]
#[case::while_you_control_the_source(
    "Creature",
    "WhileYouControlSource",
    "When ~ enters, gain control of target creature for as long as you control ~."
)]
#[case::an_artifact(
    "Artifact",
    "WhileYouControlSource",
    "When ~ enters, gain control of target artifact for as long as you control ~."
)]
#[case::while_the_source_remains(
    "Creature",
    "WhileSourceOnBattlefield",
    "When ~ enters, gain control of target creature for as long as ~ remains on the battlefield."
)]
fn a_control_that_lasts_as_long_as_the_source_does_is_written_with_its_duration(
    #[case] target: &str,
    #[case] duration: &str,
    #[case] clause: &str,
) {
    assert!(written(&taking(target, duration), clause), "{clause}");
}

#[rstest]
#[case::the_other_duration_of_the_source(
    "When ~ enters, gain control of target creature for as long as ~ remains on the battlefield."
)]
#[case::until_end_of_turn("When ~ enters, gain control of target creature until end of turn.")]
#[case::another_kind(
    "When ~ enters, gain control of target artifact for as long as you control ~."
)]
#[case::for_good("When ~ enters, gain control of target creature.")]
#[case::an_opponent_controls(
    "When ~ enters, gain control of target creature for as long as an opponent controls ~."
)]
fn a_control_for_as_long_as_you_control_the_source_does_not_read_as_another_duration_or_kind(
    #[case] clause: &str,
) {
    assert!(!written(
        &taking("Creature", "WhileYouControlSource"),
        clause
    ));
}
