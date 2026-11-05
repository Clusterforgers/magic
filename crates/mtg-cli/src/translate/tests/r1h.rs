//! Goal: the combat words of Forge scripts (block limits, attack taxes, damage that can't be prevented, more blocks, must be blocked) become the rules and modifications of the DSL, and a shape the DSL cannot say stays a named gap.
use rstest::rstest;
use serde_json::{Value, json};

use super::{abilities, gaps};

fn permanent(line: &str) -> String {
    format!("Name:T\nTypes:Creature Bear\nPT:2/2\nS:{line}\n")
}

fn spell(line: &str) -> String {
    format!("Name:T\nTypes:Instant\nA:SP$ {line}\n")
}

#[rstest]
#[case::at_most_one("Mode$ MinMaxBlocker | ValidCard$ Card.Self | Max$ 1", json!("This"), json!({"MaxBlockers": 1}))]
#[case::at_least_three("Mode$ MinMaxBlocker | ValidCard$ Creature.Self | Min$ 3", json!("This"), json!({"MinBlockers": 3}))]
#[case::enchanted("Mode$ MinMaxBlocker | ValidCard$ Creature.EnchantedBy | Max$ 1", json!("Attached"), json!({"MaxBlockers": 1}))]
fn a_block_limit_is_a_modification(
    #[case] line: &str,
    #[case] affected: Value,
    #[case] modification: Value,
) {
    let script = permanent(line);
    assert_eq!(gaps(&script), Vec::<String>::new());
    let found = &abilities(&script)[0]["Static"];
    assert_eq!(found["affected"], affected);
    assert_eq!(found["mods"], json!([modification]));
}

#[rstest]
#[case::all(
    "Mode$ MinMaxBlocker | ValidCard$ Card.Self | Min$ All",
    "key:Static.MinMaxBlocker.Min=All"
)]
#[case::both(
    "Mode$ MinMaxBlocker | ValidCard$ Card.Self | Min$ 2 | Max$ 3",
    "static:MinMaxBlocker-shape"
)]
fn a_block_limit_of_another_shape_is_a_gap(#[case] line: &str, #[case] gap: &str) {
    assert_eq!(gaps(&permanent(line)), vec![gap.to_owned()]);
}

#[rstest]
#[case::mana("Target$ You | Cost$ 2", json!({"attacked": "Player", "mana": "{2}"}))]
#[case::both("Target$ You,Planeswalker.YouCtrl | Cost$ 1", json!({"attacked": "Both", "mana": "{1}"}))]
#[case::walkers("Target$ Planeswalker.YouCtrl | Cost$ 1", json!({"attacked": "Planeswalkers", "mana": "{1}"}))]
#[case::life("Target$ You,Planeswalker.YouCtrl | Cost$ PayLife<2>", json!({"attacked": "Both", "life": 2}))]
#[case::phyrexian("Target$ You,Planeswalker.YouCtrl | Cost$ WP", json!({"attacked": "Both", "mana": "{W/P}"}))]
fn an_attack_tax_is_a_rule(#[case] words: &str, #[case] tax: Value) {
    let script = permanent(&format!(
        "Mode$ CantAttackUnless | ValidCard$ Creature | {words}"
    ));
    assert_eq!(gaps(&script), Vec::<String>::new());
    assert_eq!(
        abilities(&script)[0]["Rule"]["rule"],
        json!({"AttackTax": tax})
    );
}

#[rstest]
#[case::x(
    "ValidCard$ Creature | Target$ You | Cost$ X",
    "cost:Static.CantAttackUnless.X"
)]
#[case::itself(
    "ValidCard$ Card.Self | Target$ You | Cost$ 2",
    "key:Static.CantAttackUnless.ValidCard-of-itself"
)]
#[case::everyone("ValidCard$ Creature | Cost$ 2", "key:Static.CantAttackUnless.Target")]
fn an_attack_tax_the_rule_cannot_say_is_a_gap(#[case] words: &str, #[case] gap: &str) {
    let script = permanent(&format!("Mode$ CantAttackUnless | {words}"));
    assert_eq!(gaps(&script), vec![gap.to_owned()]);
}

#[rstest]
#[case::all("Mode$ CantPreventDamage", json!({}))]
#[case::combat("Mode$ CantPreventDamage | IsCombat$ True", json!({"combat": true}))]
#[case::itself("Mode$ CantPreventDamage | ValidSource$ Card.Self", json!({"source": ["This"]}))]
fn damage_that_cannot_be_prevented_is_a_rule(#[case] line: &str, #[case] rule: Value) {
    let script = permanent(line);
    assert_eq!(gaps(&script), Vec::<String>::new());
    assert_eq!(
        abilities(&script)[0]["Rule"]["rule"],
        json!({"DamageCantBePrevented": rule})
    );
}

#[rstest]
#[case::one("CanBlockAmount$ 1", 1)]
#[case::seven("CanBlockAmount$ 7", 7)]
#[case::any("CanBlockAny$ True", 99)]
fn more_blocks_are_a_modification(#[case] words: &str, #[case] extra: u32) {
    let script = permanent(&format!(
        "Mode$ Continuous | AffectedDefined$ Self | {words}"
    ));
    assert_eq!(gaps(&script), Vec::<String>::new());
    assert_eq!(
        abilities(&script)[0]["Static"]["mods"],
        json!([{"BlocksAdditional": extra}])
    );
}

#[test]
fn a_count_that_is_not_a_number_is_a_gap() {
    let script = permanent("Mode$ Continuous | AffectedDefined$ Self | CanBlockAmount$ X");
    assert_eq!(
        gaps(&script),
        vec!["key:Static.Continuous.CanBlockAmount-count".to_owned()]
    );
}

#[rstest]
#[case::pump("Pump | ValidTgts$ Creature | CanBlockAny$ True", json!({"BlocksAdditional": 99}))]
#[case::amount("Pump | ValidTgts$ Creature | NumAtt$ +3 | CanBlockAmount$ 2", json!({"BlocksAdditional": 2}))]
fn a_pump_can_add_blocks(#[case] line: &str, #[case] modification: Value) {
    let script = spell(line);
    assert_eq!(gaps(&script), Vec::<String>::new());
    let mods = &abilities(&script)[0]["Spell"]["effects"][0]["Continuous"]["mods"];
    assert!(mods.as_array().unwrap().contains(&modification), "{mods}");
}

#[rstest]
#[case::must_be_blocked("HIDDEN CARDNAME must be blocked if able.", json!({"MustBeBlocked": "AtLeastOne"}))]
#[case::by_all("HIDDEN All creatures able to block CARDNAME do so.", json!({"MustBeBlocked": "Every"}))]
fn a_pump_can_force_blocks(#[case] keyword: &str, #[case] quirk: Value) {
    let script = spell(&format!(
        "Pump | ValidTgts$ Creature | NumAtt$ +3 | KW$ {keyword}"
    ));
    assert_eq!(gaps(&script), Vec::<String>::new());
    let effects = &abilities(&script)[0]["Spell"]["effects"];
    assert_eq!(
        effects[0]["Continuous"]["mods"],
        json!([{"ModifyPT": {"power": 3, "toughness": 0}}])
    );
    assert_eq!(
        effects[1]["Rule"],
        json!({"rule": {"Combat": {"who": {"Target": 0}, "quirk": quirk}}, "duration": "UntilEndOfTurn"})
    );
}

#[test]
fn a_pump_that_only_forces_a_block_has_no_continuous_effect() {
    let script = spell("Pump | ValidTgts$ Creature | KW$ HIDDEN CARDNAME must be blocked if able.");
    let effects = &abilities(&script)[0]["Spell"]["effects"];
    assert_eq!(effects.as_array().unwrap().len(), 1);
    assert!(effects[0].get("Rule").is_some());
}

#[test]
fn damage_to_the_pumped_creature_is_prevented_this_turn() {
    let script = spell(
        "Pump | ValidTgts$ Creature | KW$ Prevent all damage that would be dealt to CARDNAME.",
    );
    assert_eq!(gaps(&script), Vec::<String>::new());
    let shield = &abilities(&script)[0]["Spell"]["effects"][0]["Shield"];
    assert_eq!(shield["by"], json!("Prevent"));
    assert_eq!(shield["when"], json!({"Damage": {"to": {"Target": 0}}}));
}

#[rstest]
#[case::cant_be_blocked_by(
    "Mode$ CantBlockBy | ValidAttacker$ Creature.IsRemembered | ValidBlocker$ Creature.powerLE2",
    json!({"Continuous": {"affected": {"Target": 0}, "mods": [{"CantBeBlockedBy": [{"Type": "Creature"}, {"Compare": {"op": "Le", "stat": "Power", "to": 2}}]}], "duration": "UntilEndOfTurn"}})
)]
#[case::cant_block_the_source(
    "Mode$ CantBlockBy | ValidAttacker$ Creature.EffectSource | ValidBlocker$ Creature.IsRemembered",
    json!({"Continuous": {"affected": {"Target": 0}, "mods": [{"CanBlockOnly": [{"Not": {"Is": "This"}}]}], "duration": "UntilEndOfTurn"}})
)]
#[case::must_block(
    "Mode$ MustBlock | ValidCreature$ Card.IsRemembered",
    json!({"Rule": {"rule": {"MustBlock": {"Target": 0}}, "duration": "UntilEndOfTurn"}})
)]
fn an_effect_line_with_a_combat_static_acts_on_the_remembered_creature(
    #[case] static_line: &str,
    #[case] effect: Value,
) {
    let script = format!(
        "Name:T\nTypes:Instant\nA:SP$ Effect | ValidTgts$ Creature | RememberObjects$ Targeted | StaticAbilities$ S | ExileOnMoved$ Battlefield\nSVar:S:{static_line}\n"
    );
    assert_eq!(gaps(&script), Vec::<String>::new());
    assert_eq!(abilities(&script)[0]["Spell"]["effects"][0], effect);
}

fn combat_rule(who: &str, quirk: &str) -> Value {
    json!({"Combat": {"who": who, "quirk": quirk}})
}

#[test]
fn the_untap_choice_is_a_rule_of_the_card() {
    let script =
        "Name:T\nTypes:Artifact\nK:You may choose not to untap CARDNAME during your untap step.\n";
    assert_eq!(gaps(script), Vec::<String>::new());
    assert_eq!(
        abilities(script)[0]["Rule"]["rule"],
        combat_rule("This", "MayStayTapped")
    );
}

#[test]
fn a_pump_until_the_source_untaps_lasts_while_it_is_tapped() {
    let script = "Name:T\nTypes:Artifact\nA:AB$ Pump | Cost$ 2 T | ValidTgts$ Creature | NumAtt$ +2 | Duration$ UntilUntaps\n";
    assert_eq!(gaps(script), Vec::<String>::new());
    let effect = &abilities(script)[0]["Activated"]["effects"][0]["Continuous"];
    assert_eq!(effect["duration"], json!("WhileSourceTapped"));
}

#[rstest]
#[case::itself(
    "Mode$ AssignNoCombatDamage | ValidCard$ Card.Self",
    "This",
    "AssignsNoCombatDamage"
)]
#[case::unblocked(
    "Mode$ AssignCombatDamageAsUnblocked | ValidCard$ Card.Self | Optional$ True",
    "This",
    "AssignsAsUnblocked"
)]
#[case::toughness(
    "Mode$ CombatDamageToughness | ValidCard$ Card.Self",
    "This",
    "AssignsByToughness"
)]
fn a_combat_damage_static_is_a_rule_about_creatures(
    #[case] line: &str,
    #[case] who: &str,
    #[case] quirk: &str,
) {
    let script = permanent(line);
    assert_eq!(gaps(&script), Vec::<String>::new());
    assert_eq!(
        abilities(&script)[0]["Rule"]["rule"],
        combat_rule(who, quirk)
    );
}

#[rstest]
#[case::toughness_above_power("Creature.powerLTtoughness+YouCtrl")]
#[case::equipped_with_toughness_above_power("Creature.EquippedBy+powerLTtoughness")]
fn damage_by_toughness_of_some_creatures_names_them(#[case] valid: &str) {
    let script = permanent(&format!("Mode$ CombatDamageToughness | ValidCard$ {valid}"));
    assert_eq!(gaps(&script), Vec::<String>::new());
    let rule = &abilities(&script)[0]["Rule"]["rule"]["Combat"];
    assert_eq!(rule["quirk"], json!("AssignsByToughness"));
    assert!(rule["who"].get("Each").is_some(), "{rule}");
}

#[test]
fn damage_by_toughness_of_every_creature_stays_a_rule_of_the_players() {
    let script = permanent("Mode$ CombatDamageToughness | ValidCard$ Creature.YouCtrl");
    assert_eq!(
        abilities(&script)[0]["Rule"]["rule"],
        json!({"ToughnessAssignsDamage": "You"})
    );
}

#[test]
fn a_mandatory_assignment_as_unblocked_stays_a_gap() {
    let script = permanent("Mode$ AssignCombatDamageAsUnblocked | ValidCard$ Card.Self");
    assert_eq!(
        gaps(&script),
        vec!["key:Static.AssignCombatDamageAsUnblocked.mandatory".to_owned()]
    );
}

#[test]
fn a_creature_that_assigns_no_combat_damage_this_turn_is_a_rule_for_the_turn() {
    let script = "Name:T\nTypes:Creature Bear\nPT:2/2\nA:AB$ Effect | Cost$ 1 | StaticAbilities$ SNo | SpellDescription$ x\nSVar:SNo:Mode$ AssignNoCombatDamage | ValidCard$ Card.EffectSource | Description$ EFFECTSOURCE assigns no combat damage this turn.\n";
    assert_eq!(gaps(script), Vec::<String>::new());
    let effect = &abilities(script)[0]["Activated"]["effects"][0];
    assert_eq!(
        effect["Rule"],
        json!({"rule": combat_rule("This", "AssignsNoCombatDamage"), "duration": "UntilEndOfTurn"})
    );
}
