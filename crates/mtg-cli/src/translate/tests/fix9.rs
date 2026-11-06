//! Goal: the faults fix pass 9 found stay closed: "dealt damage this way" asks that the damage was dealt, "a permanent card" outside the battlefield tests the card's types, a printed "creature card of the chosen type" keeps the creature, and a trigger check the card does not print is a gap.
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
#[case::the_spell("", r#"{"DamagedBy":"This"}"#)]
#[case::itself(" | DamageSource$ Self", r#"{"DamagedBy":"This"}"#)]
fn a_condition_on_the_damaged_target_asks_that_damage_was_dealt(
    #[case] source: &str,
    #[case] wanted: &str,
) {
    let (json, found) = card(
        "Instant",
        &format!(
            "A:SP$ DealDamage | ValidTgts$ Creature | NumDmg$ 4 | RememberDamaged$ True{source} | SubAbility$ DBSac\nSVar:DBSac:DB$ Sacrifice | SacValid$ Land | Defined$ You | ConditionDefined$ Remembered | ConditionPresent$ Card.nonRed | ConditionCompare$ GE1 | SubAbility$ DBCleanup\nSVar:DBCleanup:DB$ Cleanup | ClearRemembered$ True"
        ),
    );
    assert!(found.is_empty(), "{found:?}");
    assert!(json.contains(wanted), "{json}");
}

#[test]
fn a_condition_on_a_tapped_target_is_not_about_damage() {
    let (json, found) = card(
        "Instant",
        "A:SP$ Tap | ValidTgts$ Creature | RememberTapped$ True | SubAbility$ DBDraw\nSVar:DBDraw:DB$ Draw | ConditionDefined$ Remembered | ConditionPresent$ Card.Blue | SubAbility$ DBCleanup\nSVar:DBCleanup:DB$ Cleanup | ClearRemembered$ True",
    );
    assert!(found.is_empty(), "{found:?}");
    assert!(!json.contains("DamagedBy"), "{json}");
}

const PERMANENT: &str = r#"{"AnyOf":[{"Type":"Artifact"},{"Type":"Battle"},{"Type":"Creature"},{"Type":"Enchantment"},{"Type":"Land"},{"Type":"Planeswalker"}]}"#;

#[rstest]
// Lion Sash: "if it was a permanent card".
#[case::permanent("Permanent", true)]
// A creature is a permanent type already.
#[case::creature("Permanent.Creature", false)]
#[case::any_card("Card", false)]
fn an_exiled_permanent_card_is_one_of_the_permanent_types(
    #[case] valid: &str,
    #[case] typed: bool,
) {
    let (json, found) = card(
        "Artifact Creature Cat",
        &format!(
            "PT:1/1\nA:AB$ ChangeZone | Cost$ W | Origin$ Graveyard | Destination$ Exile | ValidTgts$ Card | RememberChanged$ True | SubAbility$ DBPutCounter\nSVar:DBPutCounter:DB$ PutCounter | CounterType$ P1P1 | CounterNum$ 1 | ConditionDefined$ Remembered | ConditionPresent$ {valid} | SubAbility$ DBCleanup\nSVar:DBCleanup:DB$ Cleanup | ClearRemembered$ True"
        ),
    );
    assert!(found.is_empty(), "{found:?}");
    assert_eq!(json.contains(PERMANENT), typed, "{json}");
}

#[rstest]
// Lifespinner: a legendary Spirit permanent card.
#[case::permanent("Permanent.Spirit+Legendary", true)]
#[case::creature("Creature.Spirit+Legendary", false)]
#[case::permanent_creature("Permanent.Creature", false)]
fn a_searched_permanent_card_is_one_of_the_permanent_types(
    #[case] change: &str,
    #[case] typed: bool,
) {
    let (json, found) = card(
        "Sorcery",
        &format!(
            "A:SP$ ChangeZone | Origin$ Library | Destination$ Battlefield | ChangeType$ {change} | ChangeNum$ 1"
        ),
    );
    assert!(found.is_empty(), "{found:?}");
    assert_eq!(json.contains(PERMANENT), typed, "{json}");
}

#[rstest]
// From the Rubble prints "creature card"; Gathering Stone-like text prints only "card".
#[case::creature_printed(
    "return target creature card of the chosen type from your graveyard",
    true
)]
#[case::card_printed("return target card of the chosen type from your graveyard", false)]
fn a_printed_creature_card_of_the_chosen_type_keeps_the_creature(
    #[case] words: &str,
    #[case] creature: bool,
) {
    let (json, found) = card(
        "Enchantment",
        &format!(
            "K:ETBReplacement:Other:ChooseCT\nSVar:ChooseCT:DB$ ChooseType | Type$ Creature | SpellDescription$ As CARDNAME enters, choose a creature type.\nT:Mode$ Phase | Phase$ End of Turn | ValidPlayer$ You | TriggerZones$ Battlefield | Execute$ TrigReturn | TriggerDescription$ At the beginning of your end step, {words}.\nSVar:TrigReturn:DB$ ChangeZone | Origin$ Graveyard | Destination$ Hand | ValidTgts$ Card.ChosenType+YouOwn\nOracle:At the beginning of your end step, {words}."
        ),
    );
    assert!(found.is_empty(), "{found:?}");
    assert_eq!(json.contains(r#"{"Type":"Creature"}"#), creature, "{json}");
}

#[rstest]
// Rodolf Duskbringer: X is defined in the text and no "if" is printed.
#[case::unprinted(
    "At the beginning of your end step, you may pay {1}. When you do, return target creature with mana value X or less from your graveyard to the battlefield, where X is the amount of life you gained this turn.",
    "",
    true
)]
#[case::printed_if(
    "At the beginning of your end step, if you gained life this turn, return target creature with mana value X or less from your graveyard to the battlefield, where X is the amount of life you gained this turn.",
    "",
    false
)]
#[case::compared(
    "At the beginning of your end step, return target creature with mana value X or less from your graveyard to the battlefield, where X is the amount of life you gained this turn.",
    " | SVarCompare$ GE1",
    false
)]
fn a_trigger_check_the_card_does_not_print_is_a_gap(
    #[case] text: &str,
    #[case] compare: &str,
    #[case] gap: bool,
) {
    let (_, found) = card(
        "Creature Vampire",
        &format!(
            "PT:4/4\nT:Mode$ Phase | Phase$ End of Turn | ValidPlayer$ You | TriggerZones$ Battlefield | CheckSVar$ X{compare} | Execute$ TrigReturn | TriggerDescription$ {text}\nSVar:TrigReturn:DB$ ChangeZone | ValidTgts$ Creature.cmcLEX+YouOwn | Origin$ Graveyard | Destination$ Battlefield\nSVar:X:Count$LifeYouGainedThisTurn"
        ),
    );
    assert_eq!(
        found.iter().any(|g| g.contains("CheckSVar-not-printed")),
        gap,
        "{found:?}"
    );
}
