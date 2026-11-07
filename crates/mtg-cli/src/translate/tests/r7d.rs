//! Goal: the shield that exiles a target that would die names the kinds the target may be: a creature, or a creature or
//! planeswalker (CR 700.4), so a planeswalker target that would die is exiled too; an Aura that may enchant a noncreature
//! permanent grants only while that permanent is a creature.
use rstest::rstest;
use serde_json::{Value, json};

use super::{abilities, gaps};

#[rstest]
#[case::enchanted_permanent(
    "As long as enchanted permanent is a creature, it has lifelink.",
    Some(json!({"Matches": {"subject": "Attached", "selector": [{"Type": "Creature"}]}}))
)]
#[case::enchanted_creature("Enchanted creature has lifelink.", None)]
fn r7d_an_aura_of_any_permanent_states_the_creature_condition(
    #[case] description: &str,
    #[case] condition: Option<Value>,
) {
    let text = format!(
        "Name:T\nTypes:Enchantment Aura\nK:Enchant:Permanent\nS:Mode$ Continuous | AffectedDefined$ Enchanted | Affected$ Creature | AddKeyword$ Lifelink | Description$ {description}\n"
    );
    assert_eq!(gaps(&text), Vec::<String>::new());
    let found = abilities(&text)
        .into_iter()
        .find_map(|ability| ability.get("Static").cloned())
        .unwrap_or_default();
    assert_eq!(found.get("condition").cloned(), condition, "{found}");
}

#[rstest]
#[case::creature("Creature", json!({"Type": "Creature"}))]
#[case::creature_or_planeswalker(
    "Creature,Planeswalker",
    json!({"AnyOf": [{"Type": "Creature"}, {"Type": "Planeswalker"}]})
)]
fn r7d_the_dying_shield_names_the_target_kinds(#[case] valid: &str, #[case] dying: Value) {
    let text = format!(
        "Name:T\nTypes:Sorcery\nA:SP$ DealDamage | ValidTgts$ {valid} | NumDmg$ 4 | ReplaceDyingDefined$ Targeted\n"
    );
    assert_eq!(gaps(&text), Vec::<String>::new());
    let effects = &abilities(&text)[0]["Spell"]["effects"];
    assert_eq!(
        effects[1]["Shield"]["when"]["ZoneChange"]["what"],
        json!([{"Is": {"Target": 0}}, dying])
    );
}
