//! Goal: "do this only once each turn" on a first step that is a choice reads as the limit spent when the choice is done, a reveal without a look first reads as the reveal, and a target player beside `Defined$` reads as Forge resolves it (the drawer, the new controller, whose library or hand), while every other shape stays a gap.
use rstest::rstest;
use serde_json::{Value, json};

use super::{abilities, gaps};

fn card(lines: &str) -> String {
    format!("Name:T\nManaCost:2 U\nTypes:Artifact\n{lines}")
}

fn first(text: &str) -> Value {
    let found = abilities(text);
    assert!(gaps(text).is_empty(), "{:?}", gaps(text));
    found.into_iter().next().unwrap_or(Value::Null)
}

fn has(value: &Value, part: &Value) -> bool {
    value.to_string().contains(&part.to_string())
}

#[rstest]
#[case::optional("OptionalDecider$ You | ResolvedLimit$ 1", "\"OncePerTurnDone\"")]
#[case::mandatory("ResolvedLimit$ 1", "\"OncePerTurn\"")]
#[case::triggers_once("OptionalDecider$ You | ActivationLimit$ 1", "\"OncePerTurn\"")]
fn a_limit_on_a_choice_counts_the_choices_done(#[case] keys: &str, #[case] limit: &str) {
    let text = card(&format!(
        "T:Mode$ LifeGained | ValidPlayer$ You | TriggerZones$ Battlefield | Execute$ Draw | {keys} | TriggerDescription$ x\nSVar:Draw:DB$ Draw | NumCards$ 1\n"
    ));
    let found = first(&text);
    assert_eq!(found["Triggered"]["limit"].to_string(), limit, "{found}");
}

#[rstest]
#[case::twice("OptionalDecider$ You | ResolvedLimit$ 2")]
#[case::both_limits("OptionalDecider$ You | ResolvedLimit$ 1 | ActivationLimit$ 1")]
fn another_limit_on_a_choice_stays_a_gap(#[case] keys: &str) {
    let text = card(&format!(
        "T:Mode$ LifeGained | ValidPlayer$ You | TriggerZones$ Battlefield | Execute$ Draw | {keys} | TriggerDescription$ x\nSVar:Draw:DB$ Draw | NumCards$ 1\n"
    ));
    assert_eq!(
        gaps(&text),
        vec!["key:Trigger.ResolvedLimit-optional".to_owned()]
    );
}

#[test]
fn a_reveal_without_a_look_reveals_the_top_card() {
    let found = first(&card(
        "A:AB$ PeekAndReveal | Cost$ 2 | NoPeek$ True | SpellDescription$ Reveal the top card of your library.\n",
    ));
    assert!(
        has(&found, &json!({"Reveal": {"what": {"Bound": "top"}}})),
        "{found}"
    );
}

#[rstest]
#[case::with_a_pick("NoPeek$ True | RevealValid$ Creature")]
#[case::not_revealed("NoPeek$ True | NoReveal$ True")]
fn a_reveal_without_a_look_and_a_pick_stays_a_gap(#[case] keys: &str) {
    let text = card(&format!(
        "A:AB$ PeekAndReveal | Cost$ 2 | {keys} | SpellDescription$ x\n"
    ));
    assert!(!gaps(&text).is_empty(), "{:?}", abilities(&text));
}

#[rstest]
#[case::this_target_draws("Defined$ ThisTargetedPlayer", json!({"Draw": {"count": 2, "who": {"Target": 0}}}))]
#[case::the_target_draws("Defined$ TargetedPlayer", json!({"Draw": {"count": 2, "who": {"Target": 0}}}))]
fn a_draw_with_a_target_and_a_defined_player(#[case] defined: &str, #[case] draw: Value) {
    let found = first(&card(&format!(
        "A:AB$ Draw | Cost$ 1 | NumCards$ 2 | ValidTgts$ Player | {defined} | SpellDescription$ x\n"
    )));
    assert_eq!(found["Activated"]["effects"][0], draw, "{found}");
    assert_eq!(found["Activated"]["targets"][0]["what"], json!("Player"));
}

#[test]
fn a_target_the_draw_never_reads_is_unused() {
    let text = card(
        "A:AB$ Draw | Cost$ 1 | NumCards$ 2 | ValidTgts$ Player | Defined$ You | SpellDescription$ x\n",
    );
    assert_eq!(gaps(&text), vec!["targets:unused".to_owned()]);
}

#[test]
fn a_target_opponent_gains_control_of_the_defined_object() {
    let found = first(&card(
        "A:AB$ GainControl | Cost$ T | Defined$ Self | ValidTgts$ Opponent | SpellDescription$ Target opponent gains control of CARDNAME.\n",
    ));
    let effect = &found["Activated"]["effects"][0]["Continuous"];
    assert_eq!(effect["affected"], json!("This"), "{found}");
    assert_eq!(effect["mods"], json!([{"SetController": {"Target": 0}}]));
}

#[rstest]
#[case::the_target_again("Defined$ Targeted", true)]
#[case::another_object("Defined$ Self", false)]
fn a_defined_object_beside_a_target_object(#[case] defined: &str, #[case] read: bool) {
    let text = card(&format!(
        "A:AB$ GainControl | Cost$ T | ValidTgts$ Creature | {defined} | SpellDescription$ x\n"
    ));
    assert_eq!(gaps(&text).is_empty(), read, "{:?}", gaps(&text));
}

#[rstest]
#[case::at_random("Random$ True", json!({"ChooseAtRandom": {"as": "revealed", "from": {"Each": [{"OwnedBy": {"Target": 0}}, {"In": "Hand"}]}}}))]
#[case::chosen_by_the_target(
    "NumCards$ 2",
    json!({"Choose": {"as": "revealed", "count": {"max": 2, "min": 2}, "from": {"Each": [{"In": "Hand"}, {"OwnedBy": {"Target": 0}}]}, "who": {"Target": 0}}})
)]
fn a_target_player_reveals_from_their_hand(#[case] keys: &str, #[case] pick: Value) {
    let found = first(&card(&format!(
        "A:AB$ Reveal | Cost$ 1 | ValidTgts$ Opponent | {keys} | SpellDescription$ x\n"
    )));
    assert_eq!(found["Activated"]["effects"][0], pick, "{found}");
}

#[rstest]
#[case::look("NoReveal$ True", true)]
#[case::a_pick("RevealValid$ Creature", false)]
#[case::up_to("TargetMin$ 0 | TargetMax$ 1", false)]
fn the_controller_looks_at_the_target_players_library(#[case] keys: &str, #[case] read: bool) {
    let text = card(&format!(
        "A:AB$ PeekAndReveal | Cost$ T | ValidTgts$ Player | {keys} | SpellDescription$ x\n"
    ));
    if read {
        let found = first(&text);
        assert!(
            has(
                &found,
                &json!({"LookAtTop": {"as": "top", "who": {"Target": 0}}})
            ),
            "{found}"
        );
    } else {
        assert!(!gaps(&text).is_empty(), "{:?}", abilities(&text));
    }
}

#[rstest]
#[case::a_card_at_random(
    "Random$ True",
    "Look at a card at random in target player's hand.",
    "key:Reveal.Random-look"
)]
#[case::the_cards_chosen(
    "NumCards$ 2",
    "Target opponent chooses two cards from their hand. Look at those cards.",
    "key:Reveal.ValidTgts-look"
)]
fn a_look_at_another_players_hand_is_no_reveal(
    #[case] keys: &str,
    #[case] oracle: &str,
    #[case] gap: &str,
) {
    let text = card(&format!(
        "A:AB$ Reveal | Cost$ 1 | ValidTgts$ Opponent | {keys} | SpellDescription$ {oracle}\nOracle:{oracle}\n"
    ));
    assert_eq!(gaps(&text), vec![gap.to_owned()]);
}

#[test]
fn a_reveal_at_random_of_an_earlier_target_still_reads() {
    let found = first(&card(
        "A:AB$ LoseLife | Cost$ T | ValidTgts$ Opponent | LifeAmount$ 2 | SubAbility$ DBReveal | SpellDescription$ x\nSVar:DBReveal:DB$ Reveal | Defined$ Targeted | Random$ True\n",
    ));
    assert!(has(&found, &json!({"OwnedBy": {"Target": 0}})), "{found}");
}
