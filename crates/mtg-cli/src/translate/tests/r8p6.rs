//! Goal: round 8 piece 6 reads the long tail of expression forms: player words of damage triggers, the target as a valid-string base, delayed triggers that choose a target, names chosen earlier in the ability, remembered cards that may be played, the spell of a cast trigger and every counter of a kind.
#![allow(clippy::unwrap_used)]

use rstest::rstest;

use super::{abilities, gaps};

/// The translation of a creature with the given lines as compact JSON, and its gaps.
fn creature(lines: &str) -> (String, Vec<String>) {
    let text = format!("Name:Subject\nTypes:Creature Elf\nPT:1/1\n{lines}\n");
    (
        serde_json::to_string(&abilities(&text)).unwrap(),
        gaps(&text),
    )
}

/// The translation of a sorcery with the given lines.
fn sorcery(lines: &str) -> (String, Vec<String>) {
    let text = format!("Name:Subject\nTypes:Sorcery\n{lines}\n");
    (
        serde_json::to_string(&abilities(&text)).unwrap(),
        gaps(&text),
    )
}

#[rstest]
#[case::enchanted_player("Player.EnchantedBy", r#""to":"AttachedPlayer""#)]
#[case::another_player("Player.Other", r#""to":"Opponents""#)]
#[case::defending_player("Player.TriggeredDefendingPlayer", r#""to":"Defending""#)]
fn a_damage_trigger_reads_the_player_it_names(#[case] valid: &str, #[case] expected: &str) {
    let (json, found) = creature(&format!(
        "T:Mode$ DamageDone | ValidSource$ Card.Self | ValidTarget$ {valid} | Execute$ D\nSVar:D:DB$ Draw | Defined$ You"
    ));
    assert!(found.is_empty(), "{found:?}");
    assert!(json.contains(expected), "{json}");
}

#[rstest]
#[case::more_lands("Player.withMoreLandsThanYou", "player:Player.withMoreLandsThanYou")]
#[case::dealt_damage(
    "Player.wasDealtDamageThisTurnBySource",
    "player:Player.wasDealtDamageThisTurnBySource"
)]
fn a_player_property_the_engine_lacks_stays_a_gap_by_name(#[case] valid: &str, #[case] gap: &str) {
    let (_, found) = creature(&format!(
        "T:Mode$ DamageDone | ValidSource$ Card.Self | ValidTarget$ {valid} | Execute$ D\nSVar:D:DB$ Draw | Defined$ You"
    ));
    assert_eq!(found, [gap]);
}

#[test]
fn a_player_that_becomes_a_target_stays_a_gap() {
    let (_, found) = creature(
        "T:Mode$ BecomesTarget | ValidTarget$ You,Permanent.YouCtrl | Execute$ D\nSVar:D:DB$ Draw | Defined$ You",
    );
    assert_eq!(found, ["trigger:BecomesTarget-a-player"]);
}

#[test]
fn a_random_discard_cost_stays_a_gap_by_name() {
    let (_, found) = creature("A:AB$ Draw | Cost$ Discard<1/Random> | NumCards$ 1");
    assert_eq!(found, ["cost:Discard-random"]);
}

#[rstest]
#[case::same_name_other(
    "A:SP$ Pump | ValidTgts$ Creature | SubAbility$ P\nSVar:P:DB$ PumpAll | ValidCards$ Targeted.Creature+Other+sameName | NumAtt$ +2 | NumDef$ +2",
    r#"{"SharesName":{"Target":0}}"#
)]
#[case::the_target_or_its_namesakes(
    "A:SP$ Pump | ValidTgts$ Permanent.nonLand | SubAbility$ P\nSVar:P:DB$ ChangeZoneAll | Origin$ Battlefield | Destination$ Hand | ChangeType$ TargetedCard.Self,Permanent.NotDefinedTargeted+sharesNameWith Targeted",
    r#"{"Is":{"Target":0}}"#
)]
#[case::attachments_of_the_target(
    "A:SP$ Pump | ValidTgts$ Creature | SubAbility$ P\nSVar:P:DB$ DestroyAll | ValidCards$ Targeted.Equipment+Attached",
    r#"{"AttachedTo":{"Target":0}}"#
)]
#[case::an_aura_on_the_targeted_line(
    "A:SP$ ChangeZoneAll | ValidTgts$ Creature | ChangeType$ TargetedCard.Self,Aura.AttachedTo Targeted | Origin$ Battlefield | Destination$ Hand",
    r#"{"AttachedTo":{"Target":0}}"#
)]
fn the_target_is_a_base_of_a_valid_string(#[case] lines: &str, #[case] expected: &str) {
    let (json, found) = sorcery(lines);
    assert!(found.is_empty(), "{found:?}");
    assert!(json.contains(expected), "{json}");
}

#[test]
fn the_target_with_no_target_declared_stays_a_gap() {
    let (_, found) = sorcery("A:SP$ DestroyAll | ValidCards$ Targeted.sameName");
    assert_eq!(found, ["defined:Targeted-without-target"]);
}

#[test]
fn a_delayed_trigger_that_chooses_its_target_waits_for_that_target() {
    let (json, found) = sorcery(
        "A:SP$ DelayedTrigger | ValidTgts$ Creature | Mode$ ChangesZone | RememberObjects$ Targeted | ValidCard$ Card.IsTriggerRemembered | Origin$ Battlefield | Destination$ Graveyard | ThisTurn$ True | Execute$ R\nSVar:R:DB$ ChangeZone | Defined$ DelayTriggerRemembered | Origin$ Graveyard | Destination$ Battlefield",
    );
    assert!(found.is_empty(), "{found:?}");
    assert!(json.contains(r#""what":[{"Is":{"Target":0}}]"#), "{json}");
    assert!(json.contains(r#""what":"Triggering""#), "{json}");
    assert!(
        json.contains(r#""targets":[{"what":{"Object":[{"Type":"Creature"}]}}]"#),
        "{json}"
    );
}

#[rstest]
#[case::equipment_on_this("SVar:X:Count$Valid Equipment.Attached", r#"{"AttachedTo":"This"}"#)]
#[case::aura_on_this("SVar:X:Count$Valid Aura.Attached/Times.2", r#"{"AttachedTo":"This"}"#)]
fn an_attachment_of_the_source_is_counted(#[case] svar: &str, #[case] expected: &str) {
    let (json, found) = creature(&format!(
        "T:Mode$ Attacks | ValidCard$ Card.Self | Execute$ D\nSVar:D:DB$ Draw | NumCards$ X\n{svar}"
    ));
    assert!(found.is_empty(), "{found:?}");
    assert!(json.contains(expected), "{json}");
}

#[test]
fn a_card_named_by_an_earlier_step_is_the_named_card() {
    let (json, found) = sorcery(
        "A:SP$ NameCard | Defined$ You | ValidCards$ Card.nonLand | SubAbility$ D\nSVar:D:DB$ Discard | Defined$ You | Mode$ TgtChoose | DiscardValid$ Card.NamedCard | NumCards$ 1",
    );
    assert!(found.is_empty(), "{found:?}");
    assert!(json.contains(r#"{"ChosenAs":"name"}"#), "{json}");
}

#[test]
fn a_named_card_without_any_naming_stays_a_gap() {
    let (_, found) = sorcery(
        "A:SP$ Discard | Defined$ You | Mode$ TgtChoose | DiscardValid$ Card.NamedCard | NumCards$ 1",
    );
    assert_eq!(found, ["valid:NamedCard-without-a-choice"]);
}

#[rstest]
#[case::nonland("Card.IsRemembered+nonLand", r#""what":[{"Not":{"Type":"Land"}}]"#)]
#[case::reversed("Card.nonLand+IsRemembered", r#""what":[{"Not":{"Type":"Land"}}]"#)]
#[case::plain("Card.IsRemembered", r#""only""#)]
fn a_remembered_card_may_be_played_with_more_words(#[case] affected: &str, #[case] expected: &str) {
    let (json, found) = sorcery(&format!(
        "A:SP$ ChangeZone | Origin$ Battlefield | Destination$ Exile | ValidTgts$ Permanent | RememberChanged$ True | SubAbility$ E\nSVar:E:DB$ Effect | RememberObjects$ Remembered | StaticAbilities$ M | Duration$ Permanent | ForgetOnMoved$ Exile | SubAbility$ C\nSVar:M:Mode$ Continuous | MayPlay$ True | Affected$ {affected} | AffectedZone$ Exile\nSVar:C:DB$ Cleanup | ClearRemembered$ True"
    ));
    assert!(found.is_empty(), "{found:?}");
    assert!(json.contains(expected), "{json}");
}

#[rstest]
#[case::cast("SpellCast", true)]
#[case::cast_or_copy("SpellCastOrCopy", true)]
#[case::another_event("Attacks", false)]
fn the_spell_of_a_cast_trigger_gives_its_mana_value(#[case] mode: &str, #[case] reads: bool) {
    let (json, found) = creature(&format!(
        "T:Mode$ {mode} | ValidCard$ Card | Execute$ G\nSVar:G:DB$ GainLife | LifeAmount$ X\nSVar:X:TriggeredSpellAbility$CardManaCostLKI"
    ));
    if reads {
        assert!(found.is_empty(), "{found:?}");
        assert!(
            json.contains(r#""Stat":{"of":"Triggering","stat":"ManaValue"}"#),
            "{json}"
        );
    } else {
        assert_eq!(found, ["amount:TriggeredSpellAbility"]);
    }
}

#[test]
fn every_counter_of_a_kind_is_as_many_as_the_object_holds() {
    let (json, found) =
        creature("A:AB$ RemoveCounter | Cost$ T | CounterType$ TIME | CounterNum$ All");
    assert!(found.is_empty(), "{found:?}");
    assert!(
        json.contains(r#""count":{"Counters":{"counter":{"Other":"time"},"on":"This"}}"#),
        "{json}"
    );
}

#[test]
fn damage_to_players_alone_hits_no_object() {
    let (json, found) = sorcery("A:SP$ DamageAll | ValidPlayers$ Player.Opponent | NumDmg$ 3");
    assert!(found.is_empty(), "{found:?}");
    assert!(
        json.contains(r#""to":"Opponents""#) && !json.contains("Each"),
        "{json}"
    );
}
