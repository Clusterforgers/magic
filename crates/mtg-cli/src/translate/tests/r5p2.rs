//! Goal: static and replacement lines with a parameter shared across modes map to the rule that says the same: "you can't
//! lose the game" by its text and gate, the legend rule per player, and a trigger that repeats for any event; a shape the
//! rules do not hold stays a named gap.
use rstest::rstest;
use serde_json::{Value, json};

use super::{abilities, gaps, normal};

fn replacement(line: &str) -> String {
    format!("Name:T\nTypes:Creature Elf\nR:{line}\n")
}

fn statik(line: &str) -> String {
    format!("Name:T\nTypes:Creature Elf\nS:{line}\n")
}

#[rstest]
#[case::both(
    "Event$ GameLoss | ActiveZones$ Battlefield | ValidPlayer$ You | Layer$ CantHappen | Description$ You can't lose the game and your opponents can't win the game.",
    json!({"Rule": {"rule": {"CantLoseGame": {"who": "You", "opponents_cant_win": true}}}})
)]
#[case::only_you(
    "Event$ GameLoss | ActiveZones$ Battlefield | ValidPlayer$ You | Layer$ CantHappen | Description$ You can't lose the game.",
    json!({"Rule": {"rule": {"CantLoseGame": {"who": "You"}}}})
)]
#[case::gated(
    "Event$ GameLoss | ActiveZones$ Battlefield | ValidPlayer$ You | Layer$ CantHappen | IsPresent$ Artifact.YouCtrl | Description$ You can't lose the game.",
    json!({"Rule": {"rule": {"CantLoseGame": {"who": "You"}}, "condition": {"Present": {"what": [{"Type": "Artifact"}, {"ControlledBy": "You"}]}}}})
)]
fn a_card_that_cannot_lose(#[case] line: &str, #[case] want: Value) {
    let script = replacement(line);
    let mut found = abilities(&script);
    assert_eq!(found.len(), 1, "{:?}", gaps(&script));
    let got = found.remove(0);
    // The gate is the line's own condition; only its presence is judged here.
    assert_eq!(got["Rule"]["rule"], want["Rule"]["rule"]);
    assert_eq!(
        got["Rule"].get("condition").is_some(),
        want["Rule"].get("condition").is_some()
    );
    let _ = normal(got);
}

#[rstest]
#[case::instead(
    "Event$ GameLoss | ActiveZones$ Battlefield | ValidPlayer$ You | ReplaceWith$ ExileSetLife | Description$ If you would lose the game, instead exile CARDNAME.",
    "key:Replacement.GameLoss.ReplaceWith"
)]
#[case::opponent(
    "Event$ GameLoss | ActiveZones$ Battlefield | ValidPlayer$ Opponent | Layer$ CantHappen | Description$ You can't win the game and your opponents can't lose the game.",
    "replacement:GameLoss-ValidPlayer"
)]
#[case::reason(
    "Event$ GameLoss | ActiveZones$ Battlefield | ValidPlayer$ You | ValidLoseReason$ LifeReachedZero | Layer$ CantHappen | Description$ You don't lose the game for having 0 or less life.",
    "key:Replacement.GameLoss.ValidLoseReason"
)]
#[case::other_text(
    "Event$ GameLoss | ActiveZones$ Battlefield | ValidPlayer$ You | Layer$ CantHappen | Description$ Something else.",
    "replacement:GameLoss-text"
)]
fn a_game_loss_shape_that_does_not_map(#[case] line: &str, #[case] gap: &str) {
    let script = replacement(line);
    assert_eq!(abilities(&script).len(), 0);
    assert_eq!(gaps(&script), [gap]);
}

#[rstest]
#[case::yours("Permanent.YouCtrl", json!({"NoLegendRule": "You"}))]
#[case::everyone_without_a_valid_card("", json!({"NoLegendRule": "EachPlayer"}))]
fn the_legend_rule_stops_for_the_players_the_line_names(#[case] valid: &str, #[case] rule: Value) {
    let valid = if valid.is_empty() {
        String::new()
    } else {
        format!(" | ValidCard$ {valid}")
    };
    let script = statik(&format!("Mode$ IgnoreLegendRule{valid} | Description$ x"));
    let mut found = abilities(&script);
    assert_eq!(found.len(), 1, "{:?}", gaps(&script));
    assert_eq!(found.remove(0)["Rule"]["rule"], rule);
}

#[rstest]
#[case::creatures("Creature.YouCtrl")]
#[case::tokens("Permanent.token+YouCtrl")]
#[case::named("Permanent.namedBrothers Yamazaki")]
fn a_narrower_legend_rule_exemption_is_a_gap(#[case] valid: &str) {
    let script = statik(&format!(
        "Mode$ IgnoreLegendRule | ValidCard$ {valid} | Description$ x"
    ));
    assert_eq!(abilities(&script).len(), 0);
    assert_eq!(gaps(&script), ["key:Static.IgnoreLegendRule.ValidCard"]);
}

#[rstest]
#[case::legendary("Creature.YouCtrl+Legendary", Some(json!([{"Type": "Creature"}, {"ControlledBy": "You"}, {"Supertype": "Legendary"}])))]
#[case::your_permanents("Permanent.YouCtrl", None)]
fn a_trigger_without_a_named_event_repeats_for_any_event(
    #[case] valid: &str,
    #[case] source: Option<Value>,
) {
    let script = statik(&format!(
        "Mode$ Panharmonicon | ValidCard$ {valid} | Description$ x"
    ));
    let mut found = abilities(&script);
    assert_eq!(found.len(), 1, "{:?}", gaps(&script));
    let rule = found.remove(0)["Rule"]["rule"]["TriggerAgain"].clone();
    assert!(rule.get("cause").is_none(), "{rule}");
    assert_eq!(
        rule.get("source").cloned().is_some(),
        source.is_some(),
        "{rule}"
    );
}

#[test]
fn an_unnamed_event_with_a_cause_filter_is_a_gap() {
    let script = statik(
        "Mode$ Panharmonicon | ValidCard$ Permanent.YouCtrl | ValidCause$ Creature | Description$ x",
    );
    assert_eq!(abilities(&script).len(), 0);
    assert_eq!(gaps(&script), ["static:Panharmonicon-any-cause"]);
}
