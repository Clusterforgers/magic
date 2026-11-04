//! Goal: the keys fg5ka maps mean what the card prints, and a shape the node cannot say stays a named gap.
use rstest::rstest;
use serde_json::{Value, json};

use super::{abilities, gaps, normal};

fn spell(line: &str) -> String {
    format!(
        "Name:T\nTypes:Sorcery\nA:SP$ {line}\nSVar:Win:DB$ GainLife | LifeAmount$ 2\nSVar:Lose:DB$ LoseLife | LifeAmount$ 2\n"
    )
}

#[rstest]
#[case::counter_unless_life(
    "Counter | ValidTgts$ Card | TargetType$ Spell | UnlessCost$ PayLife<3>",
    json!({"Pay": {"who": {"ControllerOf": {"Target": 0}}, "cost": [{"PayLife": 3}],
        "unpaid": [{"Counter": {"what": {"Target": 0}}}]}})
)]
#[case::you_unless_discard(
    "GainLife | LifeAmount$ 2 | UnlessCost$ Discard<1/Card> | UnlessPayer$ You",
    json!({"Pay": {"cost": [{"Discard": {}}], "unpaid": [{"GainLife": {"amount": 2}}]}})
)]
#[case::you_unless_sacrifice(
    "GainLife | LifeAmount$ 2 | UnlessCost$ Sac<1/Creature> | UnlessPayer$ You",
    json!({"Pay": {"cost": [{"Sacrifice": {"filter": [{"Type": "Creature"}]}}],
        "unpaid": [{"GainLife": {"amount": 2}}]}})
)]
fn an_unless_cost_of_items_is_paid_by_the_payer(#[case] line: &str, #[case] node: Value) {
    let script = spell(line);
    let found = abilities(&script);
    assert_eq!(found.len(), 1, "{:?}", gaps(&script));
    let effects = &found[0]["Spell"]["effects"];
    assert_eq!(effects[0], node, "{effects}");
    normal(found[0].clone());
}

#[rstest]
#[case::energy_x(
    "GainLife | LifeAmount$ 2 | UnlessCost$ PayEnergy<X> | UnlessPayer$ You",
    "amount:X"
)]
#[case::life_x(
    "GainLife | LifeAmount$ 2 | UnlessCost$ PayLife<X> | UnlessPayer$ You",
    "amount:X"
)]
#[case::mana_x(
    "GainLife | LifeAmount$ 2 | UnlessCost$ X | UnlessPayer$ You",
    "key:Unless.UnlessCost-X"
)]
#[case::each_player(
    "ChangeZone | DefinedPlayer$ Player | Origin$ Hand | Destination$ Exile | ChangeType$ Card | ChangeNum$ 2 | Mandatory$ True",
    "key:ChangeZone.Choose.DefinedPlayer"
)]
#[case::their_chooser_without_target(
    "ChangeZone | DefinedPlayer$ You | Chooser$ Targeted | Origin$ Hand | Destination$ Exile | ChangeType$ Card",
    "key:ChangeZone.Choose.Chooser"
)]
#[case::owner_filter_of_another_player(
    "ChangeZone | ValidTgts$ Opponent | DefinedPlayer$ Targeted | Origin$ Graveyard | Destination$ Exile | ChangeType$ Card.YouOwn",
    "key:ChangeZone.Choose.DefinedPlayer"
)]
fn a_shape_beyond_the_node_stays_a_gap(#[case] line: &str, #[case] gap: &str) {
    let script = spell(line);
    assert_eq!(abilities(&script), Vec::<Value>::new(), "{script}");
    assert!(
        gaps(&script).iter().any(|found| found == gap),
        "{:?}",
        gaps(&script)
    );
}

#[rstest]
#[case::you_choose_from_their_hand(
    "ChangeZone | ValidTgts$ Opponent | DefinedPlayer$ Targeted | Origin$ Hand | Destination$ Exile | Chooser$ You | ChangeType$ Card.nonLand | ChangeNum$ 1 | Mandatory$ True",
    json!({"targets": [{"what": "Opponent"}], "effects": [
        {"Choose": {"from": {"Each": [{"Not": {"Type": "Land"}}, {"In": "Hand"}, {"OwnedBy": {"Target": 0}}]}, "as": "picked"}},
        {"Exile": {"what": {"Bound": "picked"}}}]})
)]
#[case::they_choose_from_their_hand(
    "ChangeZone | ValidTgts$ Opponent | DefinedPlayer$ Targeted | Origin$ Hand | Destination$ Exile | Chooser$ Targeted | ChangeType$ Card | ChangeNum$ 1 | Mandatory$ True",
    json!({"targets": [{"what": "Opponent"}], "effects": [
        {"Choose": {"who": {"Target": 0}, "from": {"Each": [{"In": "Hand"}, {"OwnedBy": {"Target": 0}}]}, "as": "picked"}},
        {"Exile": {"what": {"Bound": "picked"}}}]})
)]
#[case::your_own_hand(
    "ChangeZone | DefinedPlayer$ You | Origin$ Hand | Destination$ Exile | ChangeType$ Card | ChangeNum$ 1 | Mandatory$ True",
    json!({"effects": [
        {"Choose": {"from": {"Each": [{"In": "Hand"}, {"OwnedBy": "You"}]}, "as": "picked"}},
        {"Exile": {"what": {"Bound": "picked"}}}]})
)]
fn a_defined_player_owns_the_cards_a_chooser_picks(#[case] line: &str, #[case] body: Value) {
    let script = spell(line);
    let found = abilities(&script);
    assert_eq!(found.len(), 1, "{:?}", gaps(&script));
    assert_eq!(normal(found[0].clone()), normal(json!({ "Spell": body })));
}
