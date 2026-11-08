//! Goal: round 5 piece P3 (zone-moving and looking effects): a discard the caster chooses from a revealed hand shows the hand, names the chooser and keeps the count and filter; values the rule does not know stay named gaps.
use rstest::rstest;
use serde_json::{Value, json};

use super::{abilities, gaps};

/// The effects of the first ability of the card.
fn effects(text: &str) -> Vec<Value> {
    let found = abilities(text);
    let ability = found.first().cloned().unwrap_or_default();
    let inner = ability.as_object().and_then(|o| o.values().next()).cloned();
    inner
        .and_then(|a| a.get("effects").cloned())
        .and_then(|e| e.as_array().cloned())
        .unwrap_or_default()
}

const HAND: &str =
    r#"{"Reveal": {"what": {"Each": [{"OwnedBy": {"Target": 0}}, {"In": "Hand"}]}}}"#;

#[rstest]
#[case::brainbite("ValidTgts$ Opponent | NumCards$ 1", json!({"Discard": {"who": {"Target": 0}, "count": {"Fixed": 1}, "random": false, "by": "You"}}))]
#[case::duress_filter("ValidTgts$ Opponent | DiscardValid$ Card.nonCreature+nonLand", json!({"Discard": {"who": {"Target": 0}, "count": {"Fixed": 1}, "random": false, "by": "You", "filter": ["Not", "Creature"]}}))]
#[case::optional("ValidTgts$ Opponent | Optional$ True | DiscardValid$ Card.nonLand", json!({"Discard": {"who": {"Target": 0}, "count": {"Fixed": 1}, "random": false, "by": "You", "optional": true, "filter": ["Not", "Land"]}}))]
fn the_caster_chooses_from_a_revealed_hand(#[case] params: &str, #[case] discard: Value) {
    let text = format!("Name:T\nTypes:Sorcery\nA:SP$ Discard | Mode$ RevealYouChoose | {params}\n");
    let found = effects(&text);
    assert_eq!(
        found.first(),
        Some(&serde_json::from_str::<Value>(HAND).unwrap_or_default())
    );
    let sent = found.get(1).cloned().unwrap_or_default();
    assert_eq!(sent["Discard"]["by"], discard["Discard"]["by"]);
    assert_eq!(sent["Discard"]["who"], discard["Discard"]["who"]);
    assert_eq!(sent["Discard"]["optional"], discard["Discard"]["optional"]);
    assert_eq!(
        sent["Discard"]["filter"].is_null(),
        discard["Discard"].get("filter").is_none()
    );
    assert_eq!(gaps(&text), Vec::<String>::new());
}

#[rstest]
#[case::your_own_hand("Defined$ You", "key:Discard.Mode=RevealYouChoose+You")]
#[case::unknown_mode("Mode$ Nonsense | ValidTgts$ Opponent", "key:Discard.Mode=Nonsense")]
fn what_the_rule_does_not_know_is_a_gap(#[case] params: &str, #[case] gap: &str) {
    let mode = if params.contains("Mode$") {
        ""
    } else {
        "Mode$ RevealYouChoose | "
    };
    let text = format!("Name:T\nTypes:Sorcery\nA:SP$ Discard | {mode}{params}\n");
    assert_eq!(gaps(&text), [gap]);
}
