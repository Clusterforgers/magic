//! Goal: a permanent that becomes a copy "except it has this ability" (alone, with a name or keywords, after "you may have") and a group that loses a keyword and can't have or gain it write back as the card's words; the copy without the ability, another keyword or another group never does.
use super::*;

/// Whether some wording the checker accepts for the ability is the clause.
fn g9b_reads(json: &str, clause: &str) -> bool {
    let ability: Ability = serde_json::from_str(json).expect("ability JSON");
    written_variants(&ability)
        .iter()
        .any(|wording| same_text(wording, clause))
}

const CRYPTOPLASM: &str = r#"{"Triggered":{"effects":[{"May":{"effects":[{"BecomeCopy":{"duration":"Forever","except":["ThisAbility"],"of":{"Target":0},"what":"This"}}]}}],"targets":[{"what":{"Object":[{"Type":"Creature"},"Other"]}}],"when":{"StepBegins":{"step":"Upkeep","whose":"You"}}}}"#;
const TRANSRELIQUAT: &str = r#"{"Activated":{"cost":"{1}{U}{R}","effects":[{"BecomeCopy":{"duration":"Forever","except":["ThisAbility"],"of":{"Target":0},"what":"This"}}],"targets":[{"what":{"Object":[{"Type":"Artifact"}]}}]}}"#;
const ARCHETYPE: &str = r#"{"Static":{"affected":{"Each":[{"Type":"Creature"},{"ControlledBy":"Opponents"}]},"mods":[{"LockKeyword":"FirstStrike"}]}}"#;

#[rstest]
#[case::transreliquat(
    TRANSRELIQUAT,
    "{1}{U}{R}: This artifact becomes a copy of target artifact, except it has this ability."
)]
#[case::cryptoplasm(
    CRYPTOPLASM,
    "At the beginning of your upkeep, you may have this creature become a copy of another target creature, except it has this ability."
)]
#[case::archetype(
    ARCHETYPE,
    "Creatures your opponents control lose first strike and can't have or gain first strike."
)]
fn the_copy_that_keeps_its_ability_and_the_locked_keyword_read_back(
    #[case] json: &str,
    #[case] clause: &str,
) {
    assert!(
        g9b_reads(json, clause),
        "{clause} is not among {:#?}",
        written_variants(&serde_json::from_str(json).expect("ability JSON"))
    );
}

#[rstest]
#[case::the_ability_dropped(
    &TRANSRELIQUAT.replace(r#""except":["ThisAbility"],"#, ""),
    "{1}{U}{R}: This artifact becomes a copy of target artifact, except it has this ability."
)]
#[case::the_ability_kept_unsaid(
    TRANSRELIQUAT,
    "{1}{U}{R}: This artifact becomes a copy of target artifact."
)]
#[case::without_may(
    r#"{"Triggered":{"effects":[{"BecomeCopy":{"duration":"Forever","except":["ThisAbility"],"of":{"Target":0},"what":"This"}}],"targets":[{"what":{"Object":[{"Type":"Creature"},"Other"]}}],"when":{"StepBegins":{"step":"Upkeep","whose":"You"}}}}"#,
    "At the beginning of your upkeep, you may have this creature become a copy of another target creature, except it has this ability."
)]
#[case::another_keyword(
    &ARCHETYPE.replace("FirstStrike", "Trample"),
    "Creatures your opponents control lose first strike and can't have or gain first strike."
)]
#[case::only_lost(
    &ARCHETYPE.replace("LockKeyword", "RemoveKeyword"),
    "Creatures your opponents control lose first strike and can't have or gain first strike."
)]
#[case::another_group(
    &ARCHETYPE.replace("Opponents", "You"),
    "Creatures your opponents control lose first strike and can't have or gain first strike."
)]
fn another_copy_or_keyword_differs(#[case] json: &str, #[case] clause: &str) {
    assert!(!g9b_reads(json, clause), "{clause} read back from {json}");
}
