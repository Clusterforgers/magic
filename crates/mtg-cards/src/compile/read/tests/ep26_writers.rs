//! Goal: the writers of ep26 write the card's words and nothing near them: a target "chosen at random" ("deals 2 damage to any target chosen at random", "deals 2 damage to target opponent chosen at random") reads back only when the rules draw it, and a target the controller chooses does not read as one drawn at random.
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
