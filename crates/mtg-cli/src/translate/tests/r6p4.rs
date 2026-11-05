//! Goal: round 6 piece 4 translates a defender-conditioned attack restriction into `Cant`/`Attack` with the condition read for the defending player, and names a gap for a condition it cannot say.
use rstest::rstest;
use serde_json::{Value, json};

use super::{abilities, gaps, normal};

fn card(line: &str) -> String {
    format!("Name:T\nTypes:Creature\nPT:2/2\nS:{line}\n")
}

fn attack(unless: &Value) -> Value {
    normal(
        json!({"Rule": {"rule": {"Cant": {"who": "This", "action": {"Attack": {"unless": unless}}}}}}),
    )
}

fn controls(selector: &Value) -> Value {
    json!({"Compare": {"left": {"Count": selector}, "op": "Ge", "right": 1}})
}

#[rstest]
#[case::island("controlsIsland", controls(&json!([{"Subtype": "Island"}, {"ControlledBy": "You"}])))]
#[case::monarch("isMonarch", json!({"Monarch": "You"}))]
fn the_condition_is_read_for_the_defender(#[case] word: &str, #[case] unless: Value) {
    let line = format!("Mode$ CantAttack | ValidCard$ Card.Self | UnlessDefender$ {word}");
    assert_eq!(gaps(&card(&line)), Vec::<String>::new());
    assert_eq!(abilities(&card(&line)), vec![attack(&unless)]);
}

#[rstest]
#[case::negated("!controlsLand.untapped")]
#[case::properties("controlsCreature.untapped+powerGE3")]
fn a_negation_and_properties_translate(#[case] word: &str) {
    let line = format!("Mode$ CantAttack | ValidCard$ Card.Self | UnlessDefender$ {word}");
    assert_eq!(gaps(&card(&line)), Vec::<String>::new());
}

#[rstest]
#[case::poison("IsPoisoned")]
#[case::fewer("hasFewerCreaturesInPlayThanYou")]
#[case::alternatives("controlsEnchantment,Permanent.enchanted")]
#[case::graveyard("HasCardsInGraveyard_Card_GE7")]
fn other_conditions_stay_gaps(#[case] word: &str) {
    let line = format!("Mode$ CantAttack | ValidCard$ Card.Self | UnlessDefender$ {word}");
    assert_eq!(
        gaps(&card(&line)),
        vec![format!("key:Static.CantAttack.UnlessDefender={word}")]
    );
}

fn artifact(line: &str) -> String {
    format!("Name:T\nTypes:Artifact\nS:{line}\n")
}

#[rstest]
#[case::equip_you(
    "Mode$ ReduceCost | ValidCard$ Card | ValidSpell$ Activated.Equip | Activator$ You | Amount$ 1",
    json!({"applies_to": [{"ControlledBy": "You"}], "change": "Reduce", "mana": "{1}", "activating": "Equip"})
)]
#[case::self_equip(
    "Mode$ ReduceCost | ValidCard$ Card.Self | ValidSpell$ Activated.Equip | Amount$ 2",
    json!({"applies_to": ["This"], "change": "Reduce", "mana": "{2}", "activating": "Equip"})
)]
#[case::cycling(
    "Mode$ ReduceCost | ValidCard$ Card | ValidSpell$ Activated.Cycling | Activator$ You | Amount$ 2",
    json!({"applies_to": [{"ControlledBy": "You"}], "change": "Reduce", "mana": "{2}", "activating": "Cycling"})
)]
#[case::creatures_with_a_floor(
    "Mode$ ReduceCost | ValidCard$ Creature.YouCtrl | Type$ Ability | Amount$ 2 | MinMana$ 1 | AffectedZone$ Battlefield",
    json!({"applies_to": [{"Type": "Creature"}, {"ControlledBy": "You"}], "change": "Reduce", "mana": "{2}", "activating": "Any", "least": 1})
)]
#[case::on_your_turn(
    "Mode$ ReduceCost | ValidCard$ Card | ValidSpell$ Activated.Equip | Activator$ You | Amount$ 1 | Condition$ PlayerTurn",
    json!({"applies_to": [{"ControlledBy": "You"}], "change": "Reduce", "mana": "{1}", "activating": "Equip", "condition": "YourTurn"})
)]
fn an_activation_cost_modifier_names_its_kind(#[case] line: &str, #[case] modifier: Value) {
    assert_eq!(gaps(&artifact(line)), Vec::<String>::new());
    assert_eq!(
        abilities(&artifact(line)),
        vec![normal(json!({"CostModifier": modifier}))]
    );
}

#[rstest]
#[case::target_of_the_ability(
    "Mode$ ReduceCost | ValidTarget$ Card.Self | ValidSpell$ Activated.Equip | Activator$ You | Amount$ 2",
    "key:Static.CostModifier.ValidTarget-of-an-ability"
)]
#[case::other_kind(
    "Mode$ ReduceCost | ValidCard$ Card | ValidSpell$ Activated.Boast | Activator$ You | Amount$ 1",
    "key:Static.CostModifier.ValidSpell=Activated.Boast"
)]
#[case::literal_relative(
    "Mode$ ReduceCost | ValidCard$ Card.Self | Type$ Spell | Amount$ 2 | Relative$ True",
    "key:Static.CostModifier.Relative-with-a-number"
)]
fn other_cost_modifiers_stay_gaps(#[case] line: &str, #[case] gap: &str) {
    assert_eq!(gaps(&artifact(line)), vec![gap.to_owned()]);
}

#[rstest]
#[case::first_creature_spell(
    "Mode$ ReduceCost | EffectZone$ Battlefield | ValidCard$ Card.Creature | Activator$ You | Type$ Spell | OnlyFirstSpell$ True | Amount$ 2"
)]
fn the_first_spell_of_a_kind_each_turn_counts_the_earlier_ones(#[case] line: &str) {
    assert_eq!(gaps(&artifact(line)), Vec::<String>::new());
    let found = abilities(&artifact(line));
    let text = found[0].to_string();
    assert!(
        text.contains(r#""event":"SpellCast""#) && text.contains(r#""right":0"#),
        "{text}"
    );
}

#[rstest]
#[case::loyalty_by_opponents(
    "Mode$ CantBeActivated | Activator$ Opponent | ValidCard$ Planeswalker | ValidSA$ Activated.Loyalty",
    json!({"who": "Opponents", "action": {"ActivateKind": {"kind": "Loyalty", "sources": [{"Type": "Planeswalker"}]}}})
)]
#[case::cycling_by_everyone(
    "Mode$ CantBeActivated | ValidSA$ Activated.Cycling",
    json!({"who": "EachPlayer", "action": {"ActivateKind": {"kind": "Cycling", "sources": []}}})
)]
fn a_prohibition_names_its_kind(#[case] line: &str, #[case] cant: Value) {
    assert_eq!(gaps(&artifact(line)), Vec::<String>::new());
    assert_eq!(
        abilities(&artifact(line)),
        vec![normal(json!({"Rule": {"rule": {"Cant": cant}}}))]
    );
}

#[rstest]
#[case::all_abilities_include_mana_abilities(
    "Mode$ CantBeActivated | ValidCard$ Artifact | ValidSA$ Activated"
)]
fn all_activated_abilities_stay_a_gap_while_mana_abilities_are_not_forbidden(#[case] line: &str) {
    assert_eq!(
        gaps(&artifact(line)),
        vec!["key:Static.CantBeActivated.ValidSA".to_owned()]
    );
}
