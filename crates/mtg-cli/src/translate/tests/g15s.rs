//! Goal: round 15 pack s reads the small forms of the tail: mana of the type a tapped permanent produced, extra phases
//! gated on the phase, a search of the exiled card's controller, a target revealed through `Defined$`, a counted cost
//! reduction and a Vehicle token with power and toughness.
#![allow(clippy::unwrap_used)]

use rstest::rstest;

use super::{abilities, gaps};

/// The translation of a card with the given header and lines as compact JSON, and its gaps.
fn card(types: &str, lines: &str) -> (String, Vec<String>) {
    let text = format!("Name:Subject\nTypes:{types}\n{lines}\n");
    (
        serde_json::to_string(&abilities(&text)).unwrap(),
        gaps(&text),
    )
}

#[rstest]
#[case::the_player_who_tapped("Land", "TriggeredActivator", r#""who":"Triggering""#)]
#[case::you("Land | Activator$ You", "You", r#""AddMana":{"mana":"OfTapped"}"#)]
fn mana_of_a_type_the_tapped_permanent_produced(
    #[case] valid: &str,
    #[case] defined: &str,
    #[case] expect: &str,
) {
    let (json, found) = card(
        "Enchantment",
        &format!(
            "T:Mode$ TapsForMana | ValidCard$ {valid} | Execute$ TrigMana | TriggerZones$ Battlefield | Static$ True\nSVar:TrigMana:DB$ ManaReflected | ColorOrType$ Type | ReflectProperty$ Produced | Defined$ {defined}"
        ),
    );
    assert!(found.is_empty(), "{found:?}");
    assert!(json.contains(expect) && json.contains("OfTapped"), "{json}");
}

#[rstest]
#[case::color_is_not_a_type("ColorOrType$ Color | ReflectProperty$ Produced | Defined$ You")]
#[case::other_player("ColorOrType$ Type | ReflectProperty$ Produced | Defined$ Opponent")]
#[case::a_counted_amount(
    "ColorOrType$ Type | ReflectProperty$ Produced | Defined$ You | Amount$ 2"
)]
fn mana_of_the_produced_type_with_another_shape_stays_a_gap(#[case] keys: &str) {
    let (_, found) = card(
        "Enchantment",
        &format!(
            "T:Mode$ TapsForMana | ValidCard$ Land | Execute$ TrigMana | TriggerZones$ Battlefield | Static$ True\nSVar:TrigMana:DB$ ManaReflected | {keys}"
        ),
    );
    assert!(!found.is_empty(), "a gap was expected");
}

#[rstest]
#[case::two_mains(
    "ConditionPhases$ Main1,Main2 | ExtraPhase$ Combat | FollowedBy$ Main2",
    r#""Step":"Main1""#,
    r#""phases":["Combat","Main"]"#
)]
#[case::your_main_phase(
    "ConditionPhases$ Main1,Main2 | ConditionPlayerTurn$ True | ExtraPhase$ Combat",
    r#""YourTurn""#,
    r#""phases":["Combat"]"#
)]
#[case::in_combat(
    "ConditionPhases$ BeginCombat->EndCombat | ConditionPlayerTurn$ True | AfterPhase$ EndCombat | ExtraPhase$ Combat",
    r#""InCombat""#,
    r#""phases":["Combat"]"#
)]
fn an_extra_phase_waits_for_the_phase_the_line_names(
    #[case] keys: &str,
    #[case] condition: &str,
    #[case] phases: &str,
) {
    let (json, found) = card("Sorcery", &format!("A:SP$ AddPhase | {keys}"));
    assert!(found.is_empty(), "{found:?}");
    assert!(json.contains(phases), "{json}");
    assert!(json.contains(condition), "{json}");
}

#[test]
fn a_phase_word_outside_the_turn_stays_a_gap() {
    let (_, found) = card(
        "Sorcery",
        "A:SP$ AddPhase | ConditionPhases$ Nowhere | ExtraPhase$ Combat",
    );
    assert!(!found.is_empty(), "a gap was expected");
}

#[test]
fn a_search_of_the_exiled_creatures_controller_reads_its_controller() {
    let (json, found) = card(
        "Sorcery",
        "A:SP$ ChangeZone | Origin$ Battlefield | Destination$ Exile | ValidTgts$ Creature.nonBlack | RememberLKI$ True | ForgetOtherTargets$ True | SubAbility$ ExileYard\nSVar:ExileYard:DB$ ChangeZoneAll | Origin$ Graveyard | Destination$ Exile | Defined$ RememberedController | ChangeType$ Remembered.sameName | SubAbility$ ExileHand\nSVar:ExileHand:DB$ ChangeZone | Origin$ Hand | Destination$ Exile | DefinedPlayer$ RememberedController | ChangeType$ Remembered.sameName | ChangeNum$ NumInHand | Chooser$ You\nSVar:NumInHand:PlayerCountRememberedController$CardsInHand",
    );
    assert!(found.is_empty(), "{found:?}");
    // Both the zone's owner and the count of the hand are the controller the target had.
    assert_eq!(
        json.matches(r#""OwnedBy":{"ControllerOf":{"Target":0}}"#)
            .count(),
        3,
        "{json}"
    );
}

#[test]
fn a_search_of_a_trigger_target_reads_that_player() {
    let (json, found) = card(
        "Creature Spirit",
        "PT:2/2\nT:Mode$ DamageDone | ValidSource$ Card.Self | ValidTarget$ Player | CombatDamage$ True | Execute$ TrigReveal\nSVar:TrigReveal:DB$ RevealHand | Defined$ TriggeredTarget | SubAbility$ ExileYard\nSVar:ExileYard:DB$ ChangeZoneAll | Origin$ Graveyard | Destination$ Exile | Defined$ TriggeredTarget | ChangeType$ Card.Land",
    );
    assert!(found.is_empty(), "{found:?}");
    assert!(json.contains(r#""OwnedBy":"Triggering""#), "{json}");
}

#[rstest]
#[case::targeted("Defined$ Targeted", true)]
#[case::no_defined("", true)]
#[case::someone_else("Defined$ Player", false)]
fn a_revealed_hand_with_a_target_repeats_it_in_defined(#[case] defined: &str, #[case] read: bool) {
    let (json, found) = card(
        "Sorcery",
        &format!("A:SP$ RevealHand | ValidTgts$ Opponent | {defined}"),
    );
    assert_eq!(found.is_empty(), read, "{found:?}");
    if read {
        assert!(json.contains(r#""Reveal""#), "{json}");
    }
}

#[test]
fn a_cost_reduction_counted_in_times_is_read_as_relative() {
    let (json, found) = card(
        "Creature Zombie",
        "PT:2/2\nS:Mode$ ReduceCost | ValidCard$ Card.Self | Type$ Spell | Amount$ Y | Relative$ True | EffectZone$ All\nSVar:Y:PlayerCountPropertyYou$CardsDiscardedThisTurn/Twice",
    );
    assert!(found.is_empty(), "{found:?}");
    assert!(json.contains(r#""change":"Reduce""#), "{json}");
    assert!(json.contains(r#""by":2"#), "{json}");
}

#[test]
fn a_vehicle_token_keeps_its_power_and_toughness_without_being_a_creature() {
    let (json, found) = card(
        "Sorcery",
        "A:SP$ Token | TokenScript$ c_3_2_a_vehicle_crew_1 | TokenOwner$ You",
    );
    assert!(found.is_empty(), "{found:?}");
    assert!(json.contains(r#""power":3,"#), "{json}");
    assert!(json.contains(r#""types":["Artifact"]"#), "{json}");
    assert!(json.contains(r#""Crew":1"#), "{json}");
}
