//! Goal: block triggers about the other creature read back: "blocks a creature with flying" and "becomes blocked by an
//! artifact creature" for one trigger per pair, and "blocks or becomes blocked by one or more black creatures" with the
//! other creature checked as it triggers; another side, kind, subject, count or a condition rechecked as it resolves
//! never reads back.
use super::*;

fn written(json: &str, clause: &str) -> bool {
    written_variants(&ability(json))
        .iter()
        .any(|wording| same_text(wording, clause))
}

const PUMP: &str = r#"[{"Continuous":{"affected":"This","duration":"UntilEndOfTurn","mods":[{"ModifyPT":{"power":2,"toughness":0}}]}}]"#;

fn trigger(when: &str, condition: Option<&str>) -> String {
    let condition = condition.map_or(String::new(), |found| format!(r#","if":{found}"#));
    format!(r#"{{"Triggered":{{"when":{when},"effects":{PUMP}{condition}}}}}"#)
}

/// "One or more creatures of the selector block or are blocked by the subject", checked as the ability triggers.
fn some(kind: &str, who: &str, op: &str, right: u32) -> String {
    format!(
        r#"{{"AtTrigger":{{"Compare":{{"left":{{"Count":[{kind},{{"AnyOf":[{{"BlockedBy":"{who}"}},{{"Blocks":"{who}"}}]}}]}},"op":"{op}","right":{right}}}}}}}"#
    )
}

const BLACK: &str = r#"{"Type":"Creature"},{"Color":"Black"}"#;
const BOTH: &str =
    r#"{"AnyOf":[{"Blocks":{"what":["This"]}},{"BecomesBlocked":{"what":["This"]}}]}"#;
const BLOCKS: &str = r#"{"Blocks":{"what":["This"]}}"#;
const BLOCKED: &str = r#"{"BecomesBlocked":{"what":["This"]}}"#;
const EQUIPPED: &str = r#"{"BecomesBlocked":{"what":[{"Is":"Attached"}]}}"#;
const ARTIFACT_BLOCKER: &str =
    r#"{"BecomesBlocked":{"what":["This"],"by":[{"Type":"Creature"},{"Type":"Artifact"}]}}"#;
const FLYER_BLOCKED: &str =
    r#"{"BecomesBlocked":{"what":[{"Type":"Creature"},{"HasKeyword":"Flying"}],"by":["This"]}}"#;
const CLAUSE: &str = "Whenever ~ blocks or becomes blocked by one or more black creatures, ~ gets +2/+0 until end of turn.";

#[rstest]
#[case::either_side(BOTH, Some(some(BLACK, "This", "Ge", 1)), CLAUSE)]
#[case::blocks_only(
    BLOCKS,
    Some(some(BLACK, "This", "Ge", 1)),
    "Whenever ~ blocks one or more black creatures, ~ gets +2/+0 until end of turn."
)]
#[case::blocked_only(
    BLOCKED,
    Some(some(BLACK, "This", "Ge", 1)),
    "Whenever ~ becomes blocked by one or more black creatures, ~ gets +2/+0 until end of turn."
)]
#[case::equipped(
    EQUIPPED,
    Some(some(r#"{"Type":"Creature"},"Colorless""#, "Attached", "Ge", 1)),
    "Whenever equipped creature becomes blocked by one or more colorless creatures, ~ gets +2/+0 until end of turn."
)]
#[case::orcs(
    BOTH,
    Some(some(r#"{"Subtype":"Orc"}"#, "This", "Ge", 1)),
    "Whenever ~ blocks or becomes blocked by one or more Orcs, ~ gets +2/+0 until end of turn."
)]
#[case::per_blocker(
    ARTIFACT_BLOCKER,
    None,
    "Whenever ~ becomes blocked by an artifact creature, ~ gets +2/+0 until end of turn."
)]
#[case::per_attacker(
    FLYER_BLOCKED,
    None,
    "Whenever ~ blocks a creature with flying, ~ gets +2/+0 until end of turn."
)]
fn reads_back(#[case] when: &str, #[case] condition: Option<String>, #[case] clause: &str) {
    let json = trigger(when, condition.as_deref());
    assert!(
        written(&json, clause),
        "{:?}",
        written_variants(&ability(&json))
    );
}

#[rstest]
#[case::other_color(
    BOTH,
    Some(some(r#"{"Type":"Creature"},{"Color":"Red"}"#, "This", "Ge", 1)),
    CLAUSE
)]
#[case::one_side(BLOCKS, Some(some(BLACK, "This", "Ge", 1)), CLAUSE)]
#[case::other_side(
    BLOCKED,
    Some(some(BLACK, "This", "Ge", 1)),
    "Whenever ~ blocks one or more black creatures, ~ gets +2/+0 until end of turn."
)]
#[case::two_or_more(BOTH, Some(some(BLACK, "This", "Ge", 2)), CLAUSE)]
#[case::no_filter(BOTH, None, CLAUSE)]
#[case::rechecked(
    BOTH,
    Some(some(BLACK, "This", "Ge", 1).replacen(r#"{"AtTrigger":"#, "", 1).strip_suffix('}').map(str::to_owned).unwrap_or_default()),
    CLAUSE
)]
#[case::other_subject(
    EQUIPPED,
    Some(some(BLACK, "This", "Ge", 1)),
    "Whenever equipped creature becomes blocked by one or more black creatures, ~ gets +2/+0 until end of turn."
)]
#[case::any_blocker(
    BLOCKED,
    None,
    "Whenever ~ becomes blocked by an artifact creature, ~ gets +2/+0 until end of turn."
)]
#[case::other_blocker(
    ARTIFACT_BLOCKER,
    None,
    "Whenever ~ becomes blocked by a creature, ~ gets +2/+0 until end of turn."
)]
#[case::once_not_per_blocker(
    ARTIFACT_BLOCKER,
    None,
    "Whenever ~ becomes blocked by one or more artifact creatures, ~ gets +2/+0 until end of turn."
)]
#[case::blocked_not_blocking(
    FLYER_BLOCKED,
    None,
    "Whenever ~ becomes blocked by a creature with flying, ~ gets +2/+0 until end of turn."
)]
fn differs(#[case] when: &str, #[case] condition: Option<String>, #[case] clause: &str) {
    assert!(!written(&trigger(when, condition.as_deref()), clause));
}
