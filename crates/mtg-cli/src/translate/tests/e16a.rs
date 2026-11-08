//! Goal: pack e16a's facts of the turn's history read from Forge lines: the permanents of a kind you've sacrificed this turn, the opponents dealt combat or noncombat damage, the total life lost by all players, a player's first discards and first surveil of the turn, a tap other than to attack and a permanent's first tap; a shape these readers do not know stays a gap.
use rstest::rstest;
use serde_json::{Value, json};

use super::{abilities, gaps};

/// The card text with a draw ability that may be activated only if `svar` (as X) is at least 1.
fn gated(svar: &str) -> String {
    format!(
        "Name:T\nTypes:Artifact\nA:AB$ Draw | Cost$ T | CheckSVar$ X | SpellDescription$ Draw a card.\nSVar:X:{svar}\n"
    )
}

fn condition_left(text: &str) -> Value {
    assert_eq!(gaps(text), Vec::<String>::new());
    abilities(text)[0]["Activated"]["condition"]["Compare"]["left"].clone()
}

fn dealt(what: &str) -> Value {
    json!({"Players": {"PlayersWhere": {"who": "Opponents", "test": {"Compare": {
        "left": {"Tallied": {"who": "You", "what": what}}, "op": "Ge", "right": 1}}}}})
}

#[rstest]
#[case::an_artifact(
    "PlayerCountPropertyYou$SacrificedThisTurn Artifact",
    json!({"Tally": {"event": "Sacrificed", "who": "You", "filter": [{"Type": "Artifact"}]}})
)]
#[case::a_creature(
    "PlayerCountPropertyYou$SacrificedThisTurn Creature",
    json!({"Tally": {"event": "Sacrificed", "who": "You", "filter": [{"Type": "Creature"}]}})
)]
#[case::any_permanent(
    "PlayerCountPropertyYou$SacrificedThisTurn Permanent",
    json!({"Tallied": {"who": "You", "what": "Sacrificed"}})
)]
#[case::combat(
    "PlayerCountOpponents$HasPropertywasDealtCombatDamageThisTurn",
    dealt("CombatDamageTaken")
)]
#[case::noncombat(
    "PlayerCountOpponents$HasPropertywasDealtNonCombatDamageThisTurn",
    dealt("NoncombatDamageTaken")
)]
#[case::all_life_lost(
    "PlayerCountPlayers$LifeLostThisTurn",
    json!({"Tallied": {"who": "EachPlayer", "what": "LifeLost"}})
)]
fn history_amounts_read(#[case] svar: &str, #[case] expected: Value) {
    assert_eq!(condition_left(&gated(svar)), expected);
}

/// The card text with a trigger line that gains 1 life.
fn triggered(line: &str) -> String {
    format!(
        "Name:T\nTypes:Creature\nPT:1/1\nT:{line} | TriggerZones$ Battlefield | Execute$ TrigGain | TriggerDescription$ X.\nSVar:TrigGain:DB$ GainLife | LifeAmount$ 1\n"
    )
}

#[rstest]
#[case::first_discards(
    "Mode$ DiscardedAll | ValidPlayer$ You | FirstTime$ True",
    json!({"Nth": {"event": {"Batch": {"Discarded": {"by": "You"}}}, "nth": 1}})
)]
#[case::first_surveil(
    "Mode$ Surveil | ValidPlayer$ You | FirstTime$ True",
    json!({"Nth": {"event": {"Surveilled": {"who": "You"}}, "nth": 1}})
)]
#[case::not_attacking(
    "Mode$ Taps | ValidCard$ Creature.OppCtrl | Attacker$ False",
    json!({"TappedNotAttacking": {"what": [{"Type": "Creature"}, {"ControlledBy": "Opponents"}]}})
)]
#[case::first_tap(
    "Mode$ Taps | ValidCard$ Card.Self | FirstTime$ True",
    json!({"Nth": {"event": {"Tapped": {"what": ["This"]}}, "nth": 1}})
)]
fn history_triggers_read(#[case] line: &str, #[case] expected: Value) {
    let text = triggered(line);
    assert_eq!(gaps(&text), Vec::<String>::new());
    assert_eq!(abilities(&text)[0]["Triggered"]["when"], expected);
}

#[rstest]
#[case::first_discards_of_a_kind(
    "Mode$ DiscardedAll | ValidPlayer$ You | FirstTime$ True | ValidCard$ Card.nonLand",
    "key:Trigger.DiscardedAll.FirstTime-of-a-kind"
)]
#[case::first_tap_not_attacking(
    "Mode$ Taps | ValidCard$ Card.Self | FirstTime$ True | Attacker$ False",
    "key:Trigger.Tapped.Attacker"
)]
#[case::attacking_only(
    "Mode$ Taps | ValidCard$ Card.Self | Attacker$ True",
    "key:Trigger.Tapped.Attacker"
)]
fn other_history_shapes_stay_gaps(#[case] line: &str, #[case] gap: &str) {
    let found = gaps(&triggered(line));
    assert!(found.contains(&gap.to_owned()), "{found:?}");
}
