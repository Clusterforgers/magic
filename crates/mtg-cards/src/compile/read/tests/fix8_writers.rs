//! Goal: fix pass 8 tightens the read-back writers so a rule that means something else reads Differs: "if you do" over the exile of a target allows at most one target.
use super::*;

fn written(json: &str, clause: &str) -> bool {
    written_variants(&ability(json))
        .iter()
        .any(|wording| same_text(wording, clause))
}

const IMP: &str = r#"{"Triggered":{"effects":[{"May":{"effects":[{"Exile":{"as":"exiled","what":{"Target":0}}},{"If":{"condition":{"Compare":{"left":{"Bound":"exiled~moved"},"op":"Eq","right":1}},"then":[{"GainLife":{"amount":2}}]}}]}}],"targets":[{"what":{"Object":[{"Type":"Creature"},{"In":"Graveyard"}]}}],"when":{"ZoneChange":{"to":"Battlefield","what":["This"]}}}}"#;
const IMP_TEXT: &str = "When this creature enters, you may exile target creature card from a graveyard. If you do, you gain 2 life.";

#[rstest]
#[case::one_target(IMP, true)]
// Exactly one of up to two exiled is not "if you do".
#[case::up_to_two(&IMP.replace(r#"{"In":"Graveyard"}]}}]"#, r#"{"In":"Graveyard"}]},"bounds":{"min":0,"max":2}}]"#), false)]
#[case::any_number(&IMP.replace(r#"{"In":"Graveyard"}]}}]"#, r#"{"In":"Graveyard"}]},"bounds":{"min":0,"max":null}}]"#), false)]
// A target index with no spec is not one thing taken.
#[case::unknown_target(&IMP.replace(r#""what":{"Target":0}"#, r#""what":{"Target":1}"#), false)]
fn fix8_if_you_do_after_an_exile_counts_one_target(#[case] json: &str, #[case] reads: bool) {
    assert_eq!(written(json, IMP_TEXT), reads, "{json}");
}

const DISCIPLE: &str = r#"{"Triggered":{"effects":[{"May":{"effects":[{"Sacrifice":{"as":"sacrificed","filter":[{"Type":"Creature"},"Other"]}},{"If":{"condition":{"Compare":{"left":{"Bound":"sacrificed~moved"},"op":"Ge","right":1}},"then":[{"GainLife":{"amount":{"Stat":{"of":{"Bound":"sacrificed"},"stat":"Power"}}}},{"Draw":{"count":{"Stat":{"of":{"Bound":"sacrificed"},"stat":"Power"}}}}]}}]}}],"when":{"ZoneChange":{"to":"Battlefield","what":["This"]}}}}"#;

#[rstest]
#[case::that_creature(
    "When this creature enters, you may sacrifice another creature. If you do, you gain X life and draw X cards, where X is that creature's power.",
    true
)]
// A sacrificed creature is not a permanent exiled this way.
#[case::exiled_noun(
    "When this creature enters, you may sacrifice another creature. If you do, you gain X life and draw X cards, where X is the power of the permanent exiled this way.",
    false
)]
fn fix8_a_sacrificed_creature_is_named_as_sacrificed(#[case] clause: &str, #[case] reads: bool) {
    assert_eq!(written(DISCIPLE, clause), reads);
}
