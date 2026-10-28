//! Goal: phase-out sentences ("target creature phases out", "any number of target creatures you control phase out", "all permanents you control phase out") and the protection of Teferi's Protection, alone and after a promised gift, read into `Effect::PhaseOut` and two `Effect::Rule`s and write back as the same text; a wrong verb number, another duration or half the protection stays unread.
use super::*;

const PROTECTED: &str = r#"{"Rule":{"rule":{"LifeTotalCantChange":"You"},"duration":"UntilYourNextTurn"}},{"Rule":{"rule":{"PlayersHave":{"who":"You","keyword":{"Protection":[]}}},"duration":"UntilYourNextTurn"}}"#;

#[rstest]
#[case::one_target(
    "Target creature phases out.",
    r#"{"Spell":{"targets":[{"what":{"Object":[{"Type":"Creature"}]}}],"effects":[{"PhaseOut":{"what":{"Target":0}}}]}}"#.to_owned()
)]
#[case::any_permanent(
    "Target permanent phases out.",
    r#"{"Spell":{"targets":[{"what":{"Object":[]}}],"effects":[{"PhaseOut":{"what":{"Target":0}}}]}}"#.to_owned()
)]
#[case::any_number_of_targets(
    "Any number of target creatures you control phase out.",
    r#"{"Spell":{"targets":[{"what":{"Object":[{"Type":"Creature"},{"ControlledBy":"You"}]},"bounds":{"min":0,"max":null}}],"effects":[{"PhaseOut":{"what":{"Target":0}}}]}}"#.to_owned()
)]
#[case::a_group(
    "All permanents you control phase out.",
    r#"{"Spell":{"effects":[{"PhaseOut":{"what":{"Each":[{"ControlledBy":"You"}]}}}]}}"#.to_owned()
)]
#[case::protected(
    "Until your next turn, your life total can't change and you gain protection from everything.",
    format!(r#"{{"Spell":{{"effects":[{PROTECTED}]}}}}"#)
)]
#[case::phased_and_protected(
    "All permanents you control phase out, and until your next turn, your life total can't change and you gain protection from everything.",
    format!(r#"{{"Spell":{{"effects":[{{"PhaseOut":{{"what":{{"Each":[{{"ControlledBy":"You"}}]}}}}}},{PROTECTED}]}}}}"#)
)]
#[case::after_a_promised_gift(
    "If the gift was promised, all permanents you control phase out, and until your next turn, your life total can't change and you gain protection from everything.",
    format!(r#"{{"Spell":{{"effects":[{{"If":{{"condition":"AdditionalCostPaid","then":[{{"PhaseOut":{{"what":{{"Each":[{{"ControlledBy":"You"}}]}}}}}},{PROTECTED}]}}}}]}}}}"#)
)]
fn phase_out_sentences_read_and_write_back(#[case] text: &str, #[case] written: String) {
    reads_as(text, ClauseKind::Spell, false, &written);
}

#[rstest]
#[case::plural_verb_for_one("Target creature phase out.")]
#[case::singular_verb_for_a_group("All permanents you control phases out.")]
#[case::half_the_protection("Until your next turn, your life total can't change.")]
#[case::another_duration(
    "Until end of turn, your life total can't change and you gain protection from everything."
)]
#[case::another_player(
    "Until your next turn, their life total can't change and they gain protection from everything."
)]
fn near_misses_stay_unread(#[case] text: &str) {
    assert_eq!(spell(text), None, "{text}");
}
