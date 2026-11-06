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
            "c_1_1_a_thopter_flying",
            "Name:Thopter Token\nColors:colorless\nTypes:Artifact Creature Thopter\nPT:1/1\nK:Flying\n",
        ),
        (
            "g_x_x_treefolk_forests",
            "Name:Treefolk Token\nColors:green\nTypes:Creature Treefolk\nPT:*/*\nS:Mode$ Continuous | CharacteristicDefining$ True | SetPower$ X | SetToughness$ X\nSVar:X:Count$YourLifeTotal\n",
        ),
        (
            "w_x_x_spirit",
            "Name:Spirit Token\nColors:white\nTypes:Creature Spirit\nPT:*/*\n",
        ),
        (
            "b_x_x_demon_flying",
            "Name:Demon Token\nColors:black\nTypes:Creature Demon\nPT:*/*\nK:Flying\n",
        ),
        (
            "ub_x_x_zombie_menace",
            "Name:Zombie Token\nColors:blue,black\nTypes:Creature Zombie\nPT:*/*\nK:Menace\n",
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
    world.stored = Some(
        ["lightning_bolt", "ponder", "mox_opal"]
            .map(str::to_owned)
            .into(),
    );
    world.subtypes.extend(
        [
            "Elf",
            "Goblin",
            "Wizard",
            "Aura",
            "Equipment",
            "Plains",
            "Island",
            "Assassin",
            "Mercenary",
            "Pirate",
            "Rogue",
            "Warlock",
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
#[case::pt_counters(
    "Name:Test Bloom\nTypes:Sorcery\nA:SP$ PutCounter | ValidTgts$ Creature | CounterType$ P2P0 | CounterNum$ 2\n",
    json!({"Spell": {"targets": [{"what": {"Object": [{"Type": "Creature"}]}}],
        "effects": [{"PutCounters": {"on": {"Target": 0}, "counter": {"Other": "+2/+0"}, "count": 2}}]}})
)]
#[case::remove_all_counters(
    "Name:Test Purge\nTypes:Sorcery\nA:SP$ RemoveCounter | ValidTgts$ Creature | CounterType$ ALL\n",
    json!({"Spell": {"targets": [{"what": {"Object": [{"Type": "Creature"}]}}],
        "effects": [{"RemoveAllCounters": {"from": {"Target": 0}}}]}})
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
#[case::batch_damage(
    "Name:Test Pack\nTypes:Creature Wizard\nT:Mode$ DamageDoneOnce | ValidSource$ Creature.YouCtrl | ValidTarget$ Player | CombatDamage$ True | Execute$ Draw1\nSVar:Draw1:DB$ Draw | NumCards$ 1\n",
    json!({"Triggered": {"when": {"Batch": {"DealsDamage": {"source": [{"Type": "Creature"}, {"ControlledBy": "You"}],
        "to": "EachPlayer", "combat": true}}}, "effects": [{"Draw": {}}]}})
)]
#[case::may_play_from_graveyard(
    "Name:Test Grave\nTypes:Enchantment\nS:Mode$ Continuous | MayPlay$ True | Affected$ Creature.YouOwn | AffectedZone$ Graveyard | MayPlayLimit$ 1 | Description$ You may cast creature spells from your graveyard.\n",
    json!({"Rule": {"rule": {"MayCast": {"what": [{"Type": "Creature"}], "from": "Graveyard", "once": true}}}})
)]
#[case::impulse_draw(
    "Name:Test Impulse\nTypes:Sorcery\nA:SP$ Dig | DigNum$ 2 | ChangeNum$ All | DestinationZone$ Exile | RememberChanged$ True | SubAbility$ Play\nSVar:Play:DB$ Effect | StaticAbilities$ MP | RememberObjects$ Remembered | Duration$ UntilTheEndOfYourNextTurn\nSVar:MP:Mode$ Continuous | Affected$ Card.IsRemembered | AffectedZone$ Exile | MayPlay$ True | Description$ You may play those cards.\n",
    json!({"Spell": {"effects": [{"ExileTop": {"count": 2, "as": "exiled"}},
        {"Rule": {"rule": {"MayCast": {"from": "Exile", "only": {"Bound": "exiled"}, "play": true}},
            "duration": "UntilEndOfYourNextTurn"}}]}})
)]
#[case::nth_spell(
    "Name:Test Echo\nTypes:Creature Wizard\nT:Mode$ SpellCast | ValidActivatingPlayer$ You | ActivatorThisTurnCast$ EQ2 | Execute$ Draw1\nSVar:Draw1:DB$ Draw | NumCards$ 1\n",
    json!({"Triggered": {"when": {"Nth": {"event": {"SpellCast": {"by": "You"}}, "nth": 2}}, "effects": [{"Draw": {}}]}})
)]
#[case::dig_until(
    "Name:Test Delve\nTypes:Sorcery\nA:SP$ DigUntil | Valid$ Creature | FoundDestination$ Hand | RevealedDestination$ Library | RevealedLibraryPosition$ -1 | RevealRandomOrder$ True\n",
    json!({"Spell": {"effects": [
        {"RevealUntil": {"filter": [{"Type": "Creature"}], "as": "found", "revealed_as": "revealed2"}},
        {"Move": {"what": {"Bound": "found"}, "to": "Hand"}},
        {"Move": {"what": {"Bound": "revealed2"}, "to": {"Library": {"place": "Bottom", "order": "Random"}}}}]}})
)]
#[case::kicked_counters(
    "Name:Test Drake\nTypes:Creature Wizard\nK:etbCounter:P1P1:2:CheckSVar$ WasKicked:If CARDNAME was kicked, it enters with two +1/+1 counters on it.\n",
    json!({"Replacement": {"when": {"Enters": {"what": ["This"]}},
        "by": {"WithCounters": {"counter": "+1/+1", "count": 2}}, "condition": "Kicked"}})
)]
#[case::grants_a_trigger(
    "Name:Test Banner\nTypes:Enchantment\nS:Mode$ Continuous | Affected$ Creature.YouCtrl | AddTrigger$ Spark | Description$ x\nSVar:Spark:Mode$ Attacks | ValidCard$ Card.Self | Execute$ Draw1\nSVar:Draw1:DB$ Draw | NumCards$ 1\n",
    json!({"Static": {"affected": {"Each": [{"Type": "Creature"}, {"ControlledBy": "You"}]},
        "mods": [{"GrantAbility": {"Triggered": {"when": {"Attacks": {"what": ["This"]}}, "effects": [{"Draw": {}}]}}}]}})
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
#[case::a_condition_on_a_target(
    "Name:Test Boon2\nTypes:Sorcery\nA:SP$ Tap | ValidTgts$ Creature | SubAbility$ Gain\nSVar:Gain:DB$ GainLife | LifeAmount$ 2 | ConditionDefined$ Targeted | ConditionPresent$ Creature.YouCtrl | ConditionCompare$ EQ1\n",
    json!({"Spell": {"targets": [{"what": {"Object": [{"Type": "Creature"}]}}],
        "effects": [{"Tap": {"what": {"Target": 0}}},
            {"If": {"condition": {"CountOf": {"subject": {"Target": 0}, "selector": [{"Type": "Creature"}, {"ControlledBy": "You"}], "op": "Eq", "right": 1}},
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
#[case::tap_untap_regenerate(
    "Name:Test Rest\nTypes:Creature Elf\nA:AB$ Untap | Cost$ 1 G | Defined$ Self\n",
    json!({"Activated": {"cost": "{1}{G}", "effects": [{"Untap": {"what": "This"}}]}})
)]
#[case::regenerate_itself(
    "Name:Test Shell\nTypes:Creature Elf\nA:AB$ Regenerate | Cost$ G\n",
    json!({"Activated": {"cost": "{G}", "effects": [{"Regenerate": {"what": "This"}}]}})
)]
#[case::tap_all(
    "Name:Test Chill\nTypes:Sorcery\nA:SP$ TapAll | ValidCards$ Creature.OppCtrl\n",
    json!({"Spell": {"effects": [{"Tap": {"what": {"Each": [{"Type": "Creature"}, {"ControlledBy": "Opponents"}]}}}]}})
)]
#[case::mill_scry_surveil(
    "Name:Test Delve\nTypes:Sorcery\nA:SP$ Mill | NumCards$ 3 | Defined$ You | SubAbility$ Look\nSVar:Look:DB$ Scry | ScryNum$ 2 | SubAbility$ Sift\nSVar:Sift:DB$ Surveil | Amount$ 1\n",
    json!({"Spell": {"effects": [{"Mill": {"count": 3}}, {"Scry": {"count": 2}}, {"Surveil": {}}]}})
)]
#[case::random_discard_of_a_target_player(
    "Name:Test Rake\nTypes:Sorcery\nA:SP$ Discard | ValidTgts$ Opponent | NumCards$ 2 | Mode$ Random\n",
    json!({"Spell": {"targets": [{"what": "Opponent"}],
        "effects": [{"Discard": {"who": {"Target": 0}, "count": 2, "random": true}}]}})
)]
#[case::damage_to_all(
    "Name:Test Storm\nTypes:Sorcery\nA:SP$ DamageAll | NumDmg$ 2 | ValidCards$ Creature | ValidPlayers$ Player\n",
    json!({"Spell": {"effects": [
        {"DealDamage": {"to": {"Each": [{"Type": "Creature"}]}, "amount": 2}},
        {"DealDamage": {"to": "EachPlayer", "amount": 2}}]}})
)]
#[case::counters_on_all(
    "Name:Test Rally\nTypes:Sorcery\nA:SP$ PutCounterAll | ValidCards$ Creature.YouCtrl | CounterType$ P1P1 | CounterNum$ 1\n",
    json!({"Spell": {"effects": [{"PutCounters": {"on": {"Each": [{"Type": "Creature"}, {"ControlledBy": "You"}]},
        "counter": "+1/+1"}}]}})
)]
#[case::restricted_mana(
    "Name:Test Forge\nTypes:Artifact\nA:AB$ Mana | Cost$ T | Produced$ R | RestrictValid$ Spell.Creature\n",
    json!({"Activated": {"cost": ["Tap"], "effects": [{"AddMana": {"mana": {"Fixed": ["Red"]},
        "only": [{"Spell": {"types": ["Creature"]}}]}}]}})
)]
#[case::cavern_of_souls_mana(
    "Name:Test Cavern\nTypes:Land\nA:AB$ Mana | Cost$ T | Produced$ Any | RestrictValid$ Spell.Creature+ChosenType | AddsNoCounter$ True\n",
    json!({"Activated": {"cost": ["Tap"], "effects": [{"AddMana": {"mana": {"AnyColor": 1},
        "only": [{"Spell": {"types": ["Creature"], "chosen_type": true}},
                 {"Uncounterable": {"types": ["Creature"], "chosen_type": true}}]}}]}})
)]
#[case::delighted_halfling_mana(
    "Name:Test Halfling\nTypes:Creature Halfling\nA:AB$ Mana | Cost$ T | Produced$ Any | RestrictValid$ Spell.Legendary | AddsNoCounter$ True\n",
    json!({"Activated": {"cost": ["Tap"], "effects": [{"AddMana": {"mana": {"AnyColor": 1},
        "only": [{"Spell": {"supertypes": ["Legendary"]}}, {"Uncounterable": {"supertypes": ["Legendary"]}}]}}]}})
)]
#[case::boseiju_mana(
    "Name:Test Boseiju\nTypes:Land\nA:AB$ Mana | Cost$ T | Produced$ C | AddsNoCounter$ !Permanent\n",
    json!({"Activated": {"cost": ["Tap"], "effects": [{"AddMana": {"mana": {"Fixed": ["Colorless"]},
        "only": [{"Uncounterable": {"types": ["Instant", "Sorcery"]}}]}}]}})
)]
#[case::cannot_be_blocked_by_big_creatures(
    "Name:Test Sneak\nTypes:Creature Elf\nS:Mode$ CantBlockBy | ValidAttacker$ Creature.Self | ValidBlocker$ Creature.powerGE3\n",
    json!({"Static": {"affected": "This", "mods": [{"CantBeBlockedBy": [{"Type": "Creature"},
        {"Compare": {"stat": "Power", "op": "Ge", "to": 3}}]}]}})
)]
#[case::attacks_each_combat(
    "Name:Test Rage\nTypes:Creature Goblin\nS:Mode$ MustAttack | ValidCreature$ Card.Self\n",
    json!({"Rule": {"rule": {"MustAttack": "This"}}})
)]
#[case::spells_cost_more(
    "Name:Test Tax\nTypes:Enchantment\nS:Mode$ RaiseCost | ValidCard$ Card | Type$ Spell | Activator$ Player.Opponent | Amount$ 1\n",
    json!({"CostModifier": {"applies_to": ["Spell", {"ControlledBy": "Opponents"}], "change": "Increase", "mana": "{1}"}})
)]
#[case::an_opponent_sacrifices(
    "Name:Test Edict\nTypes:Sorcery\nA:SP$ Sacrifice | ValidTgts$ Opponent | SacValid$ Creature\n",
    json!({"Spell": {"targets": [{"what": "Opponent"}],
        "effects": [{"Sacrifice": {"who": {"Target": 0}, "filter": [{"Type": "Creature"}]}}]}})
)]
#[case::at_the_next_end_step(
    "Name:Test Lantern\nTypes:Instant\nA:SP$ DelayedTrigger | Mode$ Phase | Phase$ End of Turn | Execute$ Gain\nSVar:Gain:DB$ GainLife | LifeAmount$ 3\n",
    json!({"Spell": {"effects": [{"DelayedTrigger": {"when": {"StepBegins": {"step": "End", "whose": "EachPlayer"}},
        "effects": [{"GainLife": {"amount": 3}}]}}]}})
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
#[case::change_the_target(
    "Name:Test Misdirect\nTypes:Instant\nA:SP$ ChangeTargets | TargetType$ Spell.singleTarget | ValidTgts$ Card | TgtPrompt$ x\n",
    json!({"Spell": {"targets": [{"what": {"Object": ["Spell", "SingleTarget"]}}],
        "effects": [{"ChangeTargets": {"what": {"Target": 0}, "to": "Another"}}]}})
)]
#[case::new_targets(
    "Name:Test Deflect\nTypes:Instant\nA:SP$ ChangeTargets | TargetType$ SpellAbility | Optional$ True | ValidTgts$ Card,Emblem\n",
    json!({"Spell": {"targets": [{"what": {"Object": [{"AnyOf": ["Spell", {"Ability": "Activated"}, {"Ability": "Triggered"}]}]}}],
        "effects": [{"ChangeTargets": {"what": {"Target": 0}, "to": "Choose"}}]}})
)]
#[case::enters_as_a_copy(
    "Name:Test Clone\nTypes:Creature Shapeshifter\nPT:0/0\nK:ETBReplacement:Copy:DBCopy:Optional\nSVar:DBCopy:DB$ Clone | Choices$ Creature.Other | AddTypes$ Illusion\n",
    json!({"Replacement": {"when": {"Enters": {"what": ["This"]}},
        "by": {"AsCopy": {"of": [{"Type": "Creature"}, "Other"], "except": [{"AddSubtype": "Illusion"}]}}}})
)]
#[case::becomes_a_copy_of_a_target(
    "Name:Test Mime\nTypes:Creature Shapeshifter\nPT:1/1\nA:AB$ Clone | Cost$ 1 | ValidTgts$ Creature.Other | Duration$ UntilEndOfTurn | NewName$ Mime | AddTypes$ Legendary | GainThisAbility$ True\n",
    json!({"Activated": {"cost": "{1}", "targets": [{"what": {"Object": [{"Type": "Creature"}, "Other"]}}],
        "effects": [{"BecomeCopy": {"what": "This", "of": {"Target": 0},
            "except": [{"SetName": "Mime"}, {"AddSupertype": "Legendary"}, "ThisAbility"],
            "duration": "UntilEndOfTurn"}}]}})
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
    "Name:T\nTypes:Creature Elf\nT:Mode$ Clashed | Execute$ X\nSVar:X:DB$ Draw\n",
    "trigger:Clashed"
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

#[rstest]
#[case::paid_for_x("Count$xPaid", json!("X"))]
#[case::creatures_you_control("Count$Valid Creature.YouCtrl", json!({"Count": [{"Type": "Creature"}, {"ControlledBy": "You"}]}))]
#[case::one_more("Count$Valid Creature.YouCtrl/Plus.1", json!({"Sum": [{"Count": [{"Type": "Creature"}, {"ControlledBy": "You"}]}, 1]}))]
#[case::twice_the_life("Count$YourLifeTotal/Twice", json!({"Times": {"by": 2, "of": {"Life": "You"}}}))]
#[case::opponents("PlayerCountOpponents$Amount", json!({"Players": "Opponents"}))]
#[case::that_much("TriggerCount$DamageAmount", json!("Triggering"))]
#[case::power_of_the_sacrificed("Sacrificed$CardPower", json!({"Stat": {"of": "Paid", "stat": "Power"}}))]
#[case::number("Number$4", json!(4))]
fn an_expression_of_a_named_part_is_an_amount(#[case] expression: &str, #[case] expected: Value) {
    let script = format!(
        "Name:T\nTypes:Sorcery\nA:SP$ DealDamage | ValidTgts$ Any | NumDmg$ X\nSVar:X:{expression}\n"
    );
    let found = abilities(&script);
    assert_eq!(found.len(), 1, "{:?}", gaps(&script));
    assert_eq!(
        found[0]["Spell"]["effects"][0]["DealDamage"]["amount"],
        expected
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

mod audit;
mod bv1_counts;
mod en2;
mod en3_discard;
mod en3_reveal;
mod en3_source;
mod fg3;
mod fg4a;
mod fg4a_ambient;
mod fg4a_emblem;
mod fg4b;
mod fg4b_names;
mod fg4ba;
mod fg4d_amounts;
mod fg5_amounts;
mod fg5_valid;
mod fg5e_replace;
mod fg5e_triggers;
mod fg5e_verbs;
mod fg5ea1_verbs;
mod fg5ea2;
mod fg5ka;
mod fg5s;
mod fg5s_arrest;
mod fg5s_block;
mod fg5s_hidden;
mod fg5s_play;
mod fg5s_prevent;
mod fg_digital;
mod gn2;
mod gn2_cant;
mod gn2_play;
mod ledger;
mod macro_keywords;
mod r1a;
mod r2c;
mod r3c;
#[rstest]
#[case::if_tapped_on_activation(
    "Name:T\nTypes:Creature Elf\nA:AB$ Draw | Cost$ 0 | IsPresent$ Card.Self+tapped\n",
    "key:AB.IsPresent=Self"
)]
#[case::may_the_print_lacks(
    "Name:T\nTypes:Sorcery\nA:SP$ ChangeZone | Origin$ Library | Destination$ Hand | ChangeType$ Land | Optional$ True | SpellDescription$ Search your library for a land card.\n",
    "key:ChangeZone.Optional"
)]
fn narrower_than_print_shapes_stay_gaps(#[case] script: &str, #[case] gap: &str) {
    assert!(
        gaps(script).iter().any(|found| found == gap),
        "{:?}",
        gaps(script)
    );
}

#[test]
fn the_end_step_without_your_is_every_players() {
    let script = "Name:T\nTypes:Creature Elf\nT:Mode$ Phase | Phase$ End of Turn | ValidPlayer$ You | Execute$ Go | TriggerDescription$ At the beginning of the end step, draw.\nSVar:Go:DB$ Draw\n";
    assert_eq!(
        abilities(script),
        vec![
            json!({"Triggered": {"when": {"StepBegins": {"step": "End"}}, "effects": [{"Draw": {}}]}})
        ]
    );
}
mod af1;
mod allgaps;
mod en4_counter;
mod en4_cumulative;
mod en4_look;
mod en4_static_trigger;
mod ev1;
mod f1;
mod f2;
mod f3;
mod filters;
mod fix2;
mod fix3;
mod frame;
mod p1;
mod p2;
mod p3;
mod p4;
mod p5;
mod p6;
mod p7;
mod p8;
mod p8b;
mod qf1;
mod r1b;
mod r1c;
mod r1d;
mod r1e;
mod r1f;
mod r1fix;
mod r1g;
mod r1h;
mod r2a;
mod r2b;
mod r2d;
mod r2e;
mod r2f;
mod r2g;
mod r2h;
mod r3a;
mod r3b;
mod r3d;
mod r3f;
mod r3g;
mod r3p2;
mod r3p3;
mod r3p4;
mod r3p5;
mod r3p6;
mod r3p7;
mod r3p8;
mod r4b;
mod r4f2;
mod r4p1;
mod r4p2;
mod r4p3;
mod r4p4;
mod r4p5;
mod r5p1;
mod r5p2;
mod r5p3;
mod r5p4;
mod r5p5;
mod r5p6;
mod r5q1;
mod r6p1;
mod r6p2;
mod r6p3;
mod r6p4;
mod r6p5;
mod r6p6;
mod r6q1;
mod r7p1;
mod r7p2;
mod r7p3;
mod r7p4;
mod r7p5;
mod r7p6;
mod r7p7;
mod r7q1;
mod r8p1;
mod r8p2;
mod r8p3;
mod r8p4;
mod r8p5;
mod r8p6;
mod r8q1;
mod r9p1;
mod r9p2;
mod r9p3;
mod shapes1;
mod shapes2;
mod shapes3;
mod slots;
