//! Goal: the pieces of ep30 translate as the card prints them. "Remove up to N counters" keeps the controller's stop
//! (`up_to`) for counters of any kind and of one kind; "up to all" stays a gap.
use rstest::rstest;
use serde_json::json;

use super::{abilities, gaps};

fn removal(kind: &str, num: &str) -> String {
    format!(
        "Name:T\nTypes:Sorcery\nA:SP$ RemoveCounter | ValidTgts$ Permanent | CounterType$ {kind} | CounterNum$ {num} | UpTo$ True | SpellDescription$ Remove up to some counters from target permanent.\n"
    )
}

#[rstest]
#[case::any_kind("Any", "5", json!({"RemoveCounters": {"from": {"Target": 0}, "count": 5, "up_to": true}}))]
#[case::one_kind(
    "CHARGE",
    "4",
    json!({"RemoveCounters": {"from": {"Target": 0}, "counter": "charge", "count": 4, "up_to": true}})
)]
fn ep30_up_to_counters_keep_the_stop(
    #[case] kind: &str,
    #[case] num: &str,
    #[case] effect: serde_json::Value,
) {
    let script = removal(kind, num);
    assert_eq!(gaps(&script), Vec::<String>::new());
    assert_eq!(abilities(&script)[0]["Spell"]["effects"][0], effect);
}

const MARCH: &str = "Name:T\nManaCost:X W\nTypes:Instant\nA:SP$ ChangeZone | AnnounceTitle$ how many white cards to exile | Announce$ Exiled | Origin$ Battlefield | Destination$ Exile | ValidTgts$ Creature.cmcLEX | SpellDescription$ Exile target creature with mana value X or less.\nS:Mode$ RaiseCost | ValidCard$ Card.Self | Type$ Spell | Cost$ ExileFromHand<Y/Card.White/white cards> | EffectZone$ All | Description$ As an additional cost to cast this spell, you may exile any number of white cards from your hand. This spell costs {2} less to cast for each card exiled this way.\nS:Mode$ ReduceCost | ValidCard$ Card.Self | Type$ Spell | Amount$ Z | EffectZone$ All | Relative$ True | Secondary$ True | Description$ This spell costs {2} less to cast for each card exiled this way.\nSVar:X:Count$xPaid\nSVar:Y:SVar$Exiled\nSVar:Z:SVar$Y/Times.2\nSVar:Exiled:Number$0\n";

#[test]
fn ep30_any_number_of_cards_is_a_repeatable_cost_with_its_reduction() {
    assert_eq!(gaps(MARCH), Vec::<String>::new());
    let found = abilities(MARCH);
    assert!(
        found.contains(&json!({"AdditionalCost": {"cost": [{"ExileFromHand": {"filter": [{"Color": "White"}]}}], "repeatable": true}})),
        "{found:?}"
    );
}

const TASTE: &str = "Name:T\nManaCost:3 G\nTypes:Sorcery\nS:Mode$ RaiseCost | ValidCard$ Card.Self | Type$ Spell | Cost$ 1 G | Amount$ PseudoKicker | EffectZone$ All | Description$ As an additional cost to cast this spell, you may pay {1}{G} any number of times.\nA:SP$ GainLife | LifeAmount$ SVar$X/Times.3 | Announce$ PseudoKicker | SpellDescription$ You gain 3 life plus an additional 3 life for each additional {1}{G} you paid.\nSVar:X:SVar$PseudoKicker/Plus.1\nSVar:PseudoKicker:Number$0\n";

#[test]
fn ep30_mana_any_number_of_times_is_counted() {
    assert_eq!(gaps(TASTE), Vec::<String>::new());
}

#[test]
fn ep30_up_to_all_stays_a_gap() {
    let found = gaps(&removal("CHARGE", "All"));
    assert!(
        found.iter().any(|gap| gap.contains("RemoveCounter.UpTo")),
        "{found:?}"
    );
}
