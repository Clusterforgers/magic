//! Goal: the expression words of round 5 piece 4 (a trigger's activator and card owner, a doubled pump, the life an opponent lost, the type a spell chose) turn into the DSL references the engine has, and a word that needs more than one object or more than the engine holds stays a gap.
use rstest::rstest;
use serde_json::{Value, json};

use super::{abilities, gaps, normal};

fn trigger(mode: &str, execute: &str) -> String {
    format!(
        "Name:T\nTypes:Creature Elf\nT:{mode} | TriggerZones$ Battlefield | Execute$ Go\nSVar:Go:{execute}\n"
    )
}

#[rstest]
#[case::spell_activator(
    "Mode$ SpellCast | ValidCard$ Creature",
    "DB$ LoseLife | Defined$ TriggeredActivator | LifeAmount$ 2",
    json!({"SpellCast": {"by": "EachPlayer", "what": [{"Type": "Creature"}]}}),
    json!({"LoseLife": {"who": "Triggering", "amount": 2}})
)]
#[case::dying_card_owner(
    "Mode$ ChangesZone | Origin$ Battlefield | Destination$ Graveyard | ValidCard$ Creature",
    "DB$ LoseLife | Defined$ TriggeredCardOwner | LifeAmount$ 2",
    json!({"ZoneChange": {"from": "Battlefield", "to": "Graveyard", "what": [{"Type": "Creature"}]}}),
    json!({"LoseLife": {"who": {"OwnerOf": "Triggering"}, "amount": 2}})
)]
fn trigger_words(
    #[case] mode: &str,
    #[case] execute: &str,
    #[case] when: Value,
    #[case] effect: Value,
) {
    let found = abilities(&trigger(mode, execute));
    let expected = normal(json!({"Triggered": {"when": when, "effects": [effect]}}));
    assert_eq!(found, vec![expected]);
}

#[rstest]
#[case::no_owner_for_a_life_change(
    "Mode$ LifeGained",
    "DB$ LoseLife | Defined$ TriggeredCardOwner | LifeAmount$ 2",
    "defined:TriggeredCardOwner-in-LifeGained"
)]
#[case::no_activator_for_a_death(
    "Mode$ ChangesZone | Origin$ Battlefield | Destination$ Graveyard | ValidCard$ Creature",
    "DB$ LoseLife | Defined$ TriggeredActivator | LifeAmount$ 2",
    "defined:TriggeredActivator-in-ChangesZone"
)]
fn trigger_words_the_event_does_not_hold(
    #[case] mode: &str,
    #[case] execute: &str,
    #[case] gap: &str,
) {
    assert_eq!(gaps(&trigger(mode, execute)), vec![gap.to_owned()]);
}

#[rstest]
#[case::power_and_toughness(
    "Name:S\nTypes:Sorcery\nA:SP$ Pump | ValidTgts$ Creature | NumAtt$ Double | NumDef$ Double\n",
    json!({"Stat": {"of": {"Target": 0}, "stat": "Power"}}),
    json!({"Stat": {"of": {"Target": 0}, "stat": "Toughness"}})
)]
#[case::power_only(
    "Name:S\nTypes:Sorcery\nA:SP$ Pump | ValidTgts$ Creature | NumAtt$ Double\n",
    json!({"Stat": {"of": {"Target": 0}, "stat": "Power"}}),
    json!(0)
)]
fn a_doubled_pump_adds_the_value_the_target_has(
    #[case] script: &str,
    #[case] power: Value,
    #[case] toughness: Value,
) {
    let found = abilities(script);
    let mods = &found[0]["Spell"]["effects"][0]["Continuous"]["mods"][0]["ModifyPT"];
    assert_eq!((&mods["power"], &mods["toughness"]), (&power, &toughness));
}

#[test]
fn a_doubled_pump_of_several_objects_stays_a_gap() {
    let script =
        "Name:S\nTypes:Sorcery\nA:SP$ PumpAll | ValidCards$ Creature.YouCtrl | NumAtt$ Double\n";
    assert_eq!(gaps(script), vec!["amount:Double-of-several".to_owned()]);
}

#[rstest]
#[case::opponents("PlayerCountRegisteredOpponents", "Opponents")]
#[case::opponents_plain("PlayerCountOpponents", "Opponents")]
#[case::everyone("PlayerCountPlayers", "EachPlayer")]
fn the_most_life_lost_this_turn_reads_the_turn_tally(#[case] head: &str, #[case] who: &str) {
    let script = format!(
        "Name:S\nTypes:Sorcery\nA:SP$ Draw | NumCards$ X\nSVar:X:{head}$HighestLifeLostThisTurn\n"
    );
    let found = abilities(&script);
    assert_eq!(
        found[0]["Spell"]["effects"][0]["Draw"]["count"],
        json!({"Tallied": {"who": who, "what": "LifeLost"}})
    );
}

#[test]
fn opponents_with_a_property_stay_a_gap() {
    let script = "Name:S\nTypes:Sorcery\nA:SP$ Draw | NumCards$ X\nSVar:X:PlayerCountRegisteredOpponents$HasPropertyLostLifeThisTurn\n";
    assert_eq!(gaps(script).len(), 1);
}

#[rstest]
#[case::destroy("DB$ DestroyAll | ValidCards$ Creature.IsNotChosenType", "Destroy")]
#[case::pump(
    "DB$ PumpAll | ValidCards$ Creature.IsNotChosenType | NumAtt$ -3 | NumDef$ -3",
    "Continuous"
)]
#[case::bounce(
    "DB$ ChangeZoneAll | ChangeType$ Creature.IsNotChosenType | Origin$ Battlefield | Destination$ Hand",
    "ChangeZone"
)]
fn what_is_not_of_the_chosen_type_reads_the_choice(#[case] second: &str, #[case] verb: &str) {
    let script = format!(
        "Name:S\nTypes:Sorcery\nA:SP$ ChooseType | Defined$ You | Type$ Creature | SubAbility$ Go\nSVar:Go:{second}\n"
    );
    let found = abilities(&script);
    let text = found[0].to_string();
    assert!(text.contains(r#"{"Not":{"ChosenAs":"chosen"}}"#), "{text}");
    assert!(text.contains(verb) || verb == "ChangeZone", "{text}");
    assert_eq!(gaps(&script), Vec::<String>::new());
}
