//! Goal: piece P5 (verbs): an unread static mode stays a gap named after the mode; `Unless` keys and the may wrapper are read
//! in one place and mean what the card prints.
use rstest::rstest;
use serde_json::{Value, json};

use super::{abilities, gaps};

#[test]
fn unknown_static_mode_is_a_gap() {
    let found = gaps("Name:T\nTypes:Enchantment\nS:Mode$ Nonsense | Affected$ Creature\n");
    assert_eq!(found, ["static:Nonsense"]);
}

fn spell(line: &str) -> String {
    format!(
        "Name:T\nTypes:Sorcery\nA:SP$ {line}\nSVar:Sub:DB$ LoseLife | LifeAmount$ 2\nSVar:Main:DB$ GainLife | LifeAmount$ 3\n"
    )
}

#[rstest]
#[case::not_paid(
    "GainLife | LifeAmount$ 3 | UnlessCost$ 2 | UnlessPayer$ You",
    json!({"Pay": {"cost": "{2}", "unpaid": [{"GainLife": {"amount": 3}}]}})
)]
#[case::switched(
    "GainLife | LifeAmount$ 3 | UnlessCost$ 2 | UnlessPayer$ You | UnlessSwitched$ True",
    json!({"Pay": {"cost": "{2}", "paid": [{"GainLife": {"amount": 3}}]}})
)]
#[case::switched_other_payer(
    "GainLife | LifeAmount$ 3 | UnlessCost$ 2 | UnlessPayer$ Targeted | ValidTgts$ Player | UnlessSwitched$ True",
    json!({"Pay": {"who": {"Target": 0}, "cost": "{2}", "paid": [{"GainLife": {"who": {"Target": 0}, "amount": 3}}]}})
)]
#[case::subs_when_not_paid(
    "GainLife | LifeAmount$ 3 | UnlessCost$ 2 | UnlessPayer$ You | UnlessResolveSubs$ WhenNotPaid | SubAbility$ Sub",
    json!({"Pay": {"cost": "{2}",
        "unpaid": [{"GainLife": {"amount": 3}}, {"LoseLife": {"amount": 2}}]}})
)]
#[case::subs_when_paid(
    "GainLife | LifeAmount$ 3 | UnlessCost$ 2 | UnlessPayer$ You | UnlessResolveSubs$ WhenPaid | SubAbility$ Sub",
    json!({"Pay": {"cost": "{2}",
        "paid": [{"LoseLife": {"amount": 2}}], "unpaid": [{"GainLife": {"amount": 3}}]}})
)]
#[case::switched_subs_when_not_paid(
    "GainLife | LifeAmount$ 3 | UnlessCost$ 2 | UnlessPayer$ You | UnlessSwitched$ True | UnlessResolveSubs$ WhenNotPaid | SubAbility$ Sub",
    json!({"Pay": {"cost": "{2}",
        "paid": [{"GainLife": {"amount": 3}}], "unpaid": [{"LoseLife": {"amount": 2}}]}})
)]
#[case::subs_always(
    "GainLife | LifeAmount$ 3 | UnlessCost$ 2 | UnlessPayer$ You | SubAbility$ Sub",
    json!({"Pay": {"cost": "{2}", "unpaid": [{"GainLife": {"amount": 3}}]}})
)]
fn unless_branches(#[case] line: &str, #[case] pay: Value) {
    let found = abilities(&spell(line));
    assert_eq!(found.len(), 1);
    let effects = &found[0]["Spell"]["effects"];
    assert_eq!(effects[0], pay);
    let subs_always = line.contains("SubAbility$ Sub") && !line.contains("UnlessResolveSubs");
    assert_eq!(
        effects.as_array().map(Vec::len),
        Some(1 + usize::from(subs_always))
    );
}

#[rstest]
#[case::group(
    "GainLife | LifeAmount$ 3 | UnlessCost$ 2 | UnlessPayer$ Player",
    "key:Unless.UnlessPayer=Player"
)]
#[case::group_opponents(
    "GainLife | LifeAmount$ 3 | UnlessCost$ 2 | UnlessPayer$ Opponent",
    "key:Unless.UnlessPayer=Opponent"
)]
#[case::resolve_value(
    "GainLife | LifeAmount$ 3 | UnlessCost$ 2 | UnlessPayer$ You | UnlessResolveSubs$ Sometimes",
    "key:Unless.UnlessResolveSubs=Sometimes"
)]
#[case::switch_without_cost(
    "GainLife | LifeAmount$ 3 | UnlessSwitched$ True",
    "key:Unless.without-UnlessCost"
)]
fn unless_gaps(#[case] line: &str, #[case] gap: &str) {
    assert_eq!(gaps(&spell(line)), [gap]);
}

#[rstest]
#[case::decider(
    "GainLife | LifeAmount$ 3 | OptionalDecider$ You",
    json!({"May": {"effects": [{"GainLife": {"amount": 3}}]}})
)]
#[case::optional_draw(
    "Draw | NumCards$ 1 | Optional$ True",
    json!({"May": {"effects": [{"Draw": {}}]}})
)]
#[case::decider_target(
    "GainLife | ValidTgts$ Player | LifeAmount$ 3 | OptionalDecider$ Targeted",
    json!({"May": {"who": {"Target": 0}, "effects": [{"GainLife": {"who": {"Target": 0}, "amount": 3}}]}})
)]
fn may_wrapper(#[case] line: &str, #[case] node: Value) {
    let found = abilities(&spell(line));
    assert_eq!(found[0]["Spell"]["effects"][0], node);
}

#[rstest]
#[case::group_decider(
    "GainLife | LifeAmount$ 3 | OptionalDecider$ Opponent",
    "key:May.OptionalDecider=Opponent"
)]
#[case::with_sub("Draw | Optional$ True | SubAbility$ Sub", "key:May.with-SubAbility")]
#[case::targeted_optional(
    "Draw | Optional$ True | ValidTgts$ Player",
    "key:May.Optional-decider"
)]
#[case::other_player(
    "Draw | Optional$ True | Defined$ Opponent",
    "key:May.Optional-decider"
)]
#[case::not_a_may_api(
    "Destroy | ValidTgts$ Creature | Optional$ True",
    "key:Destroy.Optional"
)]
fn may_gaps(#[case] line: &str, #[case] gap: &str) {
    assert_eq!(gaps(&spell(line)), [gap]);
}

fn plain(line: &str) -> String {
    format!("Name:T\nTypes:Sorcery\nA:SP$ {line}\n")
}

#[rstest]
#[case::attach_self_to_target(
    "Attach | ValidTgts$ Creature",
    json!({"Attach": {"what": "This", "to": {"Target": 0}}})
)]
#[case::attach_defined_host("Attach | Defined$ Enchanted", json!({"Attach": {"what": "This", "to": "Attached"}}))]
#[case::attach_object(
    "Attach | Object$ Self | ValidTgts$ Creature",
    json!({"Attach": {"what": "This", "to": {"Target": 0}}})
)]
#[case::attach_target_to_host(
    "Attach | ValidTgts$ Equipment | Object$ Targeted | Defined$ Self",
    json!({"Attach": {"what": {"Target": 0}, "to": "This"}})
)]
#[case::attach_optional(
    "Attach | ValidTgts$ Creature | Optional$ True",
    json!({"May": {"effects": [{"Attach": {"what": "This", "to": {"Target": 0}}}]}})
)]
fn attach_words(#[case] line: &str, #[case] node: Value) {
    let found = abilities(&plain(line));
    assert_eq!(found[0]["Spell"]["effects"][0], node);
}

#[rstest]
#[case::all(
    "Attach | Object$ Valid Equipment | Defined$ Self",
    "key:Attach.Object-all"
)]
#[case::both_hosts(
    "Attach | Object$ Self | ValidTgts$ Creature | Defined$ Self",
    "key:Attach.Object-shape"
)]
#[case::nothing("Attach", "key:Attach.no-ValidTgts")]
fn attach_gaps(#[case] line: &str, #[case] gap: &str) {
    assert_eq!(gaps(&plain(line)), [gap]);
}

#[rstest]
#[case::you("GainControl | ValidTgts$ Creature | NewController$ You", "You")]
#[case::default("GainControl | ValidTgts$ Creature", "You")]
#[case::other(
    "GainControl | ValidTgts$ Creature | NewController$ DefendingPlayer",
    "Defending"
)]
fn gain_control_new_controller(#[case] line: &str, #[case] who: &str) {
    let found = abilities(&plain(line));
    assert_eq!(
        found[0]["Spell"]["effects"][0]["Continuous"]["mods"][0]["SetController"],
        who
    );
}

#[rstest]
#[case::keyword("PutCounter | ValidTgts$ Creature | CounterType$ Flying", "flying")]
#[case::spaced(
    "PutCounter | ValidTgts$ Creature | CounterType$ First Strike",
    "first strike"
)]
#[case::experience("PutCounter | Defined$ You | CounterType$ Experience", "experience")]
fn keyword_and_player_counters(#[case] line: &str, #[case] name: &str) {
    let found = abilities(&plain(line));
    assert_eq!(
        found[0]["Spell"]["effects"][0]["PutCounters"]["counter"],
        json!({"Other": name})
    );
}

#[rstest]
#[case::unknown(
    "PutCounter | ValidTgts$ Creature | CounterType$ Wibble",
    "key:PutCounter.CounterType"
)]
#[case::any(
    "RemoveCounter | ValidTgts$ Creature | CounterType$ Any",
    "key:RemoveCounter.CounterType"
)]
fn unknown_counter_words(#[case] line: &str, #[case] gap: &str) {
    assert_eq!(gaps(&plain(line)), [gap]);
}

#[test]
fn a_group_cannot_take_control() {
    let line = "GainControl | ValidTgts$ Creature | NewController$ Player.Opponent";
    assert_eq!(
        gaps(&plain(line)),
        ["key:GainControl.NewController=Player.Opponent"]
    );
}

#[test]
fn the_sacrifice_prompt_is_text_only() {
    let line = "Sacrifice | SacValid$ Creature | SacMessage$ Creature";
    assert_eq!(gaps(&plain(line)), Vec::<String>::new());
}
