//! Goal: the mappers of the fg3 family turn invented Forge-format lines into the DSL nodes the engine has and leave the rest as gaps.
use rstest::rstest;
use serde_json::{Value, json};

use super::{abilities, gaps, normal};

#[rstest]
#[case::conjunction_among_alternatives(
    "Name:T\nTypes:Sorcery\nA:SP$ Destroy | ValidTgts$ Creature.Red+withFlying,Artifact\n",
    json!({"Spell": {"targets": [{"what": {"Object": [{"AnyOf": [
        {"Not": {"AnyOf": [{"Not": {"Type": "Creature"}}, {"Not": {"Color": "Red"}}, {"Not": {"HasKeyword": "Flying"}}]}},
        {"Type": "Artifact"}]}]}}],
        "effects": [{"Destroy": {"what": {"Target": 0}}}]}})
)]
#[case::each_opponent_by_a_count_of_their_own(
    "Name:T\nTypes:Sorcery\nA:SP$ RepeatEach | RepeatPlayers$ Player.Opponent | RepeatSubAbility$ Dmg\nSVar:Dmg:DB$ DealDamage | Defined$ Remembered | NumDmg$ X\nSVar:X:Count$Valid Island.RememberedPlayerCtrl\n",
    json!({"Spell": {"effects": [{"RepeatEach": {"over": "Opponents", "as": "each", "effects": [
        {"DealDamage": {"to": {"Bound": "each"}, "amount": {"Count": [{"Subtype": "Island"}, {"ControlledBy": {"Bound": "each"}}]}}}]}}]}})
)]
#[case::choose_a_color_then_sacrifice_creatures_of_it(
    "Name:T\nTypes:Sorcery\nA:SP$ ChooseColor | Defined$ You | SubAbility$ Sac\nSVar:Sac:DB$ DestroyAll | ValidCards$ Creature.ChosenColor\n",
    json!({"Spell": {"effects": [
        {"ChooseValue": {"kind": "Color", "as": "chosen"}},
        {"Destroy": {"what": {"Each": [{"Type": "Creature"}, {"ChosenAs": "chosen"}]}}}]}})
)]
#[case::choose_a_card_and_destroy_it(
    "Name:T\nTypes:Sorcery\nA:SP$ ChooseCard | Defined$ You | Choices$ Creature.OppCtrl | Mandatory$ True | SubAbility$ Kill\nSVar:Kill:DB$ Destroy | Defined$ ChosenCard\n",
    json!({"Spell": {"effects": [
        {"Choose": {"from": {"Each": [{"Type": "Creature"}, {"ControlledBy": "Opponents"}]}, "as": "card"}},
        {"Destroy": {"what": {"Bound": "card"}}}]}})
)]
#[case::choose_up_to_two_at_random_from_a_graveyard(
    "Name:T\nTypes:Sorcery\nA:SP$ ChooseCard | Choices$ Creature.YouOwn | ChoiceZone$ Graveyard | Amount$ 2 | AtRandom$ True | SubAbility$ Back\nSVar:Back:DB$ ChangeZone | Origin$ Graveyard | Destination$ Hand | Defined$ ChosenCard\n",
    json!({"Spell": {"effects": [
        {"ChooseAtRandom": {"from": {"Each": [{"Type": "Creature"}, {"OwnedBy": "You"}, {"In": "Graveyard"}]}, "count": 2, "as": "card"}},
        {"Move": {"what": {"Bound": "card"}, "to": "Hand"}}]}})
)]
#[case::put_a_creature_from_the_hand_onto_the_battlefield(
    "Name:T\nTypes:Sorcery\nA:SP$ ChangeZone | Origin$ Hand | Destination$ Battlefield | ChangeType$ Creature | SpellDescription$ You may put a creature card from your hand onto the battlefield.\n",
    json!({"Spell": {"effects": [
        {"Choose": {"from": {"Each": [{"Type": "Creature"}, {"In": "Hand"}, {"OwnedBy": "You"}]}, "count": {"min": 0, "max": 1}, "as": "picked"}},
        {"Move": {"what": {"Bound": "picked"}, "to": {"Battlefield": {}}}}]}})
)]
#[case::roll_a_d6_with_a_table(
    "Name:T\nTypes:Sorcery\nA:SP$ RollDice | Sides$ 6 | ResultSubAbilities$ 1-2:Low,6:High\nSVar:Low:DB$ GainLife | LifeAmount$ 1\nSVar:High:DB$ Draw | NumCards$ 2\n",
    json!({"Spell": {"effects": [{"RollDice": {"sides": 6, "results": [
        {"from": 1, "to": 2, "effects": [{"GainLife": {"amount": 1}}]},
        {"from": 6, "to": 6, "effects": [{"Draw": {"count": 2}}]}]}}]}})
)]
#[case::pay_and_when_you_do_a_trigger_with_its_own_target(
    "Name:T\nTypes:Creature Elf\nT:Mode$ Attacks | ValidCard$ Card.Self | Execute$ Imm\nSVar:Imm:DB$ ImmediateTrigger | Execute$ Hit | UnlessCost$ 1 | UnlessPayer$ You | UnlessSwitched$ True\nSVar:Hit:DB$ DealDamage | ValidTgts$ Creature | NumDmg$ 2\n",
    json!({"Triggered": {"when": {"Attacks": {"what": [ "This" ]}}, "effects": [
        {"Pay": {"cost": "{1}", "paid": [{"Reflexive": {
            "targets": [{"what": {"Object": [{"Type": "Creature"}]}}],
            "effects": [{"DealDamage": {"to": {"Target": 0}, "amount": 2}}]}}]}}]}})
)]
#[case::reveal_the_top_card_and_put_a_land_in_hand(
    "Name:T\nTypes:Sorcery\nA:SP$ PeekAndReveal | PeekAmount$ 1 | RevealValid$ Land | RevealOptional$ True | RememberRevealed$ True | SubAbility$ Take\nSVar:Take:DB$ ChangeZone | Origin$ Library | Destination$ Hand | Defined$ Remembered\n",
    json!({"Spell": {"effects": [
        {"LookAtTop": {"count": 1, "as": "top"}},
        {"Choose": {"from": {"Each": [{"In": "Library"}, {"Is": {"Bound": "top"}}, {"Type": "Land"}]}, "count": {"min": 0, "max": 1}, "as": "revealed2"}},
        {"Reveal": {"what": {"Bound": "revealed2"}}},
        {"Move": {"what": {"Bound": "revealed2"}, "to": "Hand"}}]}})
)]
#[case::discard_one_at_random_to_exile(
    "Name:T\nTypes:Sorcery\nA:SP$ ChangeZone | Origin$ Hand | Destination$ Exile | ChangeType$ Card | AtRandom$ True | Mandatory$ True\n",
    json!({"Spell": {"effects": [
        {"ChooseAtRandom": {"from": {"Each": [{"In": "Hand"}, {"OwnedBy": "You"}]}, "as": "picked"}},
        {"Exile": {"what": {"Bound": "picked"}}}]}})
)]
fn a_construct_with_a_node_maps(#[case] script: &str, #[case] expected: Value) {
    let mut found = abilities(script);
    assert_eq!(found.len(), 1, "{:?}", gaps(script));
    assert_eq!(normal(found.remove(0)), normal(expected));
}
