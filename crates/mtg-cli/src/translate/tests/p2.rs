//! Goal: piece P2 of the rules plan translates its constructs, one rstest table per rule.
use rstest::rstest;
use serde_json::{Value, json};

use super::{abilities, gaps};

/// A spell that draws X cards; it targets a creature only when the parts read a target.
fn drawing(svars: &str) -> String {
    if svars.contains("Targeted") {
        return format!(
            "Name:T\nTypes:Sorcery\nA:SP$ Pump | ValidTgts$ Creature | SubAbility$ D\nSVar:D:DB$ Draw | NumCards$ X\n{svars}\n"
        );
    }
    let draw = if svars.contains("NumCards$") {
        ""
    } else {
        "| NumCards$ X "
    };
    format!("Name:T\nTypes:Sorcery\nA:SP$ Draw {draw}\n{svars}\n").replace(" \n", "\n")
}

fn drawn(text: &str) -> Value {
    let found = abilities(text);
    assert!(!found.is_empty(), "{:?}", gaps(text));
    found[0]["Spell"]["effects"]
        .as_array()
        .and_then(|effects| effects.iter().find_map(|e| e.get("Draw")))
        .map_or(Value::Null, |draw| draw["count"].clone())
}

#[rstest]
#[case::colors("Targeted$CardNumColors", json!({"Stat": {"of": {"Target": 0}, "stat": "Colors"}}))]
#[case::spent("Targeted$CastTotalManaSpent", json!({"Stat": {"of": {"Target": 0}, "stat": "ManaSpent"}}))]
#[case::lki("Targeted$CardPowerLKI", json!({"Stat": {"of": {"Target": 0}, "stat": "Power"}}))]
#[case::counters(
    "Targeted$CardCounters.P1P1",
    json!({"Counters": {"on": {"Target": 0}, "counter": "+1/+1"}})
)]
fn object_stat(#[case] expression: &str, #[case] expected: Value) {
    assert_eq!(drawn(&drawing(&format!("SVar:X:{expression}"))), expected);
}

#[rstest]
#[case("Targeted$CardBogus", "amount:Targeted$CardBogus")]
#[case(
    "Targeted$CardCounters.bad kind",
    "amount:Targeted$CardCounters.bad kind"
)]
fn object_stat_gap(#[case] expression: &str, #[case] gap: &str) {
    assert_eq!(
        gaps(&drawing(&format!("SVar:X:{expression}"))),
        vec![gap.to_owned()]
    );
}

#[rstest]
#[case::hand(
    "SVar:X:Count$CardsInYourHand",
    json!({"Count": [{"In": "Hand"}, {"OwnedBy": "You"}]})
)]
#[case::commander(
    "SVar:X:Count$TotalCommanderCastFromCommandZone",
    json!({"CommanderCasts": "You"})
)]
#[case::energy(
    "SVar:X:Count$YourCountersEnergy",
    json!({"Counters": {"on": "You", "counter": {"Other": "energy"}}})
)]
#[case::controller_hand(
    "SVar:X:TargetedController$CardsInHand",
    json!({"Count": [{"In": "Hand"}, {"OwnedBy": {"ControllerOf": {"Target": 0}}}]})
)]
#[case::number_steps("SVar:X:Number$2/Twice", json!({"Times": {"by": 2, "of": 2}}))]
#[case::card_types(
    "SVar:X:Count$ValidGraveyard Card.YouOwn$CardTypes",
    json!({"CardTypes": [{"OwnedBy": "You"}, {"In": "Graveyard"}]})
)]
#[case::scried(
    "SVar:X:Count$YouScryThisTurn",
    json!({"Tallied": {"who": "You", "what": "Scried"}})
)]
#[case::surveilled(
    "SVar:X:Count$YouSurveilThisTurn",
    json!({"Tallied": {"who": "You", "what": "Surveilled"}})
)]
#[case::sacrificed(
    "SVar:X:PlayerCountPropertyYou$SacrificedThisTurn",
    json!({"Tallied": {"who": "You", "what": "Sacrificed"}})
)]
#[case::life_lost(
    "SVar:X:Count$LifeYouLostThisTurn",
    json!({"Tallied": {"who": "You", "what": "LifeLost"}})
)]
#[case::battlefield(
    "SVar:X:Count$ValidBattlefield Land.YouCtrl",
    json!({"Count": [{"Type": "Land"}, {"ControlledBy": "You"}]})
)]
#[case::most_poison(
    "SVar:X:PlayerCountOpponents$HighestCounters.Poison",
    json!({"HighestCounters": {"who": "Opponents", "counter": "poison"}})
)]
#[case::most_poison_of_all(
    "SVar:X:PlayerCountPlayers$HighestCounters.Poison",
    json!({"HighestCounters": {"who": "EachPlayer", "counter": "poison"}})
)]
#[case::all_poison_is_a_sum(
    "SVar:X:Count$TotalOppPoisonCounters",
    json!({"Counters": {"on": "Opponents", "counter": "poison"}})
)]
#[case::mana_pool("SVar:X:Count$ManaPool:All", json!({"ManaInPool": "You"}))]
#[case::left_graveyard(
    "SVar:X:Count$LeftGraveyardThisTurn Card.YouOwn",
    json!({"Tallied": {"who": "You", "what": "CardsLeftGraveyard"}})
)]
#[case::chroma("SVar:X:Count$Chroma.Red", json!({"Devotion": ["Red"]}))]
#[case::cast_from_graveyard(
    "SVar:X:Count$wasCastFromGraveyard.2.1",
    json!({"When": {"condition": {"Matches": {"subject": "This",
        "selector": [{"WasCast": {"from": "Graveyard"}}]}}, "then": 2, "otherwise": 1}})
)]
fn game_counts(#[case] svars: &str, #[case] expected: Value) {
    assert_eq!(drawn(&drawing(svars)), expected);
}

#[test]
fn a_value_written_in_place_is_the_expression() {
    let script = "Name:T\nTypes:Sorcery\nA:SP$ Draw | NumCards$ Count$CardsInYourHand/Thrice\n";
    assert_eq!(
        drawn(script),
        json!({"Times": {"by": 3, "of": {"Count": [{"In": "Hand"}, {"OwnedBy": "You"}]}}})
    );
}

#[rstest]
#[case("SVar:X:Count$Void.1.0", "amount:Count$Void")]
#[case("SVar:X:Count$YourCountersBogus", "amount:Count$YourCountersBogus")]
#[case("SVar:X:Count$ManaPool:green", "amount:Count$ManaPool:green")]
#[case(
    "SVar:X:Count$LeftGraveyardThisTurn Creature.YouOwn",
    "amount:Count$LeftGraveyardThisTurn"
)]
#[case("SVar:X:Count$Chroma.Bogus", "amount:Count$Devotion")]
#[case(
    "SVar:X:Count$wasCastFromTheirHand.2.1",
    "amount:Count$wasCastFromTheirHand"
)]
#[case(
    "SVar:X:PlayerCountOpponents$HighestCounters.Bogus kind",
    "amount:HighestCounters.Bogus kind"
)]
#[case(
    "SVar:X:Count$ValidGraveyard Card.YouOwn$LeastCardPower",
    "amount:Count$ValidGraveyard$LeastCardPower"
)]
fn refused_amounts(#[case] svars: &str, #[case] gap: &str) {
    assert_eq!(gaps(&drawing(svars)), vec![gap.to_owned()]);
}

#[test]
fn triggered_attacker_power() {
    let script = "Name:T\nTypes:Enchantment\nT:Mode$ Attacks | ValidCard$ Creature | TriggerZones$ Battlefield | Execute$ TrigDraw\nSVar:TrigDraw:DB$ Draw | NumCards$ X\nSVar:X:TriggeredAttacker$CardPower\n";
    let text = abilities(script)
        .first()
        .map(ToString::to_string)
        .unwrap_or_default();
    assert!(
        text.contains(r#""stat":"Power""#),
        "{text} {:?}",
        gaps(script)
    );
}
