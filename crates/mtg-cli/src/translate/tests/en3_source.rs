//! Goal: `ChooseSource` with the effect that holds the choice becomes a choice of an object and a shield that names it
//! (once for "the next time", all turn for "prevent all damage"), and a shape beyond it stays a named gap.
use rstest::rstest;
use serde_json::{Value, json};

use super::{abilities, gaps};

const NEXT_TIME: &str = "SVar:DBEffect:DB$ Effect | ReplacementEffects$ RPreventNextFromSource | SubAbility$ DBCleanup | ConditionDefined$ ChosenCard | ConditionPresent$ Card | ConditionCompare$ GE1\nSVar:RPreventNextFromSource:Event$ DamageDone | ValidSource$ Card.ChosenCardStrict,Emblem.ChosenCard | ValidTarget$ You | ReplaceWith$ ExileEffect | PreventionEffect$ True\nSVar:ExileEffect:DB$ ChangeZone | Defined$ Self | Origin$ Command | Destination$ Exile\nSVar:DBCleanup:DB$ Cleanup | ClearChosenCard$ True\n";

const ALL_DAMAGE: &str = "SVar:DBEffect:DB$ Effect | ReplacementEffects$ RPrevent | SubAbility$ DBCleanup | ConditionDefined$ ChosenCard | ConditionPresent$ Card,Emblem\nSVar:RPrevent:Event$ DamageDone | ValidSource$ Card.ChosenCardStrict,Emblem.ChosenCard | Prevent$ True\nSVar:DBCleanup:DB$ Cleanup | ClearChosenCard$ True\n";

fn card(ability: &str, tail: &str) -> String {
    format!("Name:T\nTypes:Enchantment\nA:{ability}\n{tail}")
}

fn sources(extra: &Value) -> Value {
    let mut from =
        vec![json!({"AnyOf": [{"In": "Battlefield"}, {"In": "Stack"}, {"In": "Command"}]})];
    from.extend(extra.as_array().cloned().unwrap_or_default());
    json!({"Choose": {"from": {"Each": from}, "as": "source"}})
}

fn shield(by: &Value, to: Option<&str>) -> Value {
    let mut damage = json!({"source": [{"Is": {"Bound": "source"}}]});
    if let Some(to) = to {
        damage["to"] = json!(to);
    }
    json!({"Shield": {"when": {"Damage": damage}, "by": by, "duration": "UntilEndOfTurn"}})
}

#[rstest]
#[case::any_source_next_time(
    "AB$ ChooseSource | Cost$ W | Choices$ Card,Emblem | AILogic$ NeedsPrevention | SubAbility$ DBEffect",
    NEXT_TIME,
    json!([sources(&json!([])), shield(&json!("PreventNext"), Some("You"))])
)]
#[case::red_source_next_time(
    "AB$ ChooseSource | Cost$ 1 | Choices$ Card.RedSource | SubAbility$ DBEffect",
    &NEXT_TIME.replace("Card.ChosenCardStrict,Emblem.ChosenCard", "Card.ChosenCardStrict+RedSource"),
    json!([sources(&json!([{"Color": "Red"}])), shield(&json!("PreventNext"), Some("You"))])
)]
#[case::black_or_red_source(
    "AB$ ChooseSource | Cost$ 1 W | Choices$ Card.BlackSource,Card.RedSource | SubAbility$ DBEffect",
    NEXT_TIME,
    json!([
        sources(&json!([{"AnyOf": [{"Color": "Black"}, {"Color": "Red"}]}])),
        shield(&json!("PreventNext"), Some("You"))
    ])
)]
#[case::all_damage_this_turn(
    "AB$ ChooseSource | Cost$ 0 | Choices$ Card,Emblem | SubAbility$ DBEffect",
    ALL_DAMAGE,
    json!([sources(&json!([])), shield(&json!("Prevent"), None)])
)]
fn a_choice_of_a_source_and_its_shield_are_a_choose_and_a_shield(
    #[case] ability: &str,
    #[case] tail: &str,
    #[case] effects: Value,
) {
    let script = card(ability, tail);
    let found = abilities(&script);
    assert_eq!(found.len(), 1, "{:?}", gaps(&script));
    let ability = &found[0]["Activated"];
    assert_eq!(ability["effects"], effects, "{ability}");
}

#[rstest]
#[case::another_chooser(
    "AB$ ChooseSource | Cost$ 1 | Defined$ Opponent | Choices$ Card,Emblem | SubAbility$ DBEffect",
    "key:ChooseSource.Defined"
)]
#[case::unless_cost(
    "AB$ ChooseSource | Cost$ 1 | Choices$ Card,Emblem | UnlessCost$ 1 | UnlessPayer$ Nobody | SubAbility$ DBEffect",
    "defined:Nobody"
)]
#[case::remembered_choice(
    "AB$ ChooseSource | Cost$ 1 | Choices$ Card,Emblem | RememberChosen$ True | SubAbility$ DBEffect",
    "key:ChooseSource.RememberChosen"
)]
#[case::shares_a_colour(
    "AB$ ChooseSource | Cost$ 1 | Choices$ Card.SharesColorWith Imprinted | SubAbility$ DBEffect",
    "valid:SharesColorWith Imprinted"
)]
fn a_choice_beyond_the_plain_source_stays_a_gap(#[case] ability: &str, #[case] gap: &str) {
    let script = card(ability, NEXT_TIME);
    assert_eq!(abilities(&script), Vec::<Value>::new());
    assert_eq!(gaps(&script), vec![gap.to_owned()]);
}
