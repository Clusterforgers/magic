//! Goal: the pieces of ep30 translate as the card prints them. "Remove up to N counters" keeps the controller's stop
//! (`up_to`) for counters of any kind and of one kind, and "up to all" stays a gap; "move any number of" counters and
//! moves from each of a kind of permanent (`ValidSource$`) read as such, a kind of source beside a target stays a gap; a
//! "you may ... any number of" cost with its announced count is a repeatable additional cost whose payments the spell
//! reads (`AdditionalPaid`); a shield used up by what it covers over two recipients is one `Union`, over three a gap.
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

fn mover(line: &str) -> String {
    format!(
        "Name:T\nTypes:Creature Beast\nPT:1/1\nT:Mode$ ChangesZone | Origin$ Any | Destination$ Battlefield | ValidCard$ Card.Self | Execute$ TrigMove | TriggerDescription$ When CARDNAME enters, move counters.\nSVar:TrigMove:DB$ MoveCounter | {line}\n"
    )
}

#[rstest]
#[case::any_number_from_others(
    "ValidSource$ Permanent.YouCtrl+Other+counters_GE1_P1P1 | Defined$ Self | CounterType$ P1P1 | CounterNum$ Any",
    json!({"MoveCounters": {"from": {"Each": [{"ControlledBy": "You"}, "Other"]}, "to": "This", "counter": "+1/+1", "count": {"Counters": {"on": {"Each": [{"ControlledBy": "You"}, "Other"]}, "counter": "+1/+1"}}, "up_to": true}})
)]
#[case::any_number_of_any_kind_onto_a_target(
    "ValidTgts$ Creature | Source$ Self | CounterType$ Any | CounterNum$ Any",
    json!({"MoveCounters": {"from": "This", "to": {"Target": 0}, "count": {"AnyCounters": "This"}, "up_to": true}})
)]
#[case::all_of_a_kind_onto_a_target(
    "ValidTgts$ Creature | Source$ Self | CounterType$ P1P1 | CounterNum$ All",
    json!({"MoveCounters": {"from": "This", "to": {"Target": 0}, "counter": "+1/+1", "count": {"Counters": {"on": "This", "counter": "+1/+1"}}}})
)]
#[case::all_from_all_creatures(
    "ValidSource$ Creature | Defined$ Self | CounterType$ P1P1 | CounterNum$ All",
    json!({"MoveCounters": {"from": {"Each": [{"Type": "Creature"}]}, "to": "This", "counter": "+1/+1", "count": {"Counters": {"on": {"Each": [{"Type": "Creature"}]}, "counter": "+1/+1"}}}})
)]
fn ep30_moves_of_any_number_and_from_each(#[case] line: &str, #[case] effect: serde_json::Value) {
    let script = mover(line);
    assert_eq!(gaps(&script), Vec::<String>::new());
    let found = abilities(&script);
    assert_eq!(found[0]["Triggered"]["effects"][0], effect, "{found:?}");
}

#[test]
fn ep30_a_kind_source_beside_a_target_stays_a_gap() {
    let found = gaps(&mover(
        "ValidSource$ Creature | ValidTgts$ Creature | CounterType$ P1P1 | CounterNum$ All",
    ));
    assert!(
        found
            .iter()
            .any(|gap| gap.contains("MoveCounter.ValidSource")),
        "{found:?}"
    );
}

fn shield(valid_target: &str, replace: &str) -> String {
    format!(
        "Name:T\nTypes:Instant\nA:SP$ Effect | ValidTgts$ Any | ReplacementEffects$ ReDamage | RememberObjects$ Targeted | SpellDescription$ Damage goes elsewhere.\nSVar:ReDamage:Event$ DamageDone | ActiveZones$ Command | ValidTarget$ {valid_target} | ReplaceWith$ Split | DamageTarget$ Remembered | Description$ The next 2 damage that would be dealt to you and/or permanents you control this turn is dealt to any target instead.\nSVar:Split:DB$ ReplaceSplitDamage | DamageTarget$ Remembered | VarName$ ShieldAmount{replace}\nSVar:ShieldAmount:Number$2\n"
    )
}

#[rstest]
#[case::shared(" | DivideShield$ True")]
#[case::plain("")]
fn ep30_a_used_up_shield_over_two_recipients_is_one_union(#[case] divide: &str) {
    let script = shield("You,Permanent.YouCtrl", divide);
    assert_eq!(gaps(&script), Vec::<String>::new());
    let found = abilities(&script);
    let effects = &found[0]["Spell"]["effects"];
    assert_eq!(effects.as_array().map(Vec::len), Some(1), "{found:?}");
    assert_eq!(
        effects[0]["Shield"]["when"]["Damage"]["to"],
        json!({"Union": ["You", {"Each": [{"ControlledBy": "You"}]}]})
    );
}

#[test]
fn ep30_three_recipients_of_a_used_up_shield_stay_a_gap() {
    let found = gaps(&shield("You,Creature.YouCtrl,Planeswalker.YouCtrl", ""));
    assert!(
        found.iter().any(|gap| gap.contains("shield-recipient")),
        "{found:?}"
    );
}

#[test]
fn ep30_up_to_all_stays_a_gap() {
    let found = gaps(&removal("CHARGE", "All"));
    assert!(
        found.iter().any(|gap| gap.contains("RemoveCounter.UpTo")),
        "{found:?}"
    );
}
