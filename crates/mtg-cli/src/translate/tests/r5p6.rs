//! Goal: round 5 piece 6 translates "becomes the chosen color or type" from a value chosen earlier in the ability, and names a gap when the choice is of another kind or missing.
use rstest::rstest;
use serde_json::json;

use super::{abilities, gaps};

fn spell(choose: &str, animate: &str) -> String {
    format!(
        "Name:T\nTypes:Sorcery\nA:SP$ {choose} | Defined$ You | SubAbility$ DBA\nSVar:DBA:DB$ {animate}\n"
    )
}

#[rstest]
#[case::color(
    "ChooseColor",
    "Animate | Defined$ Self | Colors$ ChosenColor | OverwriteColors$ True",
    "SetColorsChosen"
)]
#[case::land(
    "ChooseType | Type$ Basic Land",
    "Animate | ValidTgts$ Land | Types$ ChosenType | RemoveLandTypes$ True",
    "SetLandTypesChosen"
)]
#[case::land_all(
    "ChooseType | Type$ Basic Land",
    "AnimateAll | ValidCards$ Land.YouCtrl | Types$ ChosenType | RemoveLandTypes$ True",
    "SetLandTypesChosen"
)]
#[case::creature(
    "ChooseType | Type$ Creature",
    "Animate | Defined$ Self | Types$ ChosenType | RemoveCreatureTypes$ True",
    "SetCreatureTypesChosen"
)]
fn the_chosen_value_is_set_by_name(
    #[case] choose: &str,
    #[case] animate: &str,
    #[case] node: &str,
) {
    let text = spell(choose, animate);
    assert_eq!(gaps(&text), Vec::<String>::new());
    let found = abilities(&text);
    let effects = &found[0]["Spell"]["effects"];
    let bind = &effects[0]["ChooseValue"]["as"];
    let mods = &effects[1]["Continuous"]["mods"];
    assert_eq!(mods[0][node], *bind);
    assert_eq!(mods.as_array().map(Vec::len), Some(1), "{mods}");
}

#[rstest]
#[case::color_from_a_type(
    "ChooseType | Type$ Creature",
    "Animate | Defined$ Self | Colors$ ChosenColor | OverwriteColors$ True",
    "key:Animate.Colors=ChosenColor"
)]
#[case::land_type_from_a_creature_type(
    "ChooseType | Type$ Creature",
    "Animate | Defined$ Self | Types$ ChosenType | RemoveLandTypes$ True",
    "key:Animate.Types=ChosenType-kind"
)]
#[case::added_not_set(
    "ChooseType | Type$ Creature",
    "Animate | Defined$ Self | Types$ ChosenType",
    "key:Animate.Types=ChosenType-kind"
)]
#[case::color_added_not_set(
    "ChooseColor",
    "Animate | Defined$ Self | Colors$ ChosenColor",
    "key:Animate.Colors-added"
)]
fn a_choice_of_another_kind_is_a_gap(
    #[case] choose: &str,
    #[case] animate: &str,
    #[case] gap: &str,
) {
    assert_eq!(gaps(&spell(choose, animate)), [gap]);
}

#[test]
fn without_a_choice_the_chosen_color_is_a_gap() {
    let text = "Name:T\nTypes:Sorcery\nA:SP$ Animate | Defined$ Self | Colors$ ChosenColor | OverwriteColors$ True\n";
    assert_eq!(gaps(text), ["key:Animate.Colors=ChosenColor"]);
}

const HUSH: &str = "Name:Hush\nTypes:Enchantment\nT:Mode$ SpellCast | ValidCard$ Card | Execute$ TC | TriggerZones$ Battlefield\nSVar:TC:DB$ Counter | Defined$ TriggeredSpellAbility\n";

#[test]
fn a_cast_trigger_counters_the_spell_it_is_about() {
    assert_eq!(gaps(HUSH), Vec::<String>::new());
    let found = abilities(HUSH);
    assert_eq!(
        found[0]["Triggered"]["effects"][0],
        json!({"Counter": {"what": "Triggering"}})
    );
}

#[rstest]
#[case::other_word("Defined$ Targeted")]
#[case::with_a_target("ValidTgts$ Card | Defined$ TriggeredSpellAbility")]
fn another_defined_is_a_gap(#[case] params: &str) {
    let text = HUSH.replace("Defined$ TriggeredSpellAbility", params);
    assert_eq!(gaps(&text), ["key:Counter.Defined"]);
}

#[test]
fn outside_a_cast_trigger_the_triggered_spell_is_a_gap() {
    let text = HUSH.replace("Mode$ SpellCast", "Mode$ ChangesZone");
    assert!(gaps(&text).contains(&"key:Counter.Defined".to_owned()));
}
