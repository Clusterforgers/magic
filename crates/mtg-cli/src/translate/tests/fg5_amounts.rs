//! Goal: the game-fact counts (domain, party, mana spent), the permanents that entered or died this turn, a player's cards and life, adamant and halving translate into the DSL's `Amount`, every translated amount parses as one, and what the DSL cannot count stays a gap named for its real cause.
use mtg_cards::value::Amount;
use rstest::rstest;
use serde_json::{Value, json};

use super::{abilities, gaps};

fn drawing(svars: &str) -> String {
    format!("Name:T\nTypes:Sorcery\nA:SP$ Draw | ValidTgts$ Player | NumCards$ X\n{svars}\n")
}

fn count(text: &str) -> Value {
    let found = abilities(text);
    found[0]["Spell"]["effects"][0]["Draw"]["count"].clone()
}

#[rstest]
#[case::domain(
    "SVar:X:Count$Domain",
    json!({"Distinct": {"of": [{"Type": "Land"}, {"ControlledBy": "You"}], "stat": "BasicLandTypes"}})
)]
#[case::party("SVar:X:Count$Party", json!({"Party": "You"}))]
#[case::converge("SVar:X:Count$Converge", json!("ColorsSpent"))]
#[case::spent("SVar:X:Count$CastTotalManaSpent", json!({"ManaSpent": {"of": null}}))]
#[case::starting_life("SVar:X:Count$YourStartingLife", json!({"StartingLife": "You"}))]
#[case::library(
    "SVar:X:Count$ValidLibrary Card.YouOwn",
    json!({"Count": [{"OwnedBy": "You"}, {"In": "Library"}]})
)]
#[case::entered(
    "SVar:X:Count$ThisTurnEntered_Battlefield_Creature.YouCtrl",
    json!({"Tally": {"event": "Entered", "who": "EachPlayer", "filter": [{"Type": "Creature"}, {"ControlledBy": "You"}]}})
)]
#[case::died(
    "SVar:X:Count$ThisTurnEntered_Graveyard_from_Battlefield_Creature",
    json!({"Tally": {"event": "Died", "who": "EachPlayer", "filter": [{"Type": "Creature"}]}})
)]
#[case::adamant(
    "SVar:X:Count$Adamant.Red.3.1",
    json!({"When": {"condition": {"Compare": {"left": {"ManaSpent": {"of": "Red"}}, "op": "Ge", "right": 3}}, "then": 3, "otherwise": 1}})
)]
#[case::half_up(
    "SVar:X:Count$YourStartingLife/HalfUp",
    json!({"Half": {"of": {"StartingLife": "You"}, "up": true}})
)]
#[case::half_down_of_part(
    "SVar:X:SVar$Y/HalfDown\nSVar:Y:Count$Party",
    json!({"Half": {"of": {"Party": "You"}, "up": false}})
)]
#[case::their_hand(
    "SVar:X:TargetedPlayer$CardsInHand",
    json!({"Count": [{"In": "Hand"}, {"OwnedBy": {"Target": 0}}]})
)]
#[case::devotion("SVar:X:Count$Devotion.Black", json!({"Devotion": ["Black"]}))]
#[case::devotion_dual(
    "SVar:X:Count$DevotionDual.White.Black",
    json!({"Devotion": ["White", "Black"]})
)]
#[case::capped(
    "SVar:X:Count$YourStartingLife/LimitMax.5",
    json!({"When": {"condition": {"Compare": {"left": {"StartingLife": "You"}, "op": "Gt", "right": 5}}, "then": 5, "otherwise": {"StartingLife": "You"}}})
)]
#[case::raised(
    "SVar:X:Count$Party/LimitMin.1",
    json!({"When": {"condition": {"Compare": {"left": {"Party": "You"}, "op": "Lt", "right": 1}}, "then": 1, "otherwise": {"Party": "You"}}})
)]
#[case::from_a_number(
    "SVar:X:TargetedPlayer$CardsInHand/NMinus.3",
    json!({"Sum": [3, {"Times": {"by": -1, "of": {"Count": [{"In": "Hand"}, {"OwnedBy": {"Target": 0}}]}}}]})
)]
#[case::their_life("SVar:X:TargetedPlayer$LifeTotal", json!({"Life": {"Target": 0}}))]
#[case::opponents_hands(
    "SVar:X:PlayerCountOpponents$HighestCardsInHand",
    json!({"Largest": {"who": "Opponents", "of": [{"In": "Hand"}, {"OwnedBy": "You"}]}})
)]
#[case::everyones_lands(
    "SVar:X:PlayerCountPlayers$HighestValid Land.YouCtrl",
    json!({"Largest": {"who": "EachPlayer", "of": [{"Type": "Land"}, {"ControlledBy": "You"}]}})
)]
#[case::discarded(
    "SVar:X:PlayerCountPropertyYou$CardsDiscardedThisTurn",
    json!({"Tallied": {"who": "You", "what": "Discarded"}})
)]
fn game_facts_and_player_counts(#[case] svars: &str, #[case] expected: Value) {
    let found = count(&drawing(svars));
    assert_eq!(found, expected);
    assert!(serde_json::from_value::<Amount>(found).is_ok());
}

#[rstest]
#[case::entered_elsewhere(
    "SVar:X:Count$ThisTurnEntered_Hand_from_Battlefield_Permanent.YouOwn",
    "amount:Count$ThisTurnEntered_Hand_from_Battlefield_Permanent"
)]
#[case::unknown_token_property(
    "SVar:X:Count$ThisTurnEntered_Battlefield_Card.tokenMadeUp+YouOwn",
    "valid:tokenMadeUp"
)]
#[case::spent_of_a_kind(
    "SVar:X:Count$CastTotalManaSpent Treasure",
    "amount:Count$CastTotalManaSpent"
)]
#[case::highest_of_the_opponents(
    "SVar:X:PlayerCountOpponents$HighestValid Land.OppCtrl",
    "amount:Highest-of-the-opponents"
)]
#[case::adamant_color("SVar:X:Count$Adamant.Purple.1.0", "amount:Count$Adamant-color")]
#[case::unknown_player_word("SVar:X:TargetedPlayer$Handsize", "amount:TargetedPlayer$Handsize")]
fn what_the_dsl_cannot_count_is_a_gap(#[case] svars: &str, #[case] gap: &str) {
    assert_eq!(gaps(&drawing(svars)), vec![gap.to_owned()]);
}

#[test]
fn a_negated_part_names_the_gap_of_the_part() {
    let text = "Name:T\nTypes:Sorcery\nA:SP$ Pump | ValidTgts$ Creature | NumAtt$ -X | NumDef$ -X\nSVar:X:Count$Bogus\n";
    assert_eq!(gaps(text), vec!["amount:Count$Bogus".to_owned()]);
}

#[test]
fn the_player_of_a_trigger_names_the_player_of_the_event() {
    let script = "Name:T\nTypes:Enchantment\nT:Mode$ LifeGained | ValidPlayer$ Player | TriggerZones$ Battlefield | Execute$ TrigDraw\nSVar:TrigDraw:DB$ Draw | NumCards$ X\nSVar:X:TriggeredPlayer$CardsInHand\n";
    let found = abilities(script);
    assert_eq!(found.len(), 1, "{:?}", gaps(script));
    let text = found[0].to_string();
    assert!(
        text.contains(r#"{"Count":[{"In":"Hand"},{"OwnedBy":"Triggering"}]}"#),
        "{text}"
    );
}
