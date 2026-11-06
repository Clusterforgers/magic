//! Goal: round 4 piece g reads trigger patterns and replacement effects: events of an object that took damage, shields and redirects, in the order of CR 614 and 616.
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
#[case::combat_to_a_creature("Card.Self", "| CombatDamage$ True", r#""combat":true"#)]
#[case::any_damage_to_a_creature("Card.Self", "", r#""Each":[{"Type":"Creature"}]"#)]
#[case::by_the_enchanted_creature("Card.AttachedBy", "", r#""Is":"Attached""#)]
fn an_object_effect_on_damage_to_a_creature_acts_on_the_damaged_creature(
    #[case] source: &str,
    #[case] extra: &str,
    #[case] expected: &str,
) {
    let (json, found) = card(
        "Creature Ox",
        &format!(
            "PT:2/2\nT:Mode$ DamageDone | ValidSource$ {source} | ValidTarget$ Creature {extra} | TriggerZones$ Battlefield | Execute$ T\nSVar:T:DB$ Tap | Defined$ TriggeredTargetLKICopy"
        ),
    );
    assert!(found.is_empty(), "{found:?}");
    assert!(json.contains(expected), "{json}");
    assert!(json.contains(r#""Tap":{"what":"Triggering"}"#), "{json}");
}

#[test]
fn an_object_effect_on_damage_to_a_player_stays_a_gap() {
    let (_, found) = card(
        "Creature Ox",
        "PT:2/2\nT:Mode$ DamageDone | ValidSource$ Card.Self | ValidTarget$ Player | TriggerZones$ Battlefield | Execute$ T\nSVar:T:DB$ Pump | NumAtt$ +1 | Defined$ TriggeredTargetLKICopy",
    );
    assert!(!found.is_empty());
}

#[rstest]
#[case::life("DB$ GainLife | LifeAmount$ 2", r#"{"GainLife":{"amount":2}}"#)]
#[case::power_counters(
    "DB$ PutCounter | CounterType$ P1P1 | CounterNum$ X | Defined$ Self\nSVar:X:ReplacedCardLKI$CardPower",
    r#""stat":"Power""#
)]
fn exiling_instead_runs_what_follows_in_its_place(#[case] rest: &str, #[case] expected: &str) {
    let (json, found) = card(
        "Enchantment",
        &format!(
            "R:Event$ Moved | ActiveZones$ Battlefield | Origin$ Battlefield | Destination$ Graveyard | ValidLKI$ Creature.OppCtrl | ReplaceWith$ Ex\nSVar:Ex:DB$ ChangeZone | Origin$ Battlefield | Destination$ Exile | Defined$ ReplacedCard | SubAbility$ Rest\nSVar:Rest:{rest}"
        ),
    );
    assert!(found.is_empty(), "{found:?}");
    assert!(
        json.contains(r#""Instead":[{"Exile":{"what":"Triggering"}},"#),
        "{json}"
    );
    assert!(json.contains(expected), "{json}");
}

#[rstest]
#[case::you("You", r#""by":"You""#)]
#[case::an_opponent("Opponent", r#""by":"Opponents""#)]
fn counters_a_player_puts_on_a_creature_are_a_caused_batch(
    #[case] who: &str,
    #[case] expected: &str,
) {
    let (json, found) = card(
        "Creature Elf",
        &format!(
            "PT:1/1\nT:Mode$ CounterPlayerAddedAll | ValidObject$ Creature.YouDontCtrl+inRealZoneBattlefield | ValidSource$ {who} | TriggerZones$ Battlefield | Execute$ D\nSVar:D:DB$ Draw | Defined$ You | NumCards$ 1"
        ),
    );
    assert!(found.is_empty(), "{found:?}");
    assert!(json.contains(r#""Caused":{"#), "{json}");
    assert!(json.contains(r#""CountersPut""#), "{json}");
    assert!(json.contains(expected), "{json}");
}

#[rstest]
#[case::yours("SpellAbility.YouCtrl", true)]
#[case::an_opponents("SpellAbility.OppCtrl", false)]
fn valiant_counts_the_first_targeting_by_a_source_you_control(
    #[case] source: &str,
    #[case] read: bool,
) {
    let (json, found) = card(
        "Creature Mouse",
        &format!(
            "PT:1/1\nT:Mode$ BecomesTarget | ValidTarget$ Card.Self | ValidSource$ {source} | TriggerZones$ Battlefield | Valiant$ True | Execute$ C\nSVar:C:DB$ PutCounter | Defined$ Self | CounterType$ P1P1 | CounterNum$ 1"
        ),
    );
    assert_eq!(found.is_empty(), read, "{found:?}");
    assert_eq!(
        json.contains(r#""Nth":{"event":{"BecomesTarget""#),
        read,
        "{json}"
    );
}
