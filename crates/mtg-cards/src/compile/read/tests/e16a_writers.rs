//! Goal: round 16 pack e16a writes the facts of the turn's history as the cards print them (the permanents of a kind you've sacrificed, an opponent dealt combat or noncombat damage, the total life lost by all players, a player's first discards, a tap other than to attack); another kind, another player, another number or a missing condition does not read back.
use super::*;

fn written(json: &str, clause: &str) -> bool {
    written_variants(&ability(json))
        .iter()
        .any(|wording| same_text(wording, clause))
}

/// "{T}: You gain 3 life." with the condition `condition`.
fn gated(condition: &str) -> String {
    format!(
        r#"{{"Activated":{{"condition":{condition},"cost":["Tap"],"effects":[{{"GainLife":{{"amount":3}}}}]}}}}"#
    )
}

fn sacrificed(filter: &str, who: &str, number: u32) -> String {
    gated(&format!(
        r#"{{"Compare":{{"left":{{"Tally":{{"event":"Sacrificed","filter":{filter},"who":"{who}"}}}},"op":"Ge","right":{number}}}}}"#
    ))
}

fn dealt(what: &str, who: &str) -> String {
    gated(&format!(
        r#"{{"Compare":{{"left":{{"Players":{{"PlayersWhere":{{"test":{{"Compare":{{"left":{{"Tallied":{{"what":"{what}","who":"You"}}}},"op":"Ge","right":1}}}},"who":"{who}"}}}}}},"op":"Ge","right":1}}}}"#
    ))
}

const ARTIFACT: &str = r#"[{"Type":"Artifact"}]"#;
const CLUE: &str = r#"[{"Subtype":"Clue"}]"#;
const SAC_ARTIFACT: &str =
    "{T}: You gain 3 life. Activate only if you've sacrificed an artifact this turn.";
const NONCOMBAT: &str =
    "{T}: You gain 3 life. Activate only if an opponent has been dealt noncombat damage this turn.";

#[rstest]
#[case::an_artifact(sacrificed(ARTIFACT, "You", 1), SAC_ARTIFACT, true)]
#[case::three_clues(
    sacrificed(CLUE, "You", 3),
    "{T}: You gain 3 life. Activate only if you sacrificed three or more Clues this turn.",
    true
)]
#[case::another_kind(sacrificed(CLUE, "You", 1), SAC_ARTIFACT, false)]
#[case::another_player(sacrificed(ARTIFACT, "Opponents", 1), SAC_ARTIFACT, false)]
#[case::another_number(sacrificed(ARTIFACT, "You", 2), SAC_ARTIFACT, false)]
#[case::noncombat(dealt("NoncombatDamageTaken", "Opponents"), NONCOMBAT, true)]
#[case::was_dealt(
    dealt("NoncombatDamageTaken", "Opponents"),
    "{T}: You gain 3 life. Activate only if an opponent was dealt noncombat damage this turn.",
    true
)]
#[case::combat_for_noncombat(dealt("CombatDamageTaken", "Opponents"), NONCOMBAT, false)]
#[case::any_player(dealt("NoncombatDamageTaken", "EachPlayer"), NONCOMBAT, false)]
fn conditions_of_the_turn_read_back(
    #[case] json: String,
    #[case] clause: &str,
    #[case] same: bool,
) {
    assert_eq!(written(&json, clause), same);
}

const MOONSHAE: &str = "When ~ enters, draw cards equal to the number of opponents who were dealt combat damage this turn.";
const SIGIL: &str = "{T}: You gain life equal to the total life lost by all players this turn.";

#[rstest]
#[case::opponents_dealt_combat(
    r#"{"Triggered":{"when":{"ZoneChange":{"what":["This"],"to":"Battlefield"}},"effects":[{"Draw":{"count":{"Players":{"PlayersWhere":{"who":"Opponents","test":{"Compare":{"left":{"Tallied":{"who":"You","what":"CombatDamageTaken"}},"op":"Ge","right":1}}}}}}}]}}"#,
    MOONSHAE,
    true
)]
#[case::opponents_dealt_noncombat(
    r#"{"Triggered":{"when":{"ZoneChange":{"what":["This"],"to":"Battlefield"}},"effects":[{"Draw":{"count":{"Players":{"PlayersWhere":{"who":"Opponents","test":{"Compare":{"left":{"Tallied":{"who":"You","what":"NoncombatDamageTaken"}},"op":"Ge","right":1}}}}}}}]}}"#,
    MOONSHAE,
    false
)]
#[case::all_life_lost(
    r#"{"Activated":{"cost":["Tap"],"effects":[{"GainLife":{"amount":{"Tallied":{"who":"EachPlayer","what":"LifeLost"}}}}]}}"#,
    SIGIL,
    true
)]
#[case::your_life_lost(
    r#"{"Activated":{"cost":["Tap"],"effects":[{"GainLife":{"amount":{"Tallied":{"who":"You","what":"LifeLost"}}}}]}}"#,
    SIGIL,
    false
)]
fn amounts_of_the_turn_read_back(#[case] json: &str, #[case] clause: &str, #[case] same: bool) {
    assert_eq!(written(json, clause), same);
}

const RIELLE: &str =
    "Whenever you discard one or more cards for the first time each turn, you gain 1 life.";
const RHODA: &str = "Whenever a creature an opponent controls becomes tapped, if it isn't being declared as an attacker, you gain 1 life.";
const OPPONENTS_CREATURE: &str = r#"[{"Type":"Creature"},{"ControlledBy":"Opponents"}]"#;

fn gains(when: &str) -> String {
    format!(r#"{{"Triggered":{{"when":{when},"effects":[{{"GainLife":{{"amount":1}}}}]}}}}"#)
}

#[rstest]
#[case::first_discards(
    gains(r#"{"Nth":{"event":{"Batch":{"Discarded":{"by":"You"}}},"nth":1}}"#),
    RIELLE,
    true
)]
#[case::every_discard_batch(gains(r#"{"Batch":{"Discarded":{"by":"You"}}}"#), RIELLE, false)]
#[case::first_discards_of_a_kind(
    gains(r#"{"Nth":{"event":{"Batch":{"Discarded":{"by":"You","what":[{"Type":"Land"}]}}},"nth":1}}"#),
    RIELLE,
    false
)]
#[case::not_attacking(
    gains(&format!(r#"{{"TappedNotAttacking":{{"what":{OPPONENTS_CREATURE}}}}}"#)),
    RHODA,
    true
)]
#[case::any_tap(
    gains(&format!(r#"{{"Tapped":{{"what":{OPPONENTS_CREATURE}}}}}"#)),
    RHODA,
    false
)]
fn triggers_of_the_turn_read_back(#[case] json: String, #[case] clause: &str, #[case] same: bool) {
    assert_eq!(written(&json, clause), same);
}
