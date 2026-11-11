//! Goal: the writers of ep26 write the card's words and nothing near them: a target "chosen at random" ("deals 2 damage to any target chosen at random", "deals 2 damage to target opponent chosen at random") reads back only when the rules draw it, and a target the controller chooses does not read as one drawn at random. "Exile a creature you control" as a cost reads back and is read by the compiler; a graveyard, another type, another count or a sacrifice does not.
use super::*;

fn activated(target: &str, effect: &str) -> Ability {
    ability(&format!(
        r#"{{"Activated":{{"cost":["Tap"],"targets":[{target}],"effects":[{effect}]}}}}"#
    ))
}

const DAMAGE: &str = r#"{"DealDamage":{"amount":2,"to":{"Target":0}}}"#;
const RANDOM_ANY: &str = r#"{"what":{"AtRandom":"AnyTarget"}}"#;
const ANY: &str = r#"{"what":"AnyTarget"}"#;
const RANDOM_FOE: &str = r#"{"what":{"AtRandom":"Opponent"}}"#;
const FOE: &str = r#"{"what":"Opponent"}"#;
const PILOT: &str = "{T}: ~ deals 2 damage to any target chosen at random.";
const RAMPAGER: &str = "{T}: ~ deals 2 damage to target opponent chosen at random.";

#[rstest]
#[case::pilot(activated(RANDOM_ANY, DAMAGE), PILOT, true)]
#[case::pilot_choosing(activated(ANY, DAMAGE), PILOT, false)]
#[case::pilot_plain_text(
    activated(RANDOM_ANY, DAMAGE),
    "{T}: ~ deals 2 damage to any target.",
    false
)]
#[case::opponent(activated(RANDOM_FOE, DAMAGE), RAMPAGER, true)]
#[case::opponent_choosing(activated(FOE, DAMAGE), RAMPAGER, false)]
#[case::any_target_for_an_opponent(activated(RANDOM_ANY, DAMAGE), RAMPAGER, false)]
fn ep26_targets_chosen_at_random_write_as_printed(
    #[case] ability: Ability,
    #[case] text: &str,
    #[case] reads: bool,
) {
    let found = written_variants(&ability);
    assert_eq!(found.iter().any(|w| same_text(w, text)), reads, "{found:?}");
}

fn draws_for(cost: &str) -> Ability {
    ability(&format!(
        r#"{{"Activated":{{"cost":[{{"Mana":"{{B}}"}},{cost}],"effects":[{{"Draw":{{}}}}]}}}}"#
    ))
}

const EXILE_CREATURE: &str = r#"{"ExilePermanents":{"filter":[{"Type":"Creature"}]}}"#;
const FUMES: &str = "{B}, Exile a creature you control: Draw a card.";

#[rstest]
#[case::a_creature_you_control(draws_for(EXILE_CREATURE), FUMES, true)]
#[case::from_the_graveyard(
    draws_for(r#"{"ExileFromGraveyard":{"filter":[{"Type":"Creature"}]}}"#),
    FUMES,
    false
)]
#[case::an_artifact(
    draws_for(r#"{"ExilePermanents":{"filter":[{"Type":"Artifact"}]}}"#),
    FUMES,
    false
)]
#[case::two_creatures(
    draws_for(r#"{"ExilePermanents":{"filter":[{"Type":"Creature"}],"count":2}}"#),
    FUMES,
    false
)]
#[case::sacrificed(
    draws_for(r#"{"Sacrifice":{"filter":[{"Type":"Creature"}]}}"#),
    FUMES,
    false
)]
fn ep26_exiling_a_permanent_you_control_writes_as_printed(
    #[case] ability: Ability,
    #[case] text: &str,
    #[case] reads: bool,
) {
    let found = written_variants(&ability);
    assert_eq!(found.iter().any(|w| same_text(w, text)), reads, "{found:?}");
}

#[rstest]
#[case::read("{B}, Exile a creature you control: Draw a card.", true)]
#[case::graveyard_stays_graveyard(
    "{B}, Exile a creature card from your graveyard: Draw a card.",
    false
)]
fn ep26_the_compiler_reads_the_exile_cost(#[case] text: &str, #[case] permanent: bool) {
    let read = clause(text, ClauseKind::Activated, false).expect(text);
    let json = serde_json::to_string(&read).unwrap_or_default();
    assert_eq!(json.contains(EXILE_CREATURE), permanent, "{json}");
}
