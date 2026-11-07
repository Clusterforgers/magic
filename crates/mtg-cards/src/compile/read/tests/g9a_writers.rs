//! Goal: block triggers about the other creature read back: "blocks a creature with flying" and "becomes blocked by an
//! artifact creature" for one trigger per pair, and "blocks or becomes blocked by one or more black creatures" once, as a
//! filter of the block itself; another side, kind, subject or count never reads back.
use super::*;

fn written(json: &str, clause: &str) -> bool {
    written_variants(&ability(json))
        .iter()
        .any(|wording| same_text(wording, clause))
}

const PUMP: &str = r#"[{"Continuous":{"affected":"This","duration":"UntilEndOfTurn","mods":[{"ModifyPT":{"power":2,"toughness":0}}]}}]"#;

fn trigger(when: &str) -> String {
    format!(r#"{{"Triggered":{{"when":{when},"effects":{PUMP}}}}}"#)
}

/// A block of one or more attackers of the kind.
fn blocks(kind: &str) -> String {
    format!(r#"{{"Blocks":{{"what":["This"],"blocking":[{kind}]}}}}"#)
}

/// Becoming blocked by one or more creatures of the kind, once.
fn blocked(what: &str, kind: &str) -> String {
    format!(r#"{{"BecomesBlocked":{{"what":[{what}],"by":[{kind}],"once":true}}}}"#)
}

fn both(kind: &str) -> String {
    format!(
        r#"{{"AnyOf":[{},{}]}}"#,
        blocks(kind),
        blocked(r#""This""#, kind)
    )
}

const BLACK: &str = r#"{"Type":"Creature"},{"Color":"Black"}"#;
const ARTIFACT_BLOCKER: &str =
    r#"{"BecomesBlocked":{"what":["This"],"by":[{"Type":"Creature"},{"Type":"Artifact"}]}}"#;
const FLYER_BLOCKED: &str =
    r#"{"BecomesBlocked":{"what":[{"Type":"Creature"},{"HasKeyword":"Flying"}],"by":["This"]}}"#;
const BOTH_ANY: &str =
    r#"{"AnyOf":[{"Blocks":{"what":["This"]}},{"BecomesBlocked":{"what":["This"]}}]}"#;
const CLAUSE: &str = "Whenever ~ blocks or becomes blocked by one or more black creatures, ~ gets +2/+0 until end of turn.";

#[rstest]
#[case::either_side(both(BLACK), CLAUSE)]
#[case::blocks_only(
    blocks(BLACK),
    "Whenever ~ blocks one or more black creatures, ~ gets +2/+0 until end of turn."
)]
#[case::blocked_only(
    blocked(r#""This""#, BLACK),
    "Whenever ~ becomes blocked by one or more black creatures, ~ gets +2/+0 until end of turn."
)]
#[case::equipped(
    blocked(r#"{"Is":"Attached"}"#, r#"{"Type":"Creature"},"Colorless""#),
    "Whenever equipped creature becomes blocked by one or more colorless creatures, ~ gets +2/+0 until end of turn."
)]
#[case::orcs(
    both(r#"{"Subtype":"Orc"}"#),
    "Whenever ~ blocks or becomes blocked by one or more Orcs, ~ gets +2/+0 until end of turn."
)]
#[case::per_blocker(
    ARTIFACT_BLOCKER.to_owned(),
    "Whenever ~ becomes blocked by an artifact creature, ~ gets +2/+0 until end of turn."
)]
#[case::per_attacker(
    FLYER_BLOCKED.to_owned(),
    "Whenever ~ blocks a creature with flying, ~ gets +2/+0 until end of turn."
)]
fn reads_back(#[case] when: String, #[case] clause: &str) {
    let json = trigger(&when);
    assert!(
        written(&json, clause),
        "{:?}",
        written_variants(&ability(&json))
    );
}

#[rstest]
#[case::other_color(both(r#"{"Type":"Creature"},{"Color":"Red"}"#), CLAUSE)]
#[case::one_side(blocks(BLACK), CLAUSE)]
#[case::other_side(
    blocked(r#""This""#, BLACK),
    "Whenever ~ blocks one or more black creatures, ~ gets +2/+0 until end of turn."
)]
#[case::no_filter(BOTH_ANY.to_owned(), CLAUSE)]
#[case::other_subject(
    blocked(r#"{"Is":"Attached"}"#, BLACK),
    "Whenever ~ becomes blocked by one or more black creatures, ~ gets +2/+0 until end of turn."
)]
#[case::any_blocker(
    r#"{"BecomesBlocked":{"what":["This"]}}"#.to_owned(),
    "Whenever ~ becomes blocked by an artifact creature, ~ gets +2/+0 until end of turn."
)]
#[case::other_blocker(
    ARTIFACT_BLOCKER.to_owned(),
    "Whenever ~ becomes blocked by a creature, ~ gets +2/+0 until end of turn."
)]
#[case::once_not_per_blocker(
    ARTIFACT_BLOCKER.to_owned(),
    "Whenever ~ becomes blocked by one or more artifact creatures, ~ gets +2/+0 until end of turn."
)]
#[case::per_blocker_not_once(
    blocked(r#""This""#, r#"{"Type":"Creature"},{"Type":"Artifact"}"#),
    "Whenever ~ becomes blocked by an artifact creature, ~ gets +2/+0 until end of turn."
)]
#[case::blocked_not_blocking(
    FLYER_BLOCKED.to_owned(),
    "Whenever ~ becomes blocked by a creature with flying, ~ gets +2/+0 until end of turn."
)]
fn differs(#[case] when: String, #[case] clause: &str) {
    assert!(!written(&trigger(&when), clause));
}
