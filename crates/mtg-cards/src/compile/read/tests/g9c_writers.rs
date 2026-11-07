//! Goal: a block of a kind and a block by a kind ("blocks one or more black creatures", "becomes blocked by an artifact
//! creature", "becomes blocked by one or more Orcs", "blocks or becomes blocked") are written in their Oracle wordings,
//! and a trigger of another kind, another count or another side does not read back.
use rstest::rstest;

use super::*;

fn written(json: &str, clause: &str) -> bool {
    written_variants(&ability(json))
        .iter()
        .any(|wording| same_text(wording, clause))
}

const PUMP: &str = r#"[{"Continuous":{"affected":"This","duration":"UntilEndOfTurn","mods":[{"ModifyPT":{"power":3,"toughness":3}}]}}]"#;

fn on(when: &str) -> String {
    format!(r#"{{"Triggered":{{"when":{when},"effects":{PUMP}}}}}"#)
}

const BLACK: &str = r#"[{"Type":"Creature"},{"Color":"Black"}]"#;
const ARTIFACT: &str = r#"[{"Type":"Creature"},{"Type":"Artifact"}]"#;
const ORCS: &str = r#"[{"Subtype":"Orc"}]"#;

fn blocks(kind: &str) -> String {
    format!(r#"{{"Blocks":{{"what":["This"],"blocking":{kind}}}}}"#)
}

fn blocked(kind: &str, once: bool) -> String {
    format!(r#"{{"BecomesBlocked":{{"what":["This"],"by":{kind},"once":{once}}}}}"#)
}

fn either(first: &str, second: &str) -> String {
    format!(r#"{{"AnyOf":[{first},{second}]}}"#)
}

#[rstest]
#[case::blocks_black(
    on(&blocks(BLACK)),
    "Whenever this creature blocks one or more black creatures, this creature gets +3/+3 until end of turn.",
    true
)]
#[case::blocks_any_is_other(
    on(r#"{"Blocks":{"what":["This"]}}"#),
    "Whenever this creature blocks one or more black creatures, this creature gets +3/+3 until end of turn.",
    false
)]
#[case::blocks_other_kind(
    on(&blocks(ORCS)),
    "Whenever this creature blocks one or more black creatures, this creature gets +3/+3 until end of turn.",
    false
)]
#[case::blocked_by_an_artifact(
    on(&blocked(ARTIFACT, false)),
    "Whenever this creature becomes blocked by an artifact creature, this creature gets +3/+3 until end of turn.",
    true
)]
#[case::blocked_once_is_not_each(
    on(&blocked(ARTIFACT, true)),
    "Whenever this creature becomes blocked by an artifact creature, this creature gets +3/+3 until end of turn.",
    false
)]
#[case::blocked_by_one_or_more(
    on(&blocked(ORCS, true)),
    "Whenever this creature becomes blocked by one or more Orcs, this creature gets +3/+3 until end of turn.",
    true
)]
#[case::blocked_each_is_not_once(
    on(&blocked(ORCS, false)),
    "Whenever this creature becomes blocked by one or more Orcs, this creature gets +3/+3 until end of turn.",
    false
)]
#[case::blocked_by_any_is_other(
    on(r#"{"BecomesBlocked":{"what":["This"]}}"#),
    "Whenever this creature becomes blocked by one or more Orcs, this creature gets +3/+3 until end of turn.",
    false
)]
#[case::shared_kind(
    on(&either(&blocked(ORCS, true), &blocks(ORCS))),
    "Whenever this creature blocks or becomes blocked by one or more Orcs, this creature gets +3/+3 until end of turn.",
    true
)]
#[case::shared_kind_other_side(
    on(&either(&blocked(ORCS, true), &blocks(BLACK))),
    "Whenever this creature blocks or becomes blocked by one or more Orcs, this creature gets +3/+3 until end of turn.",
    false
)]
#[case::shared_kind_block_of_any(
    on(&either(&blocked(ORCS, true), r#"{"Blocks":{"what":["This"]}}"#)),
    "Whenever this creature blocks or becomes blocked by one or more Orcs, this creature gets +3/+3 until end of turn.",
    false
)]
#[case::blocks_or_blocked(
    on(&either(r#"{"BecomesBlocked":{"what":["This"]}}"#, r#"{"Blocks":{"what":["This"]}}"#)),
    "Whenever this creature blocks or becomes blocked, this creature gets +3/+3 until end of turn.",
    true
)]
fn a_block_of_a_kind_reads_back(#[case] json: String, #[case] clause: &str, #[case] reads: bool) {
    assert_eq!(
        written(&json, clause),
        reads,
        "{:?}",
        written_variants(&ability(&json))
    );
}
fn shadow(quirk: &str, who: &str) -> String {
    format!(r#"{{"Rule":{{"rule":{{"Combat":{{"quirk":"{quirk}","who":{who}}}}}}}}}"#)
}

#[rstest]
#[case::as_though_it_had(
    shadow("BlockAsThoughShadow", r#""This""#),
    "This creature can block creatures with shadow as though it had shadow.",
    true
)]
#[case::as_though_they_had_none(
    shadow("BlockIgnoringShadow", r#""This""#),
    "This creature can block creatures with shadow as though they didn't have shadow.",
    true
)]
#[case::other_quirk(
    shadow("BlockAsThoughShadow", r#""This""#),
    "This creature can block creatures with shadow as though they didn't have shadow.",
    false
)]
#[case::other_creature(
    shadow("BlockAsThoughShadow", r#""Attached""#),
    "This creature can block creatures with shadow as though it had shadow.",
    false
)]
fn a_block_of_shadow_reads_back(#[case] json: String, #[case] clause: &str, #[case] reads: bool) {
    assert_eq!(
        written(&json, clause),
        reads,
        "{:?}",
        written_variants(&ability(&json))
    );
}
