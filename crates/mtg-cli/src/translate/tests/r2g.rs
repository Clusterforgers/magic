//! Goal: round 2 pack g reads the engine facts it added: a power-up ability (once, cheaper by the permanent's own mana cost while it entered this turn), colors added to the ones an object has, a sacrifice of any number, landwalk ignored, crew and saddle power counted another way, a face-down permanent turned up and a life floor; each reads exactly what the card prints and the lines that differ stay gaps.
#![allow(clippy::unwrap_used)]

use rstest::rstest;
use serde_json::{Value, json};

use super::{abilities, gaps};

fn first(script: &str) -> Value {
    let found = abilities(script);
    assert!(gaps(script).is_empty(), "{:?}", gaps(script));
    found.into_iter().next().unwrap()
}

#[test]
fn a_power_up_ability_is_once_and_cheaper_while_the_permanent_entered_this_turn() {
    let script = "Name:Subject\nManaCost:2 U\nTypes:Artifact Creature Robot\nPT:1/1\nA:AB$ PutCounter | Cost$ 5 U | Defined$ Self | CounterType$ P1P1 | CounterNum$ 3 | PowerUp$ True | SpellDescription$ x\n";
    let ability = &first(script)["Activated"];
    assert_eq!(ability["limit"], "OncePerGame");
    assert_eq!(
        ability["cost"],
        json!([
            {"Mana": "{5}{U}"},
            {"Changed": {"change": "Reduce", "mana": "{2}{U}", "condition":
                {"Compare": {"left": {"Count": ["This", "EnteredThisTurn"]}, "op": "Ge", "right": 1}}}}
        ])
    );
}

#[test]
fn a_power_up_ability_of_a_card_without_a_mana_cost_stays_a_gap() {
    let script = "Name:Subject\nTypes:Creature Robot\nPT:1/1\nA:AB$ PutCounter | Cost$ 5 U | Defined$ Self | CounterType$ P1P1 | CounterNum$ 3 | PowerUp$ True | SpellDescription$ x\n";
    assert_ne!(gaps(script).len(), 0);
}

#[rstest]
#[case::an_animate_adds_the_color(
    "Name:Subject\nTypes:Creature Faerie\nA:AB$ Animate | Cost$ U | ValidTgts$ Permanent | Colors$ Blue | SpellDescription$ x\n",
    "AddColors"
)]
#[case::an_animate_that_overwrites_sets_it(
    "Name:Subject\nTypes:Creature Faerie\nA:AB$ Animate | Cost$ U | ValidTgts$ Permanent | Colors$ Blue | OverwriteColors$ True | SpellDescription$ x\n",
    "SetColors"
)]
fn animated_colors_are_added_or_set_as_the_card_says(#[case] script: &str, #[case] kind: &str) {
    let ability = first(script);
    assert!(
        ability
            .to_string()
            .contains(&format!("\"{kind}\":[\"Blue\"]")),
        "{ability}"
    );
}

#[test]
fn a_static_add_color_adds_colors_to_the_enchanted_creature() {
    let script = "Name:Subject\nManaCost:B\nTypes:Enchantment Aura\nK:Enchant creature\nS:Mode$ Continuous | AffectedDefined$ Enchanted | Affected$ Creature | AddColor$ Black | Description$ x\n";
    let found = abilities(script);
    assert!(
        found
            .iter()
            .any(|a| a.to_string().contains(r#""AddColors":["Black"]"#)),
        "{found:?}"
    );
}

#[rstest]
#[case::any_number(
    "Name:Subject\nTypes:Sorcery\nA:SP$ Sacrifice | Defined$ You | Amount$ SacX | SacValid$ Land | Optional$ True | SpellDescription$ x\nSVar:SacX:Count$Valid Land.YouCtrl\n",
    "SacrificeUpTo"
)]
#[case::one_may_be_declined(
    "Name:Subject\nTypes:Sorcery\nA:SP$ Sacrifice | Defined$ You | SacValid$ Land | Optional$ True | SpellDescription$ x\n",
    "May"
)]
#[case::a_plain_sacrifice(
    "Name:Subject\nTypes:Sorcery\nA:SP$ Sacrifice | Defined$ You | SacValid$ Land | SpellDescription$ x\n",
    "Sacrifice"
)]
fn an_optional_sacrifice_of_several_is_any_number(#[case] script: &str, #[case] verb: &str) {
    let ability = first(script);
    assert!(
        ability.to_string().contains(&format!("\"{verb}\"")),
        "{ability}"
    );
}

#[rstest]
#[case::one_land(
    "S:Mode$ IgnoreLandWalk | ValidKeyword$ Landwalk:Forest | Description$ x",
    r#""IgnoreLandwalk":[{"Subtype":"Forest"}]"#
)]
#[case::every_landwalk("S:Mode$ IgnoreLandWalk | Description$ x", r#""IgnoreAllLandwalk""#)]
#[case::the_blocker_of_an_aura(
    "S:Mode$ IgnoreLandWalk | ValidBlocker$ Creature.EnchantedBy | Secondary$ True | Description$ x",
    r#""BlockIgnoringLandwalk""#
)]
fn landwalk_that_is_ignored_is_a_combat_rule(#[case] line: &str, #[case] quirk: &str) {
    let script = format!("Name:Subject\nTypes:Enchantment\n{line}\n");
    let ability = first(&script);
    assert!(ability.to_string().contains(quirk), "{ability}");
}

#[rstest]
#[case::toughness(
    "Value$ Toughness",
    "Activated.Crew+Vehicle,Activated.Saddle+Mount",
    Some(r#""TapPower":"Toughness""#)
)]
#[case::two_more("Value$ 2", "Activated.Crew+Vehicle", Some(r#""TapPower":{"Plus":2}"#))]
#[case::a_station_is_another_rule("Value$ 2", "Activated.Station", None)]
fn crew_power_counts_another_way(
    #[case] value: &str,
    #[case] valid: &str,
    #[case] quirk: Option<&str>,
) {
    let script = format!(
        "Name:Subject\nTypes:Creature Human\nPT:1/1\nS:Mode$ TapPowerValue | ValidSA$ {valid} | ValidCard$ Card.Self | {value} | Description$ x\n"
    );
    match quirk {
        Some(quirk) => assert!(first(&script).to_string().contains(quirk)),
        None => assert_ne!(gaps(&script).len(), 0),
    }
}

#[rstest]
#[case::power_alone("Power$ 0", r#""SetPower":0"#)]
#[case::toughness_alone("Toughness$ 4", r#""SetToughness":4"#)]
#[case::both("Power$ 2 | Toughness$ 3", r#""SetPT":{"power":2,"toughness":3}"#)]
fn animating_sets_the_stats_it_names(#[case] stats: &str, #[case] mods: &str) {
    let script = format!(
        "Name:Subject\nTypes:Creature Wall\nPT:0/4\nA:AB$ Animate | Cost$ U | Defined$ Self | {stats} | SpellDescription$ x\n"
    );
    assert!(first(&script).to_string().contains(mods));
}

#[test]
fn a_creature_that_must_block_is_a_rule_for_the_turn() {
    let script = "Name:Subject\nManaCost:1 R\nTypes:Instant\nA:SP$ Effect | ValidTgts$ Creature | RememberObjects$ Targeted | StaticAbilities$ MustBlock | SpellDescription$ x\nSVar:MustBlock:Mode$ MustBlock | ValidCreature$ Card.IsRemembered | Description$ x\n";
    let ability = first(script).to_string();
    assert!(ability.contains(r#""MustBlock":{"Target":0}"#), "{ability}");
}

#[test]
fn turning_a_face_down_creature_face_up_is_an_effect() {
    let script = "Name:Subject\nManaCost:1 R\nTypes:Instant\nA:SP$ SetState | ValidTgts$ Creature.faceDown+OppCtrl | Mode$ TurnFaceUp | SpellDescription$ x\n";
    let ability = first(script);
    assert!(
        ability
            .to_string()
            .contains(r#""TurnFaceUp":{"what":{"Target":0}}"#),
        "{ability}"
    );
}

#[rstest]
#[case::one(
    "R:Event$ LifeReduced | ActiveZones$ Battlefield | ValidPlayer$ You.lifeGE1 | Result$ LT1 | IsDamage$ True | ReplaceWith$ ReduceLoss | Description$ x",
    1
)]
#[case::seven(
    "R:Event$ LifeReduced | ActiveZones$ Battlefield | ValidPlayer$ You.lifeGE7 | Result$ LT7 | IsDamage$ True | ReplaceWith$ ReduceLoss | Description$ x",
    7
)]
fn a_damage_floor_is_a_rule(#[case] line: &str, #[case] floor: i64) {
    let script = format!(
        "Name:Subject\nTypes:Creature Spirit\nPT:1/1\n{line}\nSVar:ReduceLoss:DB$ ReplaceEffect | VarName$ Amount | VarValue$ X\nSVar:X:ReplaceCount$Amount/LimitMax.Difference\nSVar:Difference:Count$YourLifeTotal/Minus.{floor}\n"
    );
    let ability = first(&script);
    assert_eq!(
        ability["Rule"]["rule"],
        json!({"LifeFloor": {"who": "You", "floor": floor}})
    );
}

#[test]
fn a_floor_that_differs_from_the_life_it_needs_stays_a_gap() {
    let script = "Name:Subject\nTypes:Creature Spirit\nPT:1/1\nR:Event$ LifeReduced | ActiveZones$ Battlefield | ValidPlayer$ You.lifeGE3 | Result$ LT1 | IsDamage$ True | ReplaceWith$ ReduceLoss | Description$ x\nSVar:ReduceLoss:DB$ ReplaceEffect | VarName$ Amount | VarValue$ X\nSVar:X:ReplaceCount$Amount/LimitMax.Difference\n";
    assert_ne!(gaps(script).len(), 0);
}
