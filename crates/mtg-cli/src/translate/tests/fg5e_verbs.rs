//! Goal: the single-node verbs (keyword and player actions, counters, exchanges, coin flips) map to their engine node with
//! the right object, and a form the node cannot say stays a named gap.
use rstest::rstest;
use serde_json::{Value, json};

use super::{abilities, gaps, normal};

fn spell(line: &str) -> String {
    format!(
        "Name:T\nTypes:Sorcery\nA:SP$ {line}\nSVar:Win:DB$ GainLife | LifeAmount$ 2\nSVar:Lose:DB$ LoseLife | LifeAmount$ 2\n"
    )
}

#[rstest]
#[case::explore("Explore | ValidTgts$ Creature.YouCtrl", json!({"Explore": {"what": {"Target": 0}}}))]
#[case::monarch("BecomeMonarch", json!({"BecomeMonarch": {}}))]
#[case::monarch_opponent("BecomeMonarch | Defined$ Opponent", json!({"BecomeMonarch": {"who": "Opponents"}}))]
#[case::initiative("TakeInitiative", json!({"TakeInitiative": {}}))]
#[case::learn("Learn", json!({"Learn": {}}))]
#[case::time_travel("TimeTravel", json!({"TimeTravel": {}}))]
#[case::incubate("Incubate | Amount$ 3", json!({"Incubate": {"count": 3}}))]
#[case::unattach("Unattach | ValidTgts$ Creature", json!({"Unattach": {"what": {"Target": 0}}}))]
#[case::tap_or_untap("TapOrUntap | ValidTgts$ Creature", json!({"TapOrUntap": {"what": {"Target": 0}}}))]
#[case::phase_out("Phases | Defined$ Self", json!({"PhaseOut": {"what": "This"}}))]
#[case::extra_turn("AddTurn | NumTurns$ 2", json!({"ExtraTurn": {"count": 2}}))]
#[case::manifest("Manifest | Amount$ 2", json!({"Manifest": {"count": 2}}))]
#[case::cloak("Cloak | Defined$ TopOfLibrary", json!({"Cloak": {}}))]
#[case::double("MultiplyCounter | Defined$ Self | CounterType$ P1P1", json!({"MultiplyCounters": {"on": "This", "counter": "+1/+1"}}))]
#[case::triple("MultiplyCounter | Defined$ Self | CounterType$ P1P1 | Multiplier$ 3", json!({"MultiplyCounters": {"on": "This", "counter": "+1/+1", "factor": 3}}))]
#[case::move_counter(
    "MoveCounter | ValidTgts$ Creature | Source$ Self | CounterType$ P1P1 | CounterNum$ 1",
    json!({"MoveCounters": {"from": "This", "to": {"Target": 0}, "counter": "+1/+1"}})
)]
#[case::exchange_control(
    "ExchangeControl | Defined$ Self | ValidTgts$ Creature",
    json!({"ExchangeControl": {"what": ["This", {"Target": 0}], "duration": "Forever"}})
)]
#[case::exchange_life("ExchangeLife | ValidTgts$ Opponent", json!({"ExchangeLife": {"with": {"Target": 0}}}))]
#[case::flip(
    "FlipCoin | NoCall$ True | WinSubAbility$ Win | LoseSubAbility$ Lose",
    json!({"FlipCoin": {"win": [{"GainLife": {"amount": 2}}], "lose": [{"LoseLife": {"amount": 2}}]}})
)]
fn a_verb_maps_to_its_node(#[case] line: &str, #[case] node: Value) {
    let script = spell(line);
    let mut found = abilities(&script);
    assert_eq!(found.len(), 1, "{:?}", gaps(&script));
    let found = found.remove(0);
    let targets = found["Spell"].get("targets").cloned();
    let mut want = json!({"Spell": {"effects": [node]}});
    if let Some(targets) = targets {
        want["Spell"]["targets"] = targets;
    }
    assert_eq!(normal(found), normal(want));
}

#[rstest]
#[case::phase_in_or_out(
    "Phases | Defined$ Self | PhaseInOrOut$ True",
    "key:Phases.PhaseInOrOut"
)]
#[case::phase_set("Phases | AllValid$ Creature", "key:Phases.AllValid")]
#[case::manifest_named("Manifest | Defined$ Remembered", "key:Manifest.Defined")]
#[case::move_any(
    "MoveCounter | ValidTgts$ Creature | Source$ Self | CounterType$ Any",
    "key:MoveCounter.Source-or-CounterType"
)]
#[case::move_all(
    "MoveCounter | ValidTgts$ Creature | Source$ Self | CounterType$ P1P1 | CounterNum$ All",
    "key:MoveCounter.CounterNum"
)]
#[case::exchange_pair(
    "ExchangeControl | ValidTgts$ Creature | TargetMin$ 2 | TargetMax$ 2",
    "key:ExchangeControl.shape"
)]
#[case::flip_other(
    "FlipCoin | NoCall$ True | Flipper$ Remembered",
    "key:FlipCoin.Flipper"
)]
fn a_form_beyond_the_node_stays_a_gap(#[case] line: &str, #[case] gap: &str) {
    let script = spell(line);
    assert_eq!(abilities(&script), Vec::<Value>::new(), "{script}");
    assert!(
        gaps(&script).iter().any(|found| found == gap),
        "{:?}",
        gaps(&script)
    );
}
