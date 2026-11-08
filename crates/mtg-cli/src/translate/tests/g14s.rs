//! Goal: round 14 pack s reads the small forms of the tail: an optional fight, bare vanishing, protection from a chosen color
//! or a card type, spells that cannot be countered this turn, damage shields that prevent and then act, a chosen host for an
//! attachment, radiance beyond pumps, a cast condition on a spell line and the shapes of replacements that stay gaps.
#![allow(clippy::unwrap_used)]

use rstest::rstest;

use super::{abilities, gaps};

/// The compact JSON of a card with the given header and lines, and its gaps.
fn card(types: &str, lines: &str) -> (String, Vec<String>) {
    let text = format!("Name:Subject\nTypes:{types}\n{lines}\n");
    (
        serde_json::to_string(&abilities(&text)).unwrap(),
        gaps(&text),
    )
}

/// A card reads without a gap and its JSON holds every fragment.
fn reads(types: &str, lines: &str, holds: &[&str]) {
    let (json, found) = card(types, lines);
    assert!(found.is_empty(), "{found:?}");
    for part in holds {
        assert!(json.contains(part), "{part} missing in {json}");
    }
}

#[rstest]
#[case::optional_fight(
    "Creature Hydra",
    "T:Mode$ ChangesZone | Origin$ Any | Destination$ Battlefield | ValidCard$ Card.Self | Execute$ F\nSVar:F:DB$ Fight | Defined$ Self | ValidTgts$ Creature.Other | Optional$ True",
    &[r#""May":{"effects":[{"Fight":{"what":"This","with":{"Target":0}}}]}"#]
)]
#[case::fight_then_exile_instead(
    "Instant",
    "A:SP$ Pump | ValidTgts$ Creature.YouCtrl | SubAbility$ F\nSVar:F:DB$ Fight | Defined$ ParentTarget | ValidTgts$ Creature.OppCtrl | ReplaceDyingDefined$ ThisTargetedCard",
    &[r#""Fight":{"what":{"Target":0},"with":{"Target":1}}"#, r#""by":{"MoveTo":"Exile"}"#, r#""Is":{"Target":1}"#]
)]
#[case::bare_vanishing("Creature Elemental", "K:Vanishing", &[r#""Keyword":{"Vanishing":0}"#])]
#[case::gains_vanishing(
    "Creature Djinn",
    "A:AB$ Pump | Cost$ T | Defined$ Self | KW$ Vanishing | Duration$ Permanent",
    &[r#""AddKeyword":{"Vanishing":0}"#]
)]
#[case::protection_from_the_chosen_color(
    "Instant",
    "A:SP$ ChooseColor | Defined$ You | SubAbility$ P\nSVar:P:DB$ ProtectionAll | ValidCards$ Creature.YouCtrl | Gains$ ChosenColor",
    &[r#""Protection":[{"ChosenAs":"#]
)]
#[case::protection_from_artifacts(
    "Instant",
    "A:SP$ Protection | ValidTgts$ Creature | Gains$ Artifact",
    &[r#""Protection":[{"Type":"Artifact"}]"#]
)]
#[case::radiance_damage(
    "Instant",
    "A:SP$ DealDamage | ValidTgts$ Creature | Radiance$ True | NumDmg$ 2",
    &[r#""SharesColor":{"Target":0}"#, r#""amount":2"#]
)]
#[case::radiance_destroy(
    "Instant",
    "A:SP$ Destroy | ValidTgts$ Enchantment | Radiance$ True",
    &[r#""Destroy":{"what":{"Each":[{"AnyOf":[{"Is":{"Target":0}},{"SharesColor":{"Target":0}}]},{"Type":"Enchantment"}]}}"#]
)]
#[case::radiance_protection(
    "Instant",
    "A:SP$ Protection | ValidTgts$ Creature | Gains$ Choice | Choices$ AnyColor | Radiance$ True",
    &["AddProtectionFromChosenColor", r#""SharesColor""#]
)]
#[case::cast_only_with_vampires(
    "Instant",
    "A:SP$ Destroy | ValidTgts$ Creature | IsPresent$ Zombie.YouCtrl | PresentCompare$ GE2",
    &[r#""CastOnly""#, r#""Subtype":"Zombie""#]
)]
#[case::search_back_to_the_top(
    "Artifact",
    "A:AB$ ChangeZone | Cost$ 1 Sac<1/CARDNAME> | Origin$ Library | Destination$ Library | ChangeType$ Enchantment | ChangeNum$ 1",
    &[r#""Shuffle""#, r#""Library":{"place":"Top"}"#]
)]
#[case::attach_all_equipment_on_the_battlefield(
    "Creature Human",
    "T:Mode$ ChangesZone | Origin$ Any | Destination$ Battlefield | ValidCard$ Card.Self | Execute$ A\nSVar:A:DB$ Attach | Object$ Valid Equipment | Defined$ Self",
    &[r#""Attach":{"to":"This","what":{"Each":[{"Subtype":"Equipment"}]}}"#]
)]
#[case::host_chosen_by_the_controller_of_the_triggering_card(
    "Enchantment Aura",
    "K:Enchant:Land\nT:Mode$ Taps | ValidCard$ Card.AttachedBy | Execute$ D\nSVar:D:DB$ Destroy | Defined$ TriggeredCardLKICopy | SubAbility$ A\nSVar:A:DB$ Attach | Object$ Self | Chooser$ TriggeredCardController | Choices$ Land | Optional$ True",
    &[r#""who":{"ControllerOf":"Triggering"}"#, r#""Attach":{"to":{"Bound":"attach"},"what":"This"}"#]
)]
fn small_forms_read(#[case] types: &str, #[case] lines: &str, #[case] holds: &[&str]) {
    reads(types, lines, holds);
}

#[rstest]
#[case::spells_you_control_cannot_be_countered(
    "A:SP$ Effect | ReplacementEffects$ R\nSVar:R:Event$ Counter | ValidSA$ Spell.YouCtrl | Layer$ CantHappen",
    &[r#""CantBeCountered":{"Each":["Spell",{"ControlledBy":"You"}]}"#, r#""duration":"UntilEndOfTurn""#]
)]
#[case::prevent_and_put_counters_on_the_recipient(
    "A:SP$ Effect | ValidTgts$ Creature.YouCtrl | ReplacementEffects$ D | RememberObjects$ Targeted\nSVar:D:Event$ DamageDone | Prevent$ True | ActiveZones$ Command | ValidTarget$ Card.IsRemembered | ReplaceWith$ C\nSVar:C:DB$ PutCounter | Defined$ ReplacedTarget | CounterType$ P1P1 | CounterNum$ X\nSVar:X:ReplaceCount$DamageAmount",
    &["PreventAnd", "PutCounters", r#""count":"Triggering""#]
)]
#[case::prevent_and_gain_life_for_the_controller(
    "A:SP$ Effect | ReplacementEffects$ R\nSVar:R:Event$ DamageDone | ValidSource$ Creature.inZoneBattlefield | ReplaceWith$ G | PreventionEffect$ True\nSVar:G:DB$ GainLife | Defined$ You | LifeAmount$ X\nSVar:X:ReplaceCount$DamageAmount",
    &["Shield", "PreventAnd", "GainLife"]
)]
fn spell_shields_read(#[case] lines: &str, #[case] holds: &[&str]) {
    reads("Instant", lines, holds);
}

#[rstest]
#[case::mill_twice_the_damage(
    "R:Event$ DamageDone | ActiveZones$ Battlefield | ValidTarget$ You | ReplaceWith$ M | AlwaysReplace$ True | PreventionEffect$ True\nSVar:M:DB$ Mill | Defined$ ReplacedTarget | NumCards$ X\nSVar:X:ReplaceCount$DamageAmount/Twice",
    &["PreventAnd", r#""Mill":{"count":{"Times":{"by":2,"of":"Triggering"}},"who":"Triggering"}"#]
)]
#[case::the_sources_controller_draws(
    "R:Event$ DamageDone | ActiveZones$ Battlefield | ValidTarget$ Card.Self | ReplaceWith$ D | PreventionEffect$ True\nSVar:D:DB$ Draw | NumCards$ X | Defined$ ReplacedSourceController\nSVar:X:ReplaceCount$DamageAmount",
    &["PreventAnd", r#""Draw":{"count":"Triggering","who":{"ControllerOf":"Dealer"}}"#]
)]
#[case::all_but_one_is_prevented(
    "R:Event$ DamageDone | ActiveZones$ Battlefield | ValidTarget$ You | ReplaceWith$ D | PreventionEffect$ True\nSVar:D:DB$ ReplaceDamage | Amount$ S\nSVar:S:ReplaceCount$DamageAmount/Minus.1",
    &[r#""PreventAllBut":1"#]
)]
#[case::hellbent_doubles_damage(
    "R:Event$ DamageDone | ActiveZones$ Battlefield | ValidSource$ Card.YouCtrl | ValidTarget$ Permanent,Player | Hellbent$ True | ReplaceWith$ D\nSVar:D:DB$ ReplaceEffect | VarName$ DamageAmount | VarValue$ X\nSVar:X:ReplaceCount$DamageAmount/Twice",
    &[r#""Multiply":2"#, r#""condition":{"Compare""#]
)]
#[case::hellbent_skips_the_upkeep(
    "R:Event$ BeginPhase | ActiveZones$ Battlefield | ValidPlayer$ You | Phase$ Upkeep | Skip$ True | Hellbent$ True",
    &[r#""SkipStep":"Upkeep""#, r#""condition":{"Compare""#]
)]
#[case::bottom_of_the_library_without_a_shuffle_word(
    "R:Event$ Moved | ActiveZones$ Battlefield | Destination$ Graveyard | ValidCard$ Card.!token+OwnedBy Player.EnchantedBy | ReplaceWith$ R\nSVar:R:DB$ ChangeZone | Hidden$ True | Origin$ All | Destination$ Library | Defined$ ReplacedCard | LibraryPosition$ -1 | Reveal$ True | Shuffle$ False",
    &[r#""IntoLibrary":{"place":"Bottom","reveal":true}"#]
)]
#[case::an_aura_destroyed_instead_is_sacrificed(
    "K:Enchant:Land\nR:Event$ Destroy | ValidCard$ Card.EnchantedBy | ReplaceWith$ S\nSVar:S:DB$ Sacrifice | SubAbility$ P\nSVar:P:DB$ Pump | Defined$ Enchanted | KW$ Indestructible",
    &[r#""Destroyed":{"what":[{"Is":"Attached"}]}"#, r#""Instead""#]
)]
fn replacements_read(#[case] lines: &str, #[case] holds: &[&str]) {
    reads("Enchantment Aura", lines, holds);
}

#[rstest]
#[case::prevented_counted_by_a_count_in_a_static(
    "R:Event$ DamageDone | ActiveZones$ Battlefield | ValidTarget$ You | ReplaceWith$ D | PreventionEffect$ True\nSVar:D:DB$ ReplaceDamage | Amount$ X\nSVar:X:Count$Valid Creature.YouCtrl",
    "replacement:DamageDone-prevented-amount"
)]
#[case::regeneration_instead_of_destruction(
    "R:Event$ Destroy | ActiveZones$ Battlefield | ValidCard$ Card.Self | Regeneration$ True | ReplaceWith$ R\nSVar:R:DB$ Regeneration | Defined$ ReplacedCard",
    "key:Replacement.Destroy.Regeneration"
)]
fn neighbouring_shapes_stay_gaps(#[case] lines: &str, #[case] gap: &str) {
    let (_, found) = card("Enchantment Aura", lines);
    assert!(found.iter().any(|one| one == gap), "{found:?}");
}

#[test]
fn a_search_back_to_the_library_that_a_later_step_reads_stays_a_gap() {
    let (_, found) = card(
        "Artifact",
        "A:AB$ ChangeZone | Cost$ 1 | Origin$ Library | Destination$ Library | ChangeType$ Land.Basic | RememberChanged$ True | SubAbility$ R\nSVar:R:DB$ ChangeZone | Defined$ Remembered | Origin$ All | Destination$ Battlefield",
    );
    assert_eq!(found, ["key:ChangeZone.LibraryPosition"]);
}

#[test]
fn an_all_attach_that_the_player_may_decline_stays_a_gap() {
    let (_, found) = card(
        "Creature Human",
        "T:Mode$ ChangesZone | Origin$ Any | Destination$ Battlefield | ValidCard$ Card.Self | Execute$ A\nSVar:A:DB$ Attach | Object$ Valid Equipment.YouCtrl | Defined$ Self | Optional$ True",
    );
    assert_eq!(found, ["key:Attach.Object-all"]);
}
