//! Goal: round 3 piece P4 reads what steps remember and what lines say about their own targets: the count of remembered objects, the controller they had, the exact mana cost, name sharing, emblem sources and the objects a delayed trigger is about.
#![allow(clippy::unwrap_used)]

use rstest::rstest;

use super::{abilities, gaps};

/// The translation of a sorcery with the given lines as compact JSON, and its gaps.
fn sorcery(lines: &str) -> (String, Vec<String>) {
    let text = format!("Name:Subject\nTypes:Sorcery\n{lines}\n");
    (
        serde_json::to_string(&abilities(&text)).unwrap(),
        gaps(&text),
    )
}

/// The translation of a creature with the given lines.
fn creature(lines: &str) -> (String, Vec<String>) {
    let text = format!("Name:Subject\nTypes:Creature Elf\nPT:1/1\n{lines}\n");
    (
        serde_json::to_string(&abilities(&text)).unwrap(),
        gaps(&text),
    )
}

#[rstest]
#[case::destroyed_all_then_counters(
    "T:Mode$ ChangesZone | Origin$ Any | Destination$ Battlefield | ValidCard$ Card.Self | Execute$ D\nSVar:D:DB$ DestroyAll | ValidCards$ Artifact,Enchantment | RememberDestroyed$ True | SubAbility$ C\nSVar:C:DB$ PutCounter | CounterType$ P1P1 | CounterNum$ X | SubAbility$ Z\nSVar:Z:DB$ Cleanup | ClearRemembered$ True\nSVar:X:Remembered$Amount",
    "destroyed"
)]
#[case::moved_all_then_draw(
    "T:Mode$ SpellCast | ValidCard$ Card | ValidActivatingPlayer$ You | Execute$ D\nSVar:D:DB$ ChangeZoneAll | Origin$ Hand | Destination$ Library | ChangeType$ Card.YouOwn | RememberChanged$ True | LibraryPosition$ -1 | SubAbility$ W\nSVar:W:DB$ Draw | NumCards$ X | SubAbility$ Z\nSVar:Z:DB$ Cleanup | ClearRemembered$ True\nSVar:X:Remembered$Amount",
    "moved"
)]
#[case::exiled_twice_over(
    "T:Mode$ ChangesZone | Origin$ Any | Destination$ Battlefield | ValidCard$ Card.Self | Execute$ D\nSVar:D:DB$ ChangeZone | Origin$ Battlefield | Destination$ Hand | ChangeType$ Plains.YouCtrl | ChangeNum$ 3 | Hidden$ True | RememberChanged$ True | SubAbility$ W\nSVar:W:DB$ GainLife | LifeAmount$ Y | SubAbility$ Z\nSVar:Z:DB$ Cleanup | ClearRemembered$ True\nSVar:Y:Remembered$Amount/Twice",
    "moved"
)]
fn the_amount_of_remembered_objects_counts_them_in_whatever_zone_they_went_to(
    #[case] lines: &str,
    #[case] expected: &str,
) {
    let (json, found) = creature(lines);
    assert!(found.is_empty(), "{found:?}");
    assert!(json.contains(expected) && json.contains("~moved"), "{json}");
    assert!(json.contains(expected), "{json}");
}

#[test]
fn the_amount_of_nothing_remembered_stays_a_gap() {
    let (_, found) = sorcery("A:SP$ Draw | NumCards$ X\nSVar:X:Remembered$Amount");
    assert_eq!(found, ["amount:Remembered-without-a-change"]);
}

#[rstest]
#[case::exiled_target_controller(
    "A:SP$ ChangeZone | Origin$ Battlefield | Destination$ Exile | ValidTgts$ Permanent.nonLand | RememberLKI$ True | SubAbility$ D\nSVar:D:DB$ Draw | Defined$ RememberedController | NumCards$ 1 | SubAbility$ Z\nSVar:Z:DB$ Cleanup | ClearRemembered$ True",
    r#""Draw":{"who":{"ControllerOf":{"Target":0}}}"#
)]
#[case::bounced_target_controller(
    "A:SP$ ChangeZone | Origin$ Battlefield | Destination$ Hand | ValidTgts$ Creature | RememberChanged$ True | SubAbility$ D\nSVar:D:DB$ GainLife | Defined$ RememberedController | LifeAmount$ 2 | SubAbility$ Z\nSVar:Z:DB$ Cleanup | ClearRemembered$ True",
    r#""GainLife":{"amount":2,"who":{"ControllerOf":{"Target":0}}}"#
)]
fn the_controller_of_what_a_step_moved_is_read_on_the_target_it_named(
    #[case] lines: &str,
    #[case] expected: &str,
) {
    let (json, found) = sorcery(lines);
    assert!(found.is_empty(), "{found:?}");
    assert!(json.contains(expected), "{json}");
}

#[rstest]
#[case::destroyed_may_not_be_destroyed(
    "A:SP$ Destroy | ValidTgts$ Artifact | RememberDestroyed$ True | SubAbility$ D\nSVar:D:DB$ Draw | Defined$ RememberedController | SubAbility$ Z\nSVar:Z:DB$ Cleanup | ClearRemembered$ True"
)]
#[case::moved_by_a_sweep(
    "A:SP$ ChangeZoneAll | Origin$ Battlefield | Destination$ Exile | ChangeType$ Creature | RememberChanged$ True | SubAbility$ D\nSVar:D:DB$ Draw | Defined$ RememberedController | SubAbility$ Z\nSVar:Z:DB$ Cleanup | ClearRemembered$ True"
)]
fn the_controller_of_what_cannot_be_traced_to_a_target_stays_a_gap(#[case] lines: &str) {
    let (_, found) = sorcery(lines);
    assert_eq!(found, ["defined:RememberedController"]);
}

#[rstest]
#[case::checked_amount(
    "A:SP$ ChangeZone | ValidTgts$ Creature | Origin$ Graveyard | Destination$ Battlefield | ConditionCheckSVar$ Y | ConditionSVarCompare$ LEX\nSVar:X:Count$Valid Creature.YouCtrl\nSVar:Y:Targeted$CardManaCost",
    r#""Stat":{"of":{"Target":0},"stat":"ManaValue"}"#
)]
#[case::present_in_the_chain(
    "A:SP$ ChangeZone | ValidTgts$ Permanent | Origin$ Battlefield | Destination$ Hand | ConditionDefined$ Targeted | ConditionPresent$ Card.Green | ConditionCompare$ EQ0",
    r#""CountOf":{"op":"Eq","right":0,"selector":[{"Color":"Green"}],"subject":{"Target":0}}"#
)]
fn a_condition_on_the_line_that_picks_a_target_reads_that_target(
    #[case] lines: &str,
    #[case] expected: &str,
) {
    let (json, found) = sorcery(lines);
    assert!(found.is_empty(), "{found:?}");
    assert!(json.contains(expected), "{json}");
}

#[test]
fn a_condition_on_a_targeted_line_with_nothing_declared_stays_a_gap() {
    let (_, found) = sorcery(
        "A:SP$ GainLife | LifeAmount$ 2 | ConditionDefined$ Targeted | ConditionPresent$ Creature",
    );
    assert_eq!(found, ["defined:Targeted-without-target"]);
}

#[rstest]
#[case::zero("ManaCost0", &[r#"{"GenericCost":0}"#])]
#[case::one("ManaCost1", &[r#"{"GenericCost":1}"#])]
fn an_exact_mana_cost_is_a_cost_of_exactly_that_generic_mana(
    #[case] word: &str,
    #[case] preds: &[&str],
) {
    let (json, found) = sorcery(&format!(
        "A:SP$ ChangeZone | Origin$ Library | Destination$ Battlefield | ChangeType$ Artifact.{word} | ChangeNum$ 1"
    ));
    assert!(found.is_empty(), "{found:?}");
    for pred in preds {
        assert!(json.contains(pred), "{pred} in {json}");
    }
}

#[rstest]
#[case::graveyard(
    "Creature.YouCtrl+doesNotShareNameWith YourGraveyard",
    r#"{"Not":{"SharesName":{"Each":[{"In":"Graveyard"},{"OwnedBy":"You"}]}}}"#
)]
#[case::valid_string(
    "Aura.doesNotShareNameWith Aura.YouCtrl",
    r#"{"Not":{"SharesName":{"Each":[{"Subtype":"Aura"},{"ControlledBy":"You"}]}}}"#
)]
fn a_name_that_is_not_shared_is_the_negation_of_sharing_it(
    #[case] valid: &str,
    #[case] expected: &str,
) {
    let (json, found) = sorcery(&format!(
        "A:SP$ ChangeZone | Origin$ Library | Destination$ Hand | ChangeType$ {valid} | ChangeNum$ 1"
    ));
    assert!(found.is_empty(), "{found:?}");
    assert!(json.contains(expected), "{json}");
}

#[test]
fn another_permanent_of_the_same_name_stays_a_gap() {
    let (_, found) = sorcery(
        "A:SP$ ChangeZone | Origin$ Library | Destination$ Hand | ChangeType$ Creature.doesNotShareNameWith OtherYourBattlefield | ChangeNum$ 1",
    );
    assert_eq!(found, ["valid:doesNotShareNameWith OtherYourBattlefield"]);
}

#[rstest]
#[case::you_control(
    "T:Mode$ DamageDone | ValidSource$ Card.YouCtrl,Emblem.YouCtrl | ValidTarget$ Opponent | CombatDamage$ False | Execute$ G\nSVar:G:DB$ GainLife | LifeAmount$ 1",
    r#""source":[{"ControlledBy":"You"}]"#
)]
#[case::another(
    "T:Mode$ DamageDone | ValidSource$ Card.Other+YouCtrl,Emblem.YouCtrl | ValidTarget$ Opponent | Execute$ G\nSVar:G:DB$ GainLife | LifeAmount$ 1",
    r#""source":["#
)]
#[case::any_source(
    "T:Mode$ DamageDone | ValidSource$ Card,Emblem | ValidTarget$ Opponent | Execute$ G\nSVar:G:DB$ GainLife | LifeAmount$ 1",
    r#""DealsDamage""#
)]
fn an_emblem_is_a_source_the_card_alternative_already_covers(
    #[case] lines: &str,
    #[case] expected: &str,
) {
    let (json, found) = creature(lines);
    assert!(found.is_empty(), "{found:?}");
    assert!(json.contains(expected), "{json}");
    assert!(!json.contains("Command"), "{json}");
}

#[test]
fn an_emblem_alternative_without_a_card_twin_stays_a_gap() {
    let (_, found) = creature(
        "T:Mode$ DamageDone | ValidSource$ Card.YouCtrl,Emblem.OppCtrl | ValidTarget$ Opponent | Execute$ G\nSVar:G:DB$ GainLife | LifeAmount$ 1",
    );
    assert_eq!(found, ["valid-base:Emblem"]);
}

#[rstest]
#[case::targeted("Targeted", r#"{"Is":{"Target":0}}"#)]
#[case::the_source("Self", r#"{"Is":"This"}"#)]
fn a_delayed_trigger_names_the_objects_it_was_made_for(#[case] word: &str, #[case] expected: &str) {
    let (json, found) = sorcery(&format!(
        "A:SP$ Pump | ValidTgts$ Creature.YouCtrl | NumAtt$ +2 | SubAbility$ D\nSVar:D:DB$ DelayedTrigger | Mode$ ChangesZone | RememberObjects$ {word} | ValidCard$ Card.IsTriggerRemembered | Origin$ Battlefield | Destination$ Graveyard | ThisTurn$ True | Execute$ W\nSVar:W:DB$ Draw"
    ));
    assert!(found.is_empty(), "{found:?}");
    assert!(json.contains(expected), "{json}");
}

#[rstest]
#[case::no_objects(
    "A:SP$ Draw | SubAbility$ D\nSVar:D:DB$ DelayedTrigger | Mode$ ChangesZone | ValidCard$ Card.IsTriggerRemembered | Origin$ Battlefield | Destination$ Graveyard | Execute$ W\nSVar:W:DB$ Draw",
    "valid:IsTriggerRemembered"
)]
#[case::an_unknown_word(
    "A:SP$ Draw | SubAbility$ D\nSVar:D:DB$ DelayedTrigger | Mode$ ChangesZone | RememberObjects$ Nonsense | ValidCard$ Card.IsTriggerRemembered | Origin$ Battlefield | Destination$ Graveyard | Execute$ W\nSVar:W:DB$ Draw",
    "key:DelayedTrigger.RememberObjects=Nonsense"
)]
fn a_delayed_trigger_without_known_objects_stays_a_gap(#[case] lines: &str, #[case] gap: &str) {
    let (_, found) = sorcery(lines);
    assert_eq!(found, [gap]);
}

#[rstest]
#[case::damage(
    "T:Mode$ DamageDone | ValidSource$ Card.Self | ValidTarget$ Player | Execute$ D\nSVar:D:DB$ Destroy | ValidTgts$ Artifact.ControlledBy TriggeredTarget",
    r#"{"ControlledBy":"Triggering"}"#
)]
fn a_creature_damaged_player_is_the_one_whose_permanents_are_read(
    #[case] lines: &str,
    #[case] expected: &str,
) {
    let (json, found) = creature(lines);
    assert!(found.is_empty(), "{found:?}");
    assert!(json.contains(expected), "{json}");
}

#[test]
fn the_player_of_a_trigger_that_names_no_target_stays_a_gap() {
    let (_, found) = creature(
        "T:Mode$ Attacks | ValidCard$ Card.Self | Execute$ D\nSVar:D:DB$ Destroy | ValidTgts$ Artifact.ControlledBy TriggeredTarget",
    );
    assert_eq!(found, ["defined:TriggeredTarget-in-Attacks"]);
}

#[rstest]
#[case::the_target(
    "A:SP$ Pump | ValidTgts$ Creature.YouCtrl | RememberObjects$ ThisTargetedCard | SubAbility$ D\nSVar:D:DB$ DealDamage | ValidTgts$ Any | NumDmg$ X | DamageSource$ Remembered | SubAbility$ Z\nSVar:Z:DB$ Cleanup | ClearRemembered$ True\nSVar:X:Remembered$CardPower",
    r#"{"DealDamage":{"amount":{"Stat":{"of":{"Target":0},"stat":"Power"}},"source":{"Target":0},"to":{"Target":1}}}"#
)]
#[case::its_controller(
    "A:SP$ Pump | ValidTgts$ Creature | RememberObjects$ TargetedController | SubAbility$ D\nSVar:D:DB$ LoseLife | Defined$ Remembered | LifeAmount$ 3 | SubAbility$ Z\nSVar:Z:DB$ Cleanup | ClearRemembered$ True",
    r#""who":{"ControllerOf":{"Target":0}}"#
)]
fn a_pump_that_remembers_names_its_object_to_the_lines_after_it(
    #[case] lines: &str,
    #[case] expected: &str,
) {
    let (json, found) = sorcery(lines);
    assert!(found.is_empty(), "{found:?}");
    assert!(json.contains(expected), "{json}");
}

#[test]
fn a_pump_that_remembers_an_unknown_word_stays_a_gap() {
    let (_, found) = sorcery("A:SP$ Pump | ValidTgts$ Creature | RememberObjects$ Nonsense");
    assert_eq!(found, ["key:Pump.RememberObjects=Nonsense"]);
}

#[rstest]
#[case::life("LifeTotal", r#""amount":{"Life":{"Bound":"each"}}"#)]
#[case::life_lost(
    "LifeLostThisTurn",
    r#""amount":{"Tallied":{"what":"LifeLost","who":{"Bound":"each"}}}"#
)]
#[case::their_creatures(
    "Valid Creature.YouCtrl+attacking",
    r#""amount":{"Largest":{"of":[{"Type":"Creature"},{"ControlledBy":"You"},"Attacking"],"who":{"Bound":"each"}}}"#
)]
fn a_number_about_the_player_a_loop_is_on_is_read_for_that_player(
    #[case] what: &str,
    #[case] expected: &str,
) {
    let (json, found) = sorcery(&format!(
        "A:SP$ RepeatEach | RepeatPlayers$ Player | RepeatSubAbility$ L\nSVar:L:DB$ LoseLife | Defined$ Remembered | LifeAmount$ X\nSVar:X:PlayerCountRemembered${what}"
    ));
    assert!(found.is_empty(), "{found:?}");
    assert!(json.contains(expected), "{json}");
}

#[test]
fn a_number_about_a_remembered_player_outside_a_loop_stays_a_gap() {
    let (_, found) = sorcery("A:SP$ Draw | NumCards$ X\nSVar:X:PlayerCountRemembered$LifeTotal");
    assert_eq!(found, ["amount:PlayerCountRemembered-without-a-player"]);
}
