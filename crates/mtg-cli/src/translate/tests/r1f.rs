//! Goal: the replacement shapes of an enters-tapped-with-counters chain, a damage shield over a group or a list of
//! recipients, an untap that does not happen, a card that goes to the hand or exile instead of dying, a skipped step and
//! a life floor read as the DSL says them, and a neighbouring shape the DSL cannot say stays a named gap.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use rstest::rstest;
use serde_json::{Value, json};

use super::{abilities, gaps, normal};

fn one(text: &str) -> Value {
    let mut found = abilities(text);
    assert_eq!(found.len(), 1, "{:?} {found:?}", gaps(text));
    normal(found.remove(0))
}

#[rstest]
#[case::tap_first("DB$ Tap | Defined$ Self | ETB$ True | SubAbility$ C", "DB$ PutCounter | Defined$ Self | ETB$ True | CounterType$ DEPLETION | CounterNum$ 2", "depletion", 2)]
#[case::counters_first("DB$ PutCounter | Defined$ Self | ETB$ True | CounterType$ STUN | CounterNum$ 3 | SubAbility$ C", "DB$ Tap | Defined$ Self | ETB$ True", "stun", 3)]
fn a_permanent_enters_tapped_with_counters(
    #[case] first: &str,
    #[case] second: &str,
    #[case] kind: &str,
    #[case] count: u32,
) {
    let text = format!(
        "Name:T\nTypes:Land\nR:Event$ Moved | ValidCard$ Card.Self | Destination$ Battlefield | ReplaceWith$ A | ReplacementResult$ Updated\nSVar:A:{first}\nSVar:C:{second}\n"
    );
    let want = json!({"Replacement": {"when": {"Enters": {"what": ["This"]}},
        "by": {"WithCounters": {"counter": {"Other": kind}, "count": count, "tapped": true}}}});
    assert_eq!(one(&text), normal(want));
}

#[test]
fn an_entering_chain_with_another_part_stays_a_gap() {
    let text = "Name:T\nTypes:Land\nR:Event$ Moved | ValidCard$ Card.Self | Destination$ Battlefield | ReplaceWith$ A | ReplacementResult$ Updated\nSVar:A:DB$ Tap | Defined$ Self | ETB$ True | SubAbility$ C\nSVar:C:DB$ Draw | Defined$ You\n";
    assert_eq!(abilities(text), Vec::<Value>::new());
    assert_ne!(gaps(text), Vec::<String>::new());
}

fn shield(target: &str, extra: &str) -> String {
    format!(
        "Name:T\nTypes:Instant\nA:SP$ Effect | ReplacementEffects$ R\nSVar:R:Event$ DamageDone | Prevent$ True | ActiveZones$ Command | ValidTarget$ {target}{extra}\n"
    )
}

#[rstest]
#[case::creatures("Creature", vec![json!({"Each": [{"Type": "Creature"}]})])]
#[case::yours("Creature.YouCtrl", vec![json!({"Each": [{"Type": "Creature"}, {"ControlledBy": "You"}]})])]
#[case::you_and_yours("You,Creature.YouCtrl", vec![json!("You"), json!({"Each": [{"Type": "Creature"}, {"ControlledBy": "You"}]})])]
fn a_prevention_shield_covers_each_recipient(#[case] target: &str, #[case] to: Vec<Value>) {
    let text = shield(target, "");
    let found: Vec<Value> = abilities(&text).into_iter().map(normal).collect();
    let want: Vec<Value> = to
        .into_iter()
        .map(|to| {
            normal(json!({"Shield": {"when": {"Damage": {"to": to}}, "by": "Prevent", "duration": "UntilEndOfTurn"}}))
        })
        .collect();
    assert_eq!(found, want, "{:?}", gaps(&text));
}

#[test]
fn a_redirect_over_a_list_of_recipients_stays_a_gap() {
    let text = shield("You,Creature.YouCtrl", " | ReplaceWith$ X\nSVar:X:DB$ DealDamage");
    assert_eq!(abilities(&text), Vec::<Value>::new());
}

fn untap(line: &str, rule: &str) -> String {
    format!(
        "Name:T\nTypes:Creature Elf\nPT:1/1\nA:AB$ Effect | Cost$ T | {line} | ReplacementEffects$ D\nSVar:D:Event$ Untap | {rule} | Layer$ CantHappen\n"
    )
}

#[test]
fn lands_do_not_untap_through_the_next_untap_step() {
    let text = untap(
        "Duration$ UntilTheEndOfYourNextUntap",
        "ValidCard$ Land.YouCtrl | ValidStepTurnToController$ Player.Activator",
    );
    let want = json!({"Continuous": {"affected": {"Each": [{"Type": "Land"}, {"ControlledBy": "You"}]},
        "mods": ["DoesntUntap"], "duration": "NextUntapStep"}});
    assert_eq!(one(&text), normal(want));
}

#[rstest]
#[case::control("AsLongAsControl", "WhileYouControlSource")]
#[case::in_play("AsLongAsInPlay", "WhileSourceOnBattlefield")]
fn a_remembered_creature_does_not_untap_for_a_while(#[case] duration: &str, #[case] word: &str) {
    let text = untap(
        &format!("ValidTgts$ Creature | RememberObjects$ Targeted | Duration$ {duration}"),
        "ValidCard$ Card.IsRemembered | ValidStepTurnToController$ You",
    );
    let found = one(&text);
    assert_eq!(found["Continuous"]["mods"], json!(["DoesntUntap"]), "{found}");
    assert_eq!(found["Continuous"]["duration"], json!(word), "{found}");
}

#[test]
fn cant_become_untapped_stays_a_gap() {
    let text = untap(
        "ValidTgts$ Creature | RememberObjects$ Targeted | Duration$ AsLongAsControl",
        "ValidCard$ Card.IsRemembered",
    );
    assert_eq!(abilities(&text), Vec::<Value>::new());
    assert!(gaps(&text).contains(&"replacement:Untap-shape".to_string()));
}

#[rstest]
#[case::exile("DB$ ChangeZone | Origin$ Battlefield | Destination$ Exile | Defined$ ReplacedCard", "Exile")]
#[case::hand("DB$ ChangeZone | Origin$ Battlefield | Destination$ Hand | Defined$ ReplacedCard", "Hand")]
fn a_creature_that_would_die_goes_elsewhere(#[case] part: &str, #[case] to: &str) {
    let text = format!(
        "Name:T\nTypes:Enchantment\nR:Event$ Moved | ActiveZones$ Battlefield | Origin$ Battlefield | Destination$ Graveyard | ValidLKI$ Creature.OppCtrl | ReplaceWith$ X\nSVar:X:{part}\n"
    );
    let want = json!({"Replacement": {"when": {"ZoneChange": {
        "what": [{"Type": "Creature"}, {"ControlledBy": "Opponents"}], "from": "Battlefield", "to": "Graveyard"}},
        "by": {"MoveTo": to}}});
    assert_eq!(one(&text), normal(want));
}

#[test]
fn a_creature_damaged_by_the_source_is_exiled_instead_of_dying() {
    let text = "Name:T\nTypes:Creature Elf\nPT:1/1\nR:Event$ Moved | ValidLKI$ Creature.DamagedBy | Destination$ Graveyard | ActiveZones$ Battlefield | ReplaceWith$ X\nSVar:X:DB$ ChangeZone | Defined$ ReplacedCard | Origin$ Battlefield | Destination$ Exile\n";
    let found = one(text);
    let pattern = &found["Replacement"]["when"]["ZoneChange"];
    assert_eq!(pattern["from"], json!("Battlefield"), "{found}");
    assert_eq!(pattern["what"][1], json!({"DamagedBy": "This"}), "{found}");
}

#[rstest]
#[case::each("", "EachPlayer")]
#[case::you(" | ValidPlayer$ You", "You")]
fn a_step_is_skipped(#[case] who: &str, #[case] expected: &str) {
    let text = format!(
        "Name:T\nTypes:Enchantment\nR:Event$ BeginPhase | ActiveZones$ Battlefield | Phase$ Draw | Skip$ True{who}\n"
    );
    let want = json!({"Rule": {"rule": {"Cant": {"who": expected, "action": {"SkipStep": "Draw"}}}}});
    assert_eq!(one(&text), normal(want));
}

#[test]
fn a_skipped_step_with_a_condition_stays_a_gap() {
    let text = "Name:T\nTypes:Enchantment\nR:Event$ BeginPhase | ActiveZones$ Battlefield | ValidPlayer$ You | Phase$ Upkeep | Skip$ True | Hellbent$ True\n";
    assert_eq!(abilities(text), Vec::<Value>::new());
}

fn floor_card(n: u32, extra: &str) -> String {
    format!(
        "Name:T\nTypes:Enchantment\nR:Event$ LifeReduced | ActiveZones$ Battlefield | ValidPlayer$ You.lifeGE{n} | Result$ LT{n} | IsDamage$ True{extra} | ReplaceWith$ Cut\nSVar:Cut:DB$ ReplaceEffect | VarName$ Amount | VarValue$ X\nSVar:X:ReplaceCount$Amount/LimitMax.Difference\nSVar:Difference:Count$YourLifeTotal/Minus.{n}\n"
    )
}

#[rstest]
#[case::one(1)]
#[case::seven(7)]
fn damage_cannot_take_life_below_the_floor(#[case] n: u32) {
    let want = json!({"Replacement": {"when": {"Damage": {"to": "You"}}, "by": {"LifeFloor": n}}});
    assert_eq!(one(&floor_card(n, "")), normal(want));
}

#[test]
fn a_life_floor_keeps_its_condition() {
    let found = one(&floor_card(1, " | IsPresent$ Creature.YouCtrl"));
    assert!(found["Replacement"].get("condition").is_some(), "{found}");
}

#[test]
fn a_floor_that_differs_from_its_amount_stays_a_gap() {
    let text = floor_card(1, "").replace("Minus.1", "Minus.2");
    assert_eq!(abilities(&text), Vec::<Value>::new());
}
