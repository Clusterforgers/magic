//! Goal: round 3 pack F (costs and mana that need the engine) translates each construct to the rule it names, one rstest table per rule.
use rstest::rstest;
use serde_json::{Value, json};

use super::{abilities, gaps};

fn creature(lines: &str) -> String {
    format!("Name:T\nManaCost:2\nTypes:Creature Eldrazi\nPT:1/1\n{lines}\n")
}

#[rstest]
#[case::two_opponents_cards(
    "A:AB$ Draw | Cost$ ExiledMoveToGrave<2/Card.OppOwn/cards your opponents own> | NumCards$ 1",
    json!([{"ExiledToGraveyard": {"count": 2, "filter": [{"OwnedBy": "Opponents"}]}}])
)]
#[case::one_exiled_with_the_source(
    "A:AB$ Draw | Cost$ ExiledMoveToGrave<1/Card.ExiledWithSource/card exiled with CARDNAME> | NumCards$ 1",
    json!([{"ExiledToGraveyard": {"filter": [{"ExiledWith": "This"}]}}])
)]
fn exiled_cards_put_into_graveyards_are_a_cost(#[case] line: &str, #[case] cost: Value) {
    let text = creature(line);
    let found = abilities(&text);
    assert!(!found.is_empty(), "{:?}", gaps(&text));
    assert_eq!(found[0]["Activated"]["cost"], cost);
}

fn enchantment(lines: &str) -> String {
    format!("Name:T\nManaCost:2\nTypes:Enchantment\n{lines}\n")
}

#[rstest]
#[case::one_black_instead(
    "R:Event$ ProduceMana | ActiveZones$ Battlefield | ValidCard$ Land | ReplaceWith$ ProduceB | Description$ x\nSVar:ProduceB:DB$ ReplaceMana | ReplaceMana$ B",
    json!({"change": {"One": "Black"}, "what": [{"Type": "Land"}], "who": "EachPlayer"})
)]
#[case::the_type_in_the_same_amount(
    "R:Event$ ProduceMana | ActiveZones$ Battlefield | ValidCard$ Land | ReplaceWith$ ProduceC | Description$ x\nSVar:ProduceC:DB$ ReplaceMana | ReplaceType$ C",
    json!({"change": {"OnlyType": "Colorless"}, "what": [{"Type": "Land"}], "who": "EachPlayer"})
)]
#[case::twice_for_your_taps(
    "R:Event$ ProduceMana | ActiveZones$ Battlefield | ValidActivator$ You | ValidCard$ Permanent | ReplaceWith$ Twice | Description$ x\nSVar:Twice:DB$ ReplaceMana | ReplaceAmount$ 2",
    json!({"change": {"Times": 2}, "what": [], "who": "You"})
)]
fn a_replacement_of_produced_mana_is_a_rule_of_the_permanent(
    #[case] lines: &str,
    #[case] rule: Value,
) {
    let text = enchantment(lines);
    let found = abilities(&text);
    assert!(!found.is_empty(), "{:?}", gaps(&text));
    assert_eq!(found[0]["Rule"]["rule"]["ManaProduced"], rule);
}

#[rstest]
#[case::a_choice_of_color(
    "R:Event$ ProduceMana | ActiveZones$ Battlefield | ValidCard$ Land | ReplaceWith$ Any | Description$ x\nSVar:Any:DB$ ReplaceMana | ReplaceType$ Any"
)]
#[case::the_chosen_color(
    "R:Event$ ProduceMana | ActiveZones$ Battlefield | ValidCard$ Land | ReplaceWith$ Chosen | Description$ x\nSVar:Chosen:DB$ ReplaceMana | ReplaceColor$ Chosen"
)]
#[case::an_unread_key(
    "R:Event$ ProduceMana | ActiveZones$ Battlefield | ValidCard$ Land | ManaAmount$ GE2 | ReplaceWith$ C | Description$ x\nSVar:C:DB$ ReplaceMana | ReplaceMana$ C"
)]
fn a_replacement_of_produced_mana_the_engine_cannot_play_stays_a_gap(#[case] lines: &str) {
    let text = enchantment(lines);
    assert!(abilities(&text).is_empty(), "{:?}", abilities(&text));
    assert!(!gaps(&text).is_empty());
}

#[test]
fn an_effect_that_replaces_produced_mana_for_a_turn_is_a_timed_rule() {
    let text = "Name:T\nManaCost:U\nTypes:Instant\nA:SP$ Effect | ReplacementEffects$ Rep | SpellDescription$ x\nSVar:Rep:Event$ ProduceMana | ActiveZones$ Command | ValidCard$ Land.nonBasic | ReplaceWith$ Pc | Description$ x\nSVar:Pc:DB$ ReplaceMana | ReplaceType$ C\n";
    let found = abilities(text);
    assert!(!found.is_empty(), "{:?}", gaps(text));
    let rule = &found[0]["Spell"]["effects"][0]["Rule"];
    assert_eq!(rule["duration"], json!("UntilEndOfTurn"));
    assert_eq!(
        rule["rule"]["ManaProduced"]["change"],
        json!({"OnlyType": "Colorless"})
    );
}

#[rstest]
#[case::alone("Waterbend<3>", json!([{"Waterbend": 3}]))]
#[case::with_the_tap("Waterbend<3> T", json!([{"Waterbend": 3}, "Tap"]))]
fn waterbend_is_a_cost_of_an_activated_ability(#[case] cost: &str, #[case] expected: Value) {
    let text = creature(&format!("A:AB$ Draw | Cost$ {cost} | NumCards$ 1"));
    let found = abilities(&text);
    assert!(!found.is_empty(), "{:?}", gaps(&text));
    assert_eq!(found[0]["Activated"]["cost"], expected);
}

#[rstest]
#[case::x("A:AB$ Draw | Cost$ Waterbend<X> | NumCards$ 1\nSVar:X:Count$xPaid")]
#[case::in_a_trigger(
    "T:Mode$ Phase | Phase$ Upkeep | ValidPlayer$ You | Execute$ TrigDraw\nSVar:TrigDraw:AB$ Draw | Cost$ Waterbend<2> | NumCards$ 1"
)]
fn waterbend_anywhere_else_stays_a_gap(#[case] lines: &str) {
    let text = creature(lines);
    assert!(abilities(&text).is_empty(), "{:?}", abilities(&text));
    assert!(!gaps(&text).is_empty());
}
