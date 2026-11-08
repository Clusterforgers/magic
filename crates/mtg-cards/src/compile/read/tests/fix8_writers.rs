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

const MOSEO: &str = r#"{"Triggered":{"effects":[{"Move":{"to":{"Battlefield":{}},"what":{"Target":0}}}],"if":{"Compare":{"left":{"Tallied":{"what":"LifeGainedTotal","who":"You"}},"op":"Ge","right":1}},"targets":[{"bounds":{"max":1,"min":0},"what":{"Object":[{"Type":"Creature"},{"Compare":{"op":"Le","stat":"ManaValue","to":{"Tallied":{"what":"LifeGainedTotal","who":"You"}}}},{"OwnedBy":"You"},{"In":"Graveyard"}]}}],"when":{"StepBegins":{"step":"End","whose":"You"}}}}"#;
const MOSEO_TAIL: &str = "return up to one target creature card with mana value X or less from your graveyard to the battlefield, where X is the amount of life you gained this turn.";

#[rstest]
#[case::gained_life("if you gained life this turn", true)]
// The life gained stands in for the hand only inside "where X is".
#[case::hand_count("if you have 1 or more cards in hand", false)]
#[case::hand_words("if you have one or more cards in hand", false)]
fn fix8_life_gained_is_not_read_as_cards_in_hand(#[case] condition: &str, #[case] reads: bool) {
    let clause = format!("At the beginning of your end step, {condition}, {MOSEO_TAIL}");
    assert_eq!(
        written(MOSEO, &clause),
        reads,
        "{:?}",
        written_variants(&ability(MOSEO))
    );
}

const TAP_OR_UNTAP: &str = r#"{"Activated":{"cost":[{"Mana":"{U}"},"Tap"],"effects":[{"TapOrUntap":{"what":{"Target":0}}}],"targets":[{"what":{"Object":[{"Type":"Creature"}]}}]}}"#;

#[rstest]
#[case::plain(TAP_OR_UNTAP, "{U}, {T}: Tap or untap target creature.", true)]
#[case::may(TAP_OR_UNTAP, "{U}, {T}: You may tap or untap target creature.", true)]
// A tap alone has a real "may": the rule without it reads Differs.
#[case::tap_may_dropped(&TAP_OR_UNTAP.replace("TapOrUntap", "Tap"), "{U}, {T}: You may tap target creature.", false)]
#[case::other_target(&TAP_OR_UNTAP.replace("Creature", "Artifact"), "{U}, {T}: You may tap or untap target creature.", false)]
fn fix8_a_tap_or_untap_may_be_said_with_may(
    #[case] json: &str,
    #[case] clause: &str,
    #[case] reads: bool,
) {
    assert_eq!(
        written(json, clause),
        reads,
        "{:?}",
        written_variants(&ability(json))
    );
}

const NONE_INSIDE: &str = r#"{"Triggered":{"effects":[{"May":{"effects":[{"Sacrifice":{"as":"sacrificed","filter":[{"Type":"Creature"}]}},{"If":{"condition":{"CountOf":{"op":"Eq","right":0,"selector":[{"Type":"Creature"}],"subject":{"Bound":"sacrificed"}}},"then":[{"RemoveCounters":{"counter":"+1/+1","from":"This"}}]}}]}}],"when":{"StepBegins":{"step":"Upkeep","whose":"You"}}}}"#;

#[rstest]
// "None taken" inside the "may" asks about an accepted step, not about a refusal.
#[case::none_is_not_dont(
    NONE_INSIDE,
    "At the beginning of your upkeep, you may sacrifice a creature. If you don't, remove a +1/+1 counter from ~.",
    false
)]
#[case::one_inside_is_do(&NONE_INSIDE.replace(r#""op":"Eq","right":0"#, r#""op":"Eq","right":1"#), "At the beginning of your upkeep, you may sacrifice a creature. If you do, remove a +1/+1 counter from ~.", true)]
fn fix8_only_if_you_do_leaves_the_may(
    #[case] json: &str,
    #[case] clause: &str,
    #[case] reads: bool,
) {
    assert_eq!(
        written(json, clause),
        reads,
        "{:?}",
        written_variants(&ability(json))
    );
}
