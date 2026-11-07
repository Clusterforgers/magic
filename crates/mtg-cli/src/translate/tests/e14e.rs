//! Goal: round 14 pack e14e reads a copy of the ability an activation put on the stack, casts during a resolution and
//! permissions that pay another cost rather than the mana cost, delayed triggers that last until the controller's next
//! turn, and leaves the neighbouring shapes a gap by name.
#![allow(clippy::unwrap_used)]

use rstest::rstest;

use super::{abilities, gaps};

/// The translation of a card with the given type line, script lines and Oracle text, as compact JSON, and its gaps.
fn card(types: &str, lines: &str, oracle: &str) -> (String, Vec<String>) {
    let text = format!("Name:Subject\nTypes:{types}\nPT:2/2\n{lines}\nOracle:{oracle}\n");
    (
        serde_json::to_string(&abilities(&text)).unwrap(),
        gaps(&text),
    )
}

const RINGS: &str = "T:Mode$ AbilityCast | ValidActivatingPlayer$ You | ValidSA$ SpellAbility.!ManaAbility | TriggerZones$ Battlefield | Execute$ TrigCopy | OptionalDecider$ You | TriggerDescription$ x\nSVar:TrigCopy:AB$ CopySpellAbility | Cost$ 2 | Defined$ TriggeredSpellAbility | MayChooseTarget$ True";

#[rstest]
#[case::paid_for(RINGS, r#"{"CopyAbility":{"new_targets":true,"what":"Dealer"}}"#)]
#[case::of_equipped_creature(
    "T:Mode$ AbilityCast | ValidCard$ Creature.EquippedBy | ValidSA$ SpellAbility.!ManaAbility | TriggerZones$ Battlefield | Execute$ TrigCopy | TriggerDescription$ x\nSVar:TrigCopy:DB$ CopySpellAbility | Defined$ TriggeredSpellAbility",
    r#"{"CopyAbility":{"what":"Dealer"}}"#
)]
fn an_activated_ability_is_copied(#[case] lines: &str, #[case] part: &str) {
    let (json, gaps) = card(
        "Artifact",
        lines,
        "Whenever you activate an ability, if it isn't a mana ability, you may pay {2}. If you do, copy that ability. You may choose new targets for the copy.",
    );
    assert!(gaps.is_empty(), "{gaps:?}");
    assert!(json.contains(part), "{json}");
}

#[rstest]
#[case::another_controller("Controller$ Opponent", "key:CopySpellAbility.Controller")]
#[case::optional("Optional$ True", "key:CopySpellAbility.Optional-of-an-ability")]
fn the_neighbours_of_a_copied_ability_stay_gaps(#[case] key: &str, #[case] gap: &str) {
    let lines = format!(
        "T:Mode$ AbilityCast | ValidActivatingPlayer$ You | ValidSA$ SpellAbility.!ManaAbility | TriggerZones$ Battlefield | Execute$ TrigCopy | TriggerDescription$ x\nSVar:TrigCopy:DB$ CopySpellAbility | Defined$ TriggeredSpellAbility | {key}"
    );
    let (_, gaps) = card(
        "Artifact",
        &lines,
        "Whenever you activate an ability, copy that ability.",
    );
    assert!(gaps.iter().any(|g| g.starts_with(gap)), "{gaps:?}");
}

#[rstest]
#[case::until_your_next_turn("UntilYourNextTurn", r#""once":false,"until":"UntilYourNextTurn""#)]
#[case::this_turn("EOT", r#""once":false,"this_turn":true"#)]
fn a_trigger_of_an_effect_lasts_its_duration(#[case] duration: &str, #[case] part: &str) {
    let lines = format!(
        "A:SP$ Effect | Triggers$ TrigTaps | Duration$ {duration}\nSVar:TrigTaps:Mode$ Taps | ValidCard$ Creature | Execute$ TrigDestroy | TriggerDescription$ x\nSVar:TrigDestroy:DB$ Destroy | Defined$ TriggeredCard"
    );
    let (json, gaps) = card(
        "Sorcery",
        &lines,
        "Until your next turn, whenever a creature becomes tapped, destroy it.",
    );
    assert!(gaps.is_empty(), "{gaps:?}");
    assert!(json.contains(part), "{json}");
}

#[test]
fn damage_of_either_creature_until_your_next_turn_stays_a_gap() {
    let lines = "A:SP$ Effect | ValidTgts$ Creature | TargetMin$ 0 | TargetMax$ 2 | Triggers$ TrigDamage | RememberObjects$ Targeted | Duration$ UntilYourNextTurn\nSVar:TrigDamage:Mode$ DamageDealtOnce | CombatDamage$ True | ValidSource$ Creature.IsRemembered | Execute$ TrigDraw | TriggerDescription$ x\nSVar:TrigDraw:DB$ Draw | NumCards$ 1";
    let (_, gaps) = card(
        "Sorcery",
        lines,
        "Choose up to two target creatures. Until your next turn, whenever either of those creatures deals combat damage, you draw a card.",
    );
    assert!(
        gaps.iter()
            .any(|g| g == "trigger:DamageDealtOnce-for-each-source"),
        "{gaps:?}"
    );
}

const LIFE_EQUAL: &str = r#""rather":[{"PayLife":{"Stat":{"of":"This","stat":"ManaValue"}}}]"#;

#[rstest]
#[case::from_the_top_of_the_library(
    "S:Mode$ Continuous | Affected$ Card.TopLibrary+YouCtrl | AffectedZone$ Library | MayPlay$ True | MayPlayAltManaCost$ PayLife<ConvertedManaCost> | Description$ You may play lands and cast spells from the top of your library.",
    LIFE_EQUAL
)]
#[case::energy(
    "S:Mode$ Continuous | Affected$ Card.TopLibrary+YouCtrl | AffectedZone$ Library | MayPlay$ True | MayPlayAltManaCost$ PayEnergy<ConvertedManaCost> | Description$ You may cast spells from the top of your library.",
    r#""rather":[{"PayEnergy":{"Stat":{"of":"This","stat":"ManaValue"}}}]"#
)]
fn a_permission_pays_another_cost(#[case] lines: &str, #[case] part: &str) {
    let (json, gaps) = card(
        "Artifact",
        lines,
        "If you cast a spell this way, pay life equal to its mana value rather than pay its mana cost.",
    );
    assert!(gaps.is_empty(), "{gaps:?}");
    assert!(json.contains(part), "{json}");
}

#[rstest]
#[case::life("PayLife<ConvertedManaCost>", LIFE_EQUAL)]
#[case::a_discard("Discard<1/Card>", r#""rather":[{"Discard":{}}]"#)]
#[case::mana("1", r#""rather":"{1}""#)]
fn a_cast_during_a_resolution_pays_another_cost(#[case] cost: &str, #[case] part: &str) {
    let lines = format!(
        "A:SP$ Play | ValidTgts$ Creature.YouOwn | TgtZone$ Graveyard | ValidSA$ Spell | Optional$ True | PlayCost$ {cost}"
    );
    let (json, gaps) = card(
        "Sorcery",
        &lines,
        "You may cast target creature card from your graveyard by paying life equal to its mana value rather than paying its mana cost.",
    );
    assert!(gaps.is_empty(), "{gaps:?}");
    assert!(json.contains(part), "{json}");
}

#[rstest]
#[case::its_suspend_cost(
    "A:SP$ Play | ValidTgts$ Creature.YouOwn | TgtZone$ Graveyard | ValidSA$ Spell | Optional$ True | PlayCost$ SuspendCost",
    "key:PlayCost=SuspendCost"
)]
#[case::free_as_well(
    "A:SP$ Play | ValidTgts$ Creature.YouOwn | TgtZone$ Graveyard | ValidSA$ Spell | Optional$ True | WithoutManaCost$ True | PlayCost$ 1",
    "key:Play.PlayCost-free"
)]
fn another_cost_the_engine_does_not_pay_stays_a_gap(#[case] lines: &str, #[case] gap: &str) {
    let (_, gaps) = card("Sorcery", lines, "You may cast target creature card.");
    assert!(gaps.iter().any(|g| g == gap), "{gaps:?}");
}

const ZADA: &str = "T:Mode$ SpellCast | ValidCard$ Instant,Sorcery | ValidActivatingPlayer$ You | TargetsValid$ Card.Self | IsSingleTarget$ True | TriggerZones$ Battlefield | Execute$ TrigCopy | TriggerDescription$ x\nSVar:TrigCopy:DB$ CopySpellAbility | Defined$ TriggeredSpellAbility | Controller$ You";

#[rstest]
#[case::each_creature_you_control(
    "CopyForEachCanTarget$ Creature.YouCtrl",
    r#""each_target":[{"Type":"Creature"},{"ControlledBy":"You"}]"#
)]
#[case::each_creature(
    "CopyForEachCanTarget$ Creature.inZoneBattlefield",
    r#""each_target":[{"Type":"Creature"},{"In":"Battlefield"}]"#
)]
fn a_spell_is_copied_for_each_other_target(#[case] key: &str, #[case] part: &str) {
    let (json, gaps) = card(
        "Creature",
        &format!("{ZADA} | {key}"),
        "Whenever you cast an instant or sorcery spell that targets only this creature, copy that spell for each other creature you control that the spell could target. Each copy targets a different one of those creatures.",
    );
    assert!(gaps.is_empty(), "{gaps:?}");
    assert!(json.contains(part), "{json}");
}

#[rstest]
#[case::with_a_count(
    "CopyForEachCanTarget$ Creature | Amount$ 2",
    "key:CopySpellAbility.CopyForEachCanTarget-with-a-count"
)]
#[case::only_one_of_many(
    "CopyForEachCanTarget$ Creature | ChooseOnlyOne$ True",
    "key:CopySpellAbility.ChooseOnlyOne"
)]
fn the_neighbours_of_a_copy_for_each_target_stay_gaps(#[case] key: &str, #[case] gap: &str) {
    let (_, gaps) = card("Creature", &format!("{ZADA} | {key}"), "x");
    assert!(gaps.iter().any(|g| g == gap), "{gaps:?}");
}

#[rstest]
#[case::equip_that_targets_this(
    "S:Mode$ ReduceCost | ValidTarget$ Card.Self | ValidSpell$ Activated.Equip | Activator$ You | Amount$ 3 | Description$ x",
    r#""activating":"Equip","applies_to":[{"ControlledBy":"You"}],"change":"Reduce","mana":"{3}","targeting":["This"]"#
)]
#[case::its_own_equip_that_targets_a_colorless_creature(
    "S:Mode$ ReduceCost | ValidTarget$ Creature.Colorless | ValidSpell$ Activated.Equip | Amount$ 2 | ValidCard$ Card.Self | Description$ x",
    r#""targeting":[{"Type":"Creature"},"Colorless"]"#
)]
fn an_ability_cost_reads_the_target_of_the_ability(#[case] lines: &str, #[case] part: &str) {
    let (json, gaps) = card(
        "Creature",
        lines,
        "Equip abilities you activate that target this creature cost {3} less to activate.",
    );
    assert!(gaps.is_empty(), "{gaps:?}");
    assert!(json.contains(part), "{json}");
    assert!(!json.contains(r#"{"Targeting""#), "{json}");
}
