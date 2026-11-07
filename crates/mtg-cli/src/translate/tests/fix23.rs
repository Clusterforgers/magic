//! Goal: rules the Differs triage found wrong match Oracle: a name after an object argument names the outer object, a
//! target the Oracle text limits to "you control" behind an adjective stays a gap, "its other types and colors" adds the
//! color, "permanent spell" is a permanent spell, "Zombie creature spell" is a creature, and a permission used once a turn
//! drops "your turn"; each with a twin that stays as it was.
use rstest::rstest;
use serde_json::{Value, json};

use super::super::card::translate_script;
use super::super::ctx::World;
use super::{gaps, world};
use crate::forge::script::Script;

/// The abilities of the first face, translated in a world that also knows Zombies and Bears.
fn abilities(text: &str) -> Vec<Value> {
    let mut world: World = world();
    world.subtypes.extend(["Zombie", "Bear"].map(str::to_owned));
    world.names.insert("Groom's Finery".to_owned());
    translate_script(&world, &Script::parse(text))
        .faces
        .into_iter()
        .next()
        .map(|face| face.abilities)
        .unwrap_or_default()
}

fn find<'a>(found: &'a [Value], pointer: &str) -> Option<&'a Value> {
    found.iter().find_map(|ability| ability.pointer(pointer))
}

#[rstest]
#[case::name_of_the_equipment(
    "Equipment.AttachedTo Creature.YouCtrl+namedGroom's Finery",
    json!([{"Subtype": "Equipment"}, {"AttachedTo": {"Each": [{"Type": "Creature"}, {"ControlledBy": "You"}]}}, {"Named": "Groom's Finery"}])
)]
#[case::controller_of_the_creature(
    "Aura.AttachedTo Creature.attacking+OppCtrl",
    json!([{"Subtype": "Aura"}, {"AttachedTo": {"Each": [{"Type": "Creature"}, "Attacking", {"ControlledBy": "Opponents"}]}}])
)]
fn a_name_after_an_object_argument_names_the_outer_object(
    #[case] present: &str,
    #[case] counted: Value,
) {
    let script = format!(
        "Name:T\nTypes:Artifact Equipment\nS:Mode$ Continuous | Affected$ Creature.EquippedBy | AddToughness$ 2 | IsPresent$ {present}\n"
    );
    let found = abilities(&script);
    assert_eq!(
        find(&found, "/Static/condition/Compare/left/Count"),
        Some(&counted),
        "{found:?} {:?}",
        gaps(&script)
    );
}

#[rstest]
#[case::blocking_you_control(
    "Creature.blocking",
    "Target blocking creature you control gets +2/+2 until end of turn.",
    true
)]
#[case::blocking_as_printed(
    "Creature.blocking+YouCtrl",
    "Target blocking creature you control gets +2/+2 until end of turn.",
    false
)]
#[case::blocking_any(
    "Creature.blocking",
    "Target blocking creature gets +2/+2 until end of turn.",
    false
)]
#[case::an_adjective_without_a_property(
    "Creature",
    "Target blocking creature you control gets +2/+2 until end of turn.",
    false
)]
fn a_target_limited_to_your_permanents_behind_an_adjective_stays_a_gap(
    #[case] valid: &str,
    #[case] oracle: &str,
    #[case] gapped: bool,
) {
    let script = format!(
        "Name:T\nTypes:Instant\nA:SP$ Pump | ValidTgts$ {valid} | NumAtt$ +2 | NumDef$ +2\nOracle:{oracle}\n"
    );
    let found = gaps(&script);
    assert_eq!(
        found
            .iter()
            .any(|gap| gap == "target:script-wider-than-oracle"),
        gapped,
        "{found:?}"
    );
}

#[rstest]
#[case::other_types_and_colors("in addition to its other types and colors", "AddColors")]
#[case::other_colors_and_types("in addition to its other colors and types", "AddColors")]
#[case::becomes_green("", "SetColors")]
fn an_animation_adds_the_color_only_in_addition_to_its_other_colors(
    #[case] words: &str,
    #[case] mod_name: &str,
) {
    let text = format!(
        "Until end of turn, target token you control becomes a green Bear creature with base power and toughness 4/4 {words}."
    );
    let script = format!(
        "Name:T\nTypes:Creature Elf\nPT:2/4\nA:AB$ Animate | Cost$ 1 | ValidTgts$ Permanent.token+YouCtrl | Power$ 4 | Toughness$ 4 | Types$ Creature,Bear | Colors$ Green | SpellDescription$ {text}\nOracle:{{1}}: {text}\n"
    );
    let found = abilities(&script);
    let mods = find(&found, "/Activated/effects/0/Continuous/mods").cloned();
    let named = mods
        .and_then(|mods| mods.as_array().cloned())
        .unwrap_or_default()
        .iter()
        .any(|one| one.get(mod_name) == Some(&json!(["Green"])));
    assert!(named, "{found:?}");
}

#[rstest]
#[case::permanent_spell("Permanent.YouCtrl", true)]
#[case::any_spell("Card.YouCtrl", false)]
fn a_permanent_spell_target_is_a_spell_of_a_permanent_type(
    #[case] valid: &str,
    #[case] permanent: bool,
) {
    let script = format!(
        "Name:T\nTypes:Creature Spirit\nPT:3/3\nA:AB$ CopySpellAbility | Cost$ 1 | ValidTgts$ {valid} | TargetType$ Spell | Amount$ 3\n"
    );
    let found = abilities(&script);
    let what = find(&found, "/Activated/targets/0/what/Object").cloned();
    let typed = what
        .and_then(|what| what.as_array().cloned())
        .unwrap_or_default()
        .iter()
        .any(|pred| pred.get("AnyOf").is_some());
    assert_eq!(typed, permanent, "{found:?}");
}

#[rstest]
#[case::zombie_creature_once(
    "Zombie creature spell",
    " | MayPlayLimit$ 1",
    json!([{"Type": "Creature"}, {"Subtype": "Zombie"}]),
    None
)]
#[case::zombie_any(
    "Zombie spell",
    " | MayPlayLimit$ 1",
    json!([{"Subtype": "Zombie"}]),
    None
)]
#[case::every_time_on_your_turn(
    "Zombie creature spell",
    "",
    json!([{"Type": "Creature"}, {"Subtype": "Zombie"}]),
    Some(json!("YourTurn"))
)]
fn a_creature_spell_of_a_type_from_the_graveyard_is_a_creature_and_once_needs_no_turn(
    #[case] spell: &str,
    #[case] limit: &str,
    #[case] what: Value,
    #[case] condition: Option<Value>,
) {
    let script = format!(
        "Name:T\nTypes:Legendary Creature Human\nPT:4/4\nS:Mode$ Continuous | Affected$ Zombie.YouCtrl | Condition$ PlayerTurn | MayPlay$ True{limit} | AffectedZone$ Graveyard | Description$ During each of your turns, you may cast a {spell} from your graveyard.\nOracle:During each of your turns, you may cast a {spell} from your graveyard.\n"
    );
    let found = abilities(&script);
    assert_eq!(
        find(&found, "/Rule/rule/MayCast/what"),
        Some(&what),
        "{found:?}"
    );
    assert_eq!(find(&found, "/Rule/condition").cloned(), condition);
}
