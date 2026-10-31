//! Goal: each mapper turns invented scripts in Forge's format into the DSL's canonical abilities, every construct without
//! a mapper leaves its line out and is named as a gap, and no ability is written that does not parse.
use std::collections::BTreeMap;

use rstest::rstest;
use serde_json::{Value, json};

use super::card::{CardOut, translate_script};
use super::ctx::World;
use crate::forge::script::{Face, Script};

/// A world with the subtypes and tokens the invented scripts use.
fn world() -> World {
    let tokens: BTreeMap<String, Face> = [
        (
            "w_1_1_soldier",
            "Name:Soldier Token\nColors:white\nTypes:Creature Soldier\nPT:1/1\n",
        ),
        (
            "g_2_2_boar_trample",
            "Name:Boar Token\nColors:green\nTypes:Creature Boar\nPT:2/2\nK:Trample\n",
        ),
        (
            "c_a_treasure_sac",
            "Name:Treasure Token\nTypes:Artifact Treasure\n",
        ),
    ]
    .iter()
    .filter_map(|(stem, text)| {
        let face = Script::parse(text).faces.into_iter().next()?;
        Some(((*stem).to_owned(), face))
    })
    .collect();
    let mut world = World::new(&[], tokens);
    world.subtypes.extend(
        [
            "Elf",
            "Goblin",
            "Wizard",
            "Aura",
            "Equipment",
            "Plains",
            "Island",
        ]
        .map(str::to_owned),
    );
    world
}

fn translate(text: &str) -> CardOut {
    translate_script(&world(), &Script::parse(text))
}

/// The abilities of the first face.
fn abilities(text: &str) -> Vec<Value> {
    translate(text)
        .faces
        .into_iter()
        .next()
        .map(|face| face.abilities)
        .unwrap_or_default()
}

fn gaps(text: &str) -> Vec<String> {
    translate(text).gaps().map(|gap| gap.0.clone()).collect()
}

#[rstest]
#[case::damage(
    "Name:Test Bolt\nTypes:Instant\nA:SP$ DealDamage | ValidTgts$ Any | NumDmg$ 3\n",
    json!({"Spell": {"targets": [{"what": "AnyTarget"}],
        "effects": [{"DealDamage": {"to": {"Target": 0}, "amount": 3}}]}})
)]
#[case::draw_and_lose(
    "Name:Test Study\nTypes:Sorcery\nA:SP$ Draw | NumCards$ 2 | SubAbility$ Pain\nSVar:Pain:DB$ LoseLife | LifeAmount$ 2\n",
    json!({"Spell": {"effects": [{"Draw": {"count": 2}}, {"LoseLife": {"amount": 2}}]}})
)]
#[case::destroy_no_regen(
    "Name:Test Doom\nTypes:Sorcery\nA:SP$ Destroy | ValidTgts$ Creature.nonBlack | NoRegen$ True\n",
    json!({"Spell": {"targets": [{"what": {"Object": [{"Type": "Creature"}, {"Not": {"Color": "Black"}}]}}],
        "effects": [{"Destroy": {"what": {"Target": 0}, "regenerate": false}}]}})
)]
#[case::wrath(
    "Name:Test Wrath\nTypes:Sorcery\nA:SP$ DestroyAll | ValidCards$ Creature\n",
    json!({"Spell": {"effects": [{"Destroy": {"what": {"Each": [{"Type": "Creature"}]}}}]}})
)]
#[case::counterspell(
    "Name:Test Denial\nTypes:Instant\nA:SP$ Counter | TargetType$ Spell | ValidTgts$ Card.nonCreature\n",
    json!({"Spell": {"targets": [{"what": {"Object": ["Spell", {"Not": {"Type": "Creature"}}]}}],
        "effects": [{"Counter": {"what": {"Target": 0}}}]}})
)]
#[case::pump(
    "Name:Test Growth\nTypes:Instant\nA:SP$ Pump | ValidTgts$ Creature | NumAtt$ +3 | NumDef$ +3 | KW$ Trample\n",
    json!({"Spell": {"targets": [{"what": {"Object": [{"Type": "Creature"}]}}],
        "effects": [{"Continuous": {"affected": {"Target": 0},
            "mods": [{"ModifyPT": {"power": 3, "toughness": 3}}, {"AddKeyword": "Trample"}],
            "duration": "UntilEndOfTurn"}}]}})
)]
#[case::counters(
    "Name:Test Bloom\nTypes:Sorcery\nA:SP$ PutCounter | ValidTgts$ Creature | CounterType$ P1P1 | CounterNum$ 2\n",
    json!({"Spell": {"targets": [{"what": {"Object": [{"Type": "Creature"}]}}],
        "effects": [{"PutCounters": {"on": {"Target": 0}, "counter": "+1/+1", "count": 2}}]}})
)]
#[case::tokens(
    "Name:Test Muster\nTypes:Sorcery\nA:SP$ Token | TokenAmount$ 2 | TokenScript$ w_1_1_soldier | TokenOwner$ You\n",
    json!({"Spell": {"effects": [{"CreateToken": {"token": {"Custom": {"name": "Soldier",
        "colors": ["White"], "types": ["Creature"], "subtypes": ["Soldier"], "power": 1, "toughness": 1}},
        "count": 2}}]}})
)]
#[case::token_with_keyword(
    "Name:Test Herd\nTypes:Sorcery\nA:SP$ Token | TokenScript$ g_2_2_boar_trample\n",
    json!({"Spell": {"effects": [{"CreateToken": {"token": {"Custom": {"name": "Boar",
        "colors": ["Green"], "types": ["Creature"], "subtypes": ["Boar"], "power": 2, "toughness": 2,
        "abilities": [{"Keyword": "Trample"}]}}}}]}})
)]
#[case::treasure(
    "Name:Test Hoard\nTypes:Sorcery\nA:SP$ Token | TokenScript$ c_a_treasure_sac\n",
    json!({"Spell": {"effects": [{"CreateToken": {"token": {"Named": "treasure"}}}]}})
)]
#[case::mana(
    "Name:Test Rock\nTypes:Artifact\nA:AB$ Mana | Cost$ T | Produced$ C | Amount$ 2\n",
    json!({"Activated": {"cost": ["Tap"], "effects": [{"AddMana": {"mana": {"Fixed": ["Colorless", "Colorless"]}}}]}})
)]
#[case::any_color(
    "Name:Test Bird\nTypes:Creature Bird\nA:AB$ Mana | Cost$ T | Produced$ Any\n",
    json!({"Activated": {"cost": ["Tap"], "effects": [{"AddMana": {"mana": {"AnyColor": 1}}}]}})
)]
#[case::activated_with_costs(
    "Name:Test Altar\nTypes:Artifact\nA:AB$ Draw | Cost$ 2 T Sac<1/Creature> | SorcerySpeed$ True\n",
    json!({"Activated": {"cost": [{"Mana": "{2}"}, "Tap", {"Sacrifice": {"filter": [{"Type": "Creature"}]}}],
        "timing": "Sorcery", "effects": [{"Draw": {}}]}})
)]
#[case::loyalty(
    "Name:Test Walker\nTypes:Legendary Planeswalker Test\nA:AB$ Draw | Cost$ SubCounter<3/LOYALTY> | Planeswalker$ True\n",
    json!({"Loyalty": {"cost": {"Minus": 3}, "effects": [{"Draw": {}}]}})
)]
#[case::enters_trigger(
    "Name:Test Seer\nTypes:Creature Elf\nT:Mode$ ChangesZone | Origin$ Any | Destination$ Battlefield | ValidCard$ Card.Self | Execute$ TrigDraw\nSVar:TrigDraw:DB$ Draw | NumCards$ 1\n",
    json!({"Triggered": {"when": {"ZoneChange": {"what": ["This"], "to": "Battlefield"}}, "effects": [{"Draw": {}}]}})
)]
#[case::may_trigger(
    "Name:Test Scout\nTypes:Creature Elf\nT:Mode$ Phase | Phase$ Upkeep | ValidPlayer$ You | OptionalDecider$ You | Execute$ TrigGain\nSVar:TrigGain:DB$ GainLife | LifeAmount$ 1\n",
    json!({"Triggered": {"when": {"StepBegins": {"step": "Upkeep", "whose": "You"}},
        "effects": [{"May": {"effects": [{"GainLife": {"amount": 1}}]}}]}})
)]
#[case::attack_trigger(
    "Name:Test Raider\nTypes:Creature Goblin\nT:Mode$ Attacks | ValidCard$ Card.Self | Execute$ TrigPump\nSVar:TrigPump:DB$ Pump | Defined$ Self | NumAtt$ +1 | NumDef$ +0\n",
    json!({"Triggered": {"when": {"Attacks": {"what": ["This"]}}, "effects": [{"Continuous": {"affected": "This",
        "mods": [{"ModifyPT": {"power": 1, "toughness": 0}}], "duration": "UntilEndOfTurn"}}]}})
)]
#[case::anthem(
    "Name:Test Banner\nTypes:Enchantment\nS:Mode$ Continuous | Affected$ Creature.YouCtrl+Other | AddPower$ 1 | AddToughness$ 1\n",
    json!({"Static": {"affected": {"Each": [{"Type": "Creature"}, {"ControlledBy": "You"}, "Other"]},
        "mods": [{"ModifyPT": {"power": 1, "toughness": 1}}]}})
)]
#[case::aura_grant(
    "Name:Test Wings\nTypes:Enchantment Aura\nS:Mode$ Continuous | AffectedDefined$ Enchanted | AddKeyword$ Flying\n",
    json!({"Static": {"affected": "Attached", "mods": [{"AddKeyword": "Flying"}]}})
)]
#[case::enters_tapped(
    "Name:Test Gate\nTypes:Land\nR:Event$ Moved | ValidCard$ Card.Self | Destination$ Battlefield | ReplacementResult$ Updated | ReplaceWith$ ETBTapped\nSVar:ETBTapped:DB$ Tap | Defined$ Self | ETB$ True\n",
    json!({"Replacement": {"when": {"Enters": {"what": ["This"]}}, "by": "Tapped"}})
)]
#[case::keywords_with_costs(
    "Name:Test Blade\nTypes:Artifact Equipment\nK:Equip:3\n",
    json!({"Keyword": {"Equip": "{3}"}})
)]
#[case::search(
    "Name:Test Fetch\nTypes:Sorcery\nA:SP$ ChangeZone | Origin$ Library | Destination$ Battlefield | ChangeType$ Land.Basic | Tapped$ True | ChangeNum$ 1\n",
    json!({"Spell": {"effects": [
        {"Search": {"filter": [{"Type": "Land"}, {"Supertype": "Basic"}], "as": "found"}},
        {"Move": {"what": {"Bound": "found"}, "to": {"Battlefield": {"tapped": true}}}},
        {"Shuffle": {}}]}})
)]
#[case::bounce(
    "Name:Test Gust\nTypes:Instant\nA:SP$ ChangeZone | ValidTgts$ Creature | Origin$ Battlefield | Destination$ Hand\n",
    json!({"Spell": {"targets": [{"what": {"Object": [{"Type": "Creature"}]}}],
        "effects": [{"Move": {"what": {"Target": 0}, "to": "Hand"}}]}})
)]
#[case::modal(
    "Name:Test Charm\nTypes:Instant\nA:SP$ Charm | Choices$ DBDraw,DBBolt\nSVar:DBDraw:DB$ Draw | NumCards$ 2\nSVar:DBBolt:DB$ DealDamage | ValidTgts$ Any | NumDmg$ 2\n",
    json!({"Spell": {"effects": [{"Modal": {"modes": [
        {"effects": [{"Draw": {"count": 2}}]},
        {"targets": [{"what": "AnyTarget"}], "effects": [{"DealDamage": {"to": {"Target": 0}, "amount": 2}}]}]}}]}})
)]
#[case::animate(
    "Name:Test Den\nTypes:Land\nA:AB$ Animate | Cost$ 2 R | Defined$ Self | Power$ 3 | Toughness$ 3 | Types$ Creature,Goblin | RemoveCreatureTypes$ True | Colors$ Red | OverwriteColors$ True | Keywords$ Haste\n",
    json!({"Activated": {"cost": "{2}{R}", "effects": [{"Continuous": {"affected": "This",
        "mods": [{"SetPT": {"power": 3, "toughness": 3}}, {"SetColors": ["Red"]},
            {"SetCreatureTypes": ["Goblin"]}, {"AddType": "Creature"}, {"AddKeyword": "Haste"}],
        "duration": "UntilEndOfTurn"}}]}})
)]
#[case::enters_with_counters(
    "Name:Test Hatchling\nTypes:Creature Elf\nK:etbCounter:P1P1:2\n",
    json!({"Replacement": {"when": {"Enters": {"what": ["This"]}}, "by": {"WithCounters": {"counter": "+1/+1", "count": 2}}}})
)]
#[case::enters_choosing(
    "Name:Test Totem\nTypes:Artifact\nK:ETBReplacement:Other:ChooseColor\nSVar:ChooseColor:DB$ ChooseColor | Defined$ You | AILogic$ Most\n",
    json!({"Replacement": {"when": {"Enters": {"what": ["This"]}}, "by": {"Choose": "Color"}}})
)]
#[case::condition_on_a_static(
    "Name:Test Knight\nTypes:Creature Elf\nS:Mode$ Continuous | Affected$ Card.Self | AddKeyword$ Flying | IsPresent$ Wizard.YouCtrl | PresentCompare$ GE2\n",
    json!({"Static": {"affected": "This", "mods": [{"AddKeyword": "Flying"}],
        "condition": {"Compare": {"left": {"Count": [{"Subtype": "Wizard"}, {"ControlledBy": "You"}]}, "op": "Ge", "right": 2}}}})
)]
#[case::cost_reduction(
    "Name:Test Lantern\nTypes:Artifact\nS:Mode$ ReduceCost | ValidCard$ Creature | Type$ Spell | Activator$ You | Amount$ 1\n",
    json!({"CostModifier": {"applies_to": ["Spell", {"Type": "Creature"}, {"ControlledBy": "You"}],
        "change": "Reduce", "mana": "{1}"}})
)]
#[case::cannot_block(
    "Name:Test Wall\nTypes:Creature Elf\nS:Mode$ CantBlock | ValidCard$ Card.Self\n",
    json!({"Static": {"affected": "This", "mods": ["CantBlock"]}})
)]
#[case::pay_on_resolution(
    "Name:Test Prodigy\nTypes:Creature Elf\nT:Mode$ Attacks | ValidCard$ Card.Self | Execute$ TrigDraw\nSVar:TrigDraw:AB$ Draw | Cost$ 1 U | NumCards$ 1\n",
    json!({"Triggered": {"when": {"Attacks": {"what": ["This"]}},
        "effects": [{"Pay": {"cost": "{1}{U}", "paid": [{"Draw": {}}]}}]}})
)]
#[case::the_controller_of_a_dying_card_is_you(
    "Name:Test Martyr\nTypes:Creature Elf\nT:Mode$ ChangesZone | Origin$ Battlefield | Destination$ Graveyard | ValidCard$ Card.Self | Execute$ TrigGain\nSVar:TrigGain:DB$ GainLife | Defined$ TriggeredCardController | LifeAmount$ 3\n",
    json!({"Triggered": {"when": {"ZoneChange": {"what": ["This"], "from": "Battlefield", "to": "Graveyard"}},
        "effects": [{"GainLife": {"amount": 3}}]}})
)]
#[case::two_events_one_trigger(
    "Name:Test Pup\nTypes:Creature Dog\nT:Mode$ ChangesZone | Origin$ Any | Destination$ Battlefield | ValidCard$ Card.Self | Execute$ TrigDraw\nT:Mode$ Attacks | ValidCard$ Card.Self | Execute$ TrigDraw | Secondary$ True\nSVar:TrigDraw:DB$ Draw\n",
    json!({"Triggered": {"when": {"AnyOf": [
        {"ZoneChange": {"what": ["This"], "to": "Battlefield"}}, {"Attacks": {"what": ["This"]}}]},
        "effects": [{"Draw": {}}]}})
)]
#[case::either_of_two_kinds(
    "Name:Test Snap\nTypes:Instant\nA:SP$ Destroy | ValidTgts$ Creature.attacking,Creature.blocking\n",
    json!({"Spell": {"targets": [{"what": {"Object": [{"Type": "Creature"}, {"AnyOf": ["Attacking", "Blocking"]}]}}],
        "effects": [{"Destroy": {"what": {"Target": 0}}}]}})
)]
#[case::cannot_be_blocked_this_turn(
    "Name:Test Cloak\nTypes:Instant\nA:SP$ Effect | ValidTgts$ Creature | RememberObjects$ Targeted | ExileOnMoved$ Battlefield | StaticAbilities$ Unblockable\nSVar:Unblockable:Mode$ CantBlockBy | ValidAttacker$ Card.IsRemembered | Description$ This creature can't be blocked this turn.\n",
    json!({"Spell": {"targets": [{"what": {"Object": [{"Type": "Creature"}]}}],
        "effects": [{"Continuous": {"affected": {"Target": 0}, "mods": ["CantBeBlocked"], "duration": "UntilEndOfTurn"}}]}})
)]
#[case::soft_counter(
    "Name:Test Hush\nManaCost:1 U\nTypes:Instant\nA:SP$ Counter | Cost$ 1 U | TargetType$ Spell | ValidTgts$ Card | UnlessCost$ 3\n",
    json!({"Spell": {"targets": [{"what": {"Object": ["Spell"]}}],
        "effects": [{"Pay": {"who": {"ControllerOf": {"Target": 0}}, "cost": "{3}",
            "unpaid": [{"Counter": {"what": {"Target": 0}}}]}}]}})
)]
#[case::doesnt_untap(
    "Name:Test Frost\nTypes:Enchantment Aura\nR:Event$ Untap | ActiveZones$ Battlefield | ValidCard$ Creature.EnchantedBy | ValidStepTurnToController$ You | Layer$ CantHappen | Description$ x\n",
    json!({"Static": {"affected": "Attached", "mods": ["DoesntUntap"]}})
)]
#[case::exile_and_return_what_was_remembered(
    "Name:Test Phase\nTypes:Instant\nA:SP$ ChangeZone | ValidTgts$ Creature | Origin$ Battlefield | Destination$ Exile | RememberChanged$ True | SubAbility$ Back\nSVar:Back:DB$ Pump | Defined$ Remembered | NumAtt$ +1 | NumDef$ +1\n",
    json!({"Spell": {"targets": [{"what": {"Object": [{"Type": "Creature"}]}}],
        "effects": [{"Exile": {"what": {"Target": 0}, "as": "exiled"}},
            {"Continuous": {"affected": {"Bound": "exiled"},
                "mods": [{"ModifyPT": {"power": 1, "toughness": 1}}], "duration": "UntilEndOfTurn"}}]}})
)]
#[case::shock_land(
    "Name:Test Fountain\nTypes:Land\nR:Event$ Moved | ValidCard$ Card.Self | Destination$ Battlefield | ReplaceWith$ DBTap | ReplacementResult$ Updated\nSVar:DBTap:DB$ Tap | Defined$ Self | ETB$ True | UnlessCost$ PayLife<2> | UnlessPayer$ You\n",
    json!({"Replacement": {"when": {"Enters": {"what": ["This"]}}, "by": {"PayLifeOrTapped": 2}}})
)]
#[case::check_land(
    "Name:Test Fortress\nTypes:Land\nR:Event$ Moved | ValidCard$ Card.Self | Destination$ Battlefield | ReplaceWith$ LandTapped | ReplacementResult$ Updated\nSVar:LandTapped:DB$ Tap | Defined$ Self | ETB$ True | ConditionPresent$ Plains.YouCtrl,Island.YouCtrl | ConditionCompare$ EQ0\n",
    json!({"Replacement": {"when": {"Enters": {"what": ["This"]}}, "by": "Tapped",
        "condition": {"Not": {"Compare": {"left": {"Count": [{"AnyOf": [{"Subtype": "Plains"}, {"Subtype": "Island"}]},
            {"ControlledBy": "You"}]}, "op": "Ge", "right": 1}}}}})
)]
#[case::prevention(
    "Name:Test Salve\nTypes:Instant\nA:SP$ PreventDamage | ValidTgts$ Any | Amount$ 3\n",
    json!({"Spell": {"targets": [{"what": "AnyTarget"}],
        "effects": [{"Shield": {"when": {"Damage": {"to": {"Target": 0}}}, "by": {"PreventUpTo": 3},
            "duration": "UntilEndOfTurn"}}]}})
)]
#[case::secondary_static_joins_the_first(
    "Name:Test Chains\nTypes:Enchantment Aura\nS:Mode$ Continuous | Affected$ Creature.EnchantedBy | AddPower$ 2 | AddToughness$ 2\nS:Mode$ CantBlock | ValidCard$ Creature.EnchantedBy | Secondary$ True\n",
    json!({"Static": {"affected": "Attached", "mods": [{"ModifyPT": {"power": 2, "toughness": 2}}, "CantBlock"]}})
)]
#[case::a_condition_on_a_part(
    "Name:Test Boon\nTypes:Sorcery\nA:SP$ Draw | SubAbility$ Gain\nSVar:Gain:DB$ GainLife | LifeAmount$ 2 | ConditionPresent$ Artifact.YouCtrl | ConditionCompare$ GE1\n",
    json!({"Spell": {"effects": [{"Draw": {}}, {"If": {"condition": {"Compare": {"left": {"Count":
        [{"Type": "Artifact"}, {"ControlledBy": "You"}]}, "op": "Ge", "right": 1}},
        "then": [{"GainLife": {"amount": 2}}]}}]}})
)]
#[case::a_creature_deals_damage_equal_to_its_power(
    "Name:Test Bite\nTypes:Sorcery\nA:SP$ Pump | ValidTgts$ Creature.YouCtrl | SubAbility$ Hit\nSVar:Hit:DB$ DealDamage | ValidTgts$ Creature.YouDontCtrl | NumDmg$ X | DamageSource$ ParentTarget\nSVar:X:ParentTargeted$CardPower\n",
    json!({"Spell": {"targets": [
        {"what": {"Object": [{"Type": "Creature"}, {"ControlledBy": "You"}]}},
        {"what": {"Object": [{"Type": "Creature"}, {"Not": {"ControlledBy": "You"}}]}}],
        "effects": [{"DealDamage": {"source": {"Target": 0}, "to": {"Target": 1},
            "amount": {"Stat": {"of": {"Target": 0}, "stat": "Power"}}}}]}})
)]
#[case::two_creatures_fight(
    "Name:Test Duel\nTypes:Sorcery\nA:SP$ Pump | ValidTgts$ Creature.YouCtrl | SubAbility$ Fight\nSVar:Fight:DB$ Fight | Defined$ ParentTarget | ValidTgts$ Creature.YouDontCtrl\n",
    json!({"Spell": {"targets": [
        {"what": {"Object": [{"Type": "Creature"}, {"ControlledBy": "You"}]}},
        {"what": {"Object": [{"Type": "Creature"}, {"Not": {"ControlledBy": "You"}}]}}],
        "effects": [{"Fight": {"what": {"Target": 0}, "with": {"Target": 1}}}]}})
)]
#[case::you_attack(
    "Name:Test Banner\nTypes:Enchantment\nT:Mode$ AttackersDeclared | AttackingPlayer$ You | ValidAttackers$ Creature | ValidAttackersAmount$ GE2 | Execute$ TrigDraw\nSVar:TrigDraw:DB$ Draw\n",
    json!({"Triggered": {"when": {"GroupAttacks": {"what": [{"Type": "Creature"}, {"ControlledBy": "You"}],
        "at_least": 2}}, "effects": [{"Draw": {}}]}})
)]
#[case::you_cycle_a_card(
    "Name:Test Drain\nTypes:Enchantment\nT:Mode$ Cycled | ValidCard$ Card.YouOwn | Execute$ TrigDraw\nSVar:TrigDraw:DB$ Draw\n",
    json!({"Triggered": {"when": {"Cycled": {"by": "You"}}, "effects": [{"Draw": {}}]}})
)]
#[case::a_token_copy(
    "Name:Test Mirror\nTypes:Sorcery\nA:SP$ CopyPermanent | ValidTgts$ Creature.YouCtrl | NumCopies$ 2\n",
    json!({"Spell": {"targets": [{"what": {"Object": [{"Type": "Creature"}, {"ControlledBy": "You"}]}}],
        "effects": [{"CreateToken": {"token": {"CopyOf": {"of": {"Target": 0}}}, "count": 2}}]}})
)]
#[case::look_and_take_one(
    "Name:Test Glimpse\nTypes:Sorcery\nA:SP$ Dig | DigNum$ 3 | ChangeNum$ 1\n",
    json!({"Spell": {"effects": [
        {"LookAtTop": {"count": 3, "as": "top"}},
        {"Choose": {"from": {"Bound": "top"}, "as": "kept2", "rest_as": "rest3"}},
        {"Move": {"what": {"Bound": "kept2"}, "to": "Hand"}},
        {"Move": {"what": {"Bound": "rest3"}, "to": {"Library": {"place": "Bottom", "order": "Chosen"}}}}]}})
)]
#[case::search_then_put_on_top(
    "Name:Test Whisper\nTypes:Sorcery\nA:SP$ ChangeZone | Origin$ Library | Destination$ Library | LibraryPosition$ 0 | ChangeType$ Card | ChangeNum$ 1\n",
    json!({"Spell": {"effects": [
        {"Search": {"filter": [], "as": "found"}},
        {"Shuffle": {}},
        {"Move": {"what": {"Bound": "found"}, "to": {"Library": {"place": "Top"}}}}]}})
)]
fn a_line_becomes_the_ability_of_the_dsl(#[case] script: &str, #[case] expected: Value) {
    let mut found = abilities(script);
    assert_eq!(found.len(), 1, "{:?}", gaps(script));
    let found = found.remove(0);
    assert_eq!(normal(found), normal(expected));
}

/// The value as the canonical form of an ability, so that spellings of the same ability compare equal.
fn normal(value: Value) -> Value {
    let ability: mtg_cards::ability::Ability = serde_json::from_value(value).unwrap();
    serde_json::to_value(ability).unwrap()
}

#[rstest]
#[case::unknown_effect(
    "Name:T\nTypes:Sorcery\nA:SP$ RollDice | Sides$ 6\n",
    "effect:RollDice"
)]
#[case::unread_key(
    "Name:T\nTypes:Sorcery\nA:SP$ Draw | NumCards$ 1 | Foo$ Bar\n",
    "key:Draw.Foo"
)]
#[case::unknown_trigger(
    "Name:T\nTypes:Creature Elf\nT:Mode$ Exploited | Execute$ X\nSVar:X:DB$ Draw\n",
    "trigger:Exploited"
)]
#[case::unknown_property(
    "Name:T\nTypes:Sorcery\nA:SP$ Destroy | ValidTgts$ Creature.withQuux\n",
    "valid:withQuux"
)]
#[case::unknown_keyword("Name:T\nTypes:Creature Elf\nK:Quuxing\n", "keyword:Quuxing")]
#[case::untargeted_object("Name:T\nTypes:Sorcery\nA:SP$ Destroy\n", "defined:none-for-Destroy")]
#[case::unknown_amount(
    "Name:T\nTypes:Sorcery\nA:SP$ Draw | NumCards$ Y\nSVar:Y:Count$Weird\n",
    "amount:Count$Weird"
)]
fn a_construct_without_a_mapper_leaves_its_line_out_and_is_named(
    #[case] script: &str,
    #[case] gap: &str,
) {
    assert_eq!(abilities(script), Vec::<Value>::new());
    assert_eq!(gaps(script), vec![gap.to_owned()]);
}

#[test]
fn a_cost_beyond_the_mana_cost_of_a_spell_is_an_additional_cost() {
    let found = abilities(
        "Name:Test Toss\nManaCost:1 R\nTypes:Instant\nA:SP$ DealDamage | Cost$ 1 R Sac<1/Creature> | ValidTgts$ Any | NumDmg$ 2\n",
    );
    assert_eq!(
        found,
        vec![
            json!({"AdditionalCost": {"cost": [{"Sacrifice": {"filter": [{"Type": "Creature"}]}}], "required": true}}),
            json!({"Spell": {"targets": [{"what": "AnyTarget"}],
                "effects": [{"DealDamage": {"to": {"Target": 0}, "amount": 2}}]}}),
        ]
    );
    // Another mana amount than the card's is an alternative cost, which is not an additional one.
    let other = "Name:Test Toss\nManaCost:1 R\nTypes:Instant\nA:SP$ Draw | Cost$ 0\n";
    assert_eq!(gaps(other), vec!["cost:spell-different-mana".to_owned()]);
}

#[test]
fn a_token_with_an_ability_of_its_own_carries_it() {
    let mut world = world();
    let face =
        Script::parse("Name:Gem Token\nTypes:Artifact Gem\nA:AB$ Draw | Cost$ T Sac<1/CARDNAME>\n")
            .faces
            .into_iter()
            .next()
            .unwrap();
    world.tokens.insert("c_a_gem_draw".to_owned(), face);
    let script = Script::parse("Name:T\nTypes:Sorcery\nA:SP$ Token | TokenScript$ c_a_gem_draw\n");
    let found = translate_script(&world, &script).faces.remove(0).abilities;
    assert_eq!(
        found,
        vec![
            json!({"Spell": {"effects": [{"CreateToken": {"token": {"Custom": {
            "name": "Gem", "types": ["Artifact"], "subtypes": ["Gem"],
            "abilities": [{"Activated": {"cost": ["Tap", "SacrificeSelf"], "effects": [{"Draw": {}}]}}]}}}}]}})
        ]
    );
}

#[test]
fn a_saga_has_a_triggered_ability_for_each_chapter() {
    let found = abilities(
        "Name:Test Saga\nTypes:Enchantment Saga\nK:Chapter:2:DBOne,DBTwo\nSVar:DBOne:DB$ Draw\nSVar:DBTwo:DB$ GainLife | LifeAmount$ 2\n",
    );
    assert_eq!(
        found,
        vec![
            json!({"Triggered": {"when": {"Chapter": {"number": 1}}, "effects": [{"Draw": {}}]}}),
            json!({"Triggered": {"when": {"Chapter": {"number": 2}}, "effects": [{"GainLife": {"amount": 2}}]}}),
        ]
    );
}

#[test]
fn only_the_lines_that_map_are_written_and_the_rest_are_gaps() {
    let card = translate(
        "Name:T\nTypes:Creature Elf\nK:Flying\nA:AB$ RollDice | Cost$ T | Sides$ 6\nK:Trample\n",
    );
    assert_eq!((card.lines(), card.translated()), (3, 2));
    assert_eq!(card.gaps().count(), 1);
}

#[test]
fn a_card_with_several_faces_is_named_as_scryfall_names_it_and_has_a_set_of_abilities_for_each() {
    let card = translate(
        "Name:Front\nTypes:Creature Elf\nK:Flying\nALTERNATE\nName:Back\nTypes:Creature Goblin\nK:Haste\n",
    );
    assert_eq!(card.name, "Front // Back");
    assert_eq!(
        card.proposal(),
        json!({"faces": [
            {"abilities": [{"Keyword": "Flying"}]},
            {"abilities": [{"Keyword": "Haste"}]}]})
    );
}

#[test]
fn an_auras_attach_line_is_the_enchant_keyword_and_adds_no_gap() {
    let card = translate(
        "Name:T\nTypes:Enchantment Aura\nK:Enchant:Creature\nA:SP$ Attach | Cost$ 1 W | ValidTgts$ Creature | AILogic$ Pump\n",
    );
    assert_eq!(card.gaps().count(), 0);
    assert_eq!(card.translated(), 1);
}

#[rstest]
#[case("Flying", json!("Flying"))]
#[case("First Strike", json!("FirstStrike"))]
#[case("Ward:2", json!({"Ward": "{2}"}))]
#[case("Cycling:1 U", json!({"Cycling": "{1}{U}"}))]
#[case("Crew:3", json!({"Crew": 3}))]
#[case("Protection:Card.Red:Protection from red", json!({"Protection": [{"Color": "Red"}]}))]
#[case("Enchant:Creature", json!({"Enchant": [{"Type": "Creature"}]}))]
#[case("Suspend:4:R", json!({"Suspend": {"time": 4, "cost": "{R}"}}))]
#[case("Swampwalk", json!({"Landwalk": [{"Subtype": "Swamp"}]}))]
fn keywords_map_to_the_keyword_set(#[case] line: &str, #[case] expected: Value) {
    let script = format!("Name:T\nTypes:Creature Elf\nK:{line}\n");
    assert_eq!(abilities(&script), vec![json!({ "Keyword": expected })]);
}
