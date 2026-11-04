//! Goal: piece P7 of the rules plan translates its constructs, one rstest table per rule.
use rstest::rstest;
use serde_json::{Value, json};

use super::{abilities, gaps};
use crate::forge::script::{Params, Script};
use crate::translate::ctx::Ctx;
use crate::translate::triggers::delayed_when;

/// The event pattern of a card whose only ability is the trigger line `trigger` that draws a card.
fn when(trigger: &str) -> Value {
    let text = format!(
        "Name:T\nTypes:Creature Elf\nPT:1/1\nT:{trigger} | Execute$ Go\nSVar:Go:DB$ Draw | NumCards$ 1\n"
    );
    abilities(&text)
        .first()
        .and_then(|ability| ability.pointer("/Triggered/when"))
        .cloned()
        .unwrap_or(Value::Null)
}

fn gap_of(trigger: &str) -> Vec<String> {
    gaps(&format!(
        "Name:T\nTypes:Creature Elf\nPT:1/1\nT:{trigger} | Execute$ Go\nSVar:Go:DB$ Draw | NumCards$ 1\n"
    ))
}

#[rstest]
#[case::enchanted_controller(
    "Mode$ Phase | Phase$ Upkeep | ValidPlayer$ Player.EnchantedController",
    json!({"StepBegins": {"step": "Upkeep", "whose": {"ControllerOf": "Attached"}}})
)]
#[case::enchanted_player(
    "Mode$ Phase | Phase$ Upkeep | ValidPlayer$ Player.EnchantedBy",
    json!({"StepBegins": {"step": "Upkeep", "whose": "AttachedPlayer"}})
)]
#[case::attackers_opponent(
    "Mode$ AttackersDeclared | AttackingPlayer$ Player.Opponent",
    json!({"GroupAttacks": {"what": [{"Type": "Creature"}, {"ControlledBy": "Opponents"}]}})
)]
#[case::attackers_anyone(
    "Mode$ AttackersDeclared | AttackingPlayer$ Player",
    json!({"GroupAttacks": {"what": [{"Type": "Creature"}]}})
)]
#[case::attackers_enchanted(
    "Mode$ AttackersDeclared | AttackingPlayer$ Player.EnchantedBy",
    json!({"GroupAttacks": {"what": [{"Type": "Creature"}, {"ControlledBy": "AttachedPlayer"}]}})
)]
#[case::attacks_you(
    "Mode$ Attacks | ValidCard$ Creature | Attacked$ You",
    json!({"AttacksPlayer": {"what": [{"Type": "Creature"}], "at": "You"}})
)]
#[case::attacks_you_or_walker(
    "Mode$ Attacks | ValidCard$ Creature | Attacked$ You,Planeswalker.YouCtrl",
    json!({"AttacksPlayerOrPlaneswalker": {"what": [{"Type": "Creature"}], "at": "You"}})
)]
#[case::attacks_enchanted(
    "Mode$ Attacks | ValidCard$ Creature | Attacked$ Player.EnchantedBy",
    json!({"AttacksPlayer": {"what": [{"Type": "Creature"}], "at": "AttachedPlayer"}})
)]
#[case::damage_dealt_once(
    "Mode$ DamageDealtOnce | ValidSource$ Card.Self | CombatDamage$ True",
    json!({"Batch": {"DealsDamage": {"source": ["This"], "combat": true}}})
)]
#[case::unlock_this_door(
    "Mode$ UnlockDoor | ValidPlayer$ You | ValidCard$ Card.Self | ThisDoor$ True",
    json!({"DoorUnlocked": {"who": "You", "door": 0}})
)]
#[case::crime("Mode$ CommitCrime | ValidPlayer$ You", json!({"W11cCrime": {"who": "You"}}))]
#[case::crime_opponent(
    "Mode$ CommitCrime | ValidPlayer$ Opponent",
    json!({"W11cCrime": {"who": "Opponents"}})
)]
#[case::dungeon(
    "Mode$ DungeonCompleted | ValidPlayer$ You",
    json!({"CompletedDungeon": {"who": "You"}})
)]
#[case::crewed(
    "Mode$ BecomesCrewed | ValidCard$ Card.Self",
    json!({"BecomesCrewed": {"what": ["This"]}})
)]
#[case::crewed_first(
    "Mode$ BecomesCrewed | ValidCard$ Card.Self | FirstTimeCrewed$ True",
    json!({"Nth": {"event": {"BecomesCrewed": {"what": ["This"]}}, "nth": 1}})
)]
#[case::attached_self(
    "Mode$ Attached | ValidSource$ Card.Self | ValidTarget$ Creature",
    json!({"Attached": {"what": ["This"], "to": [{"Type": "Creature"}]}})
)]
fn event_words_read_as_the_pattern_they_print(#[case] trigger: &str, #[case] expected: Value) {
    assert_eq!(when(trigger), expected);
}

#[rstest]
#[case::attacked_battle(
    "Mode$ Attacks | ValidCard$ Creature | Attacked$ Battle",
    "key:Trigger.Attacks.Attacked=Battle"
)]
#[case::attacked_planeswalker_of_another(
    "Mode$ Attacks | ValidCard$ Creature | Attacked$ You,Planeswalker.OppCtrl",
    "key:Trigger.Attacks.Attacked=You,Planeswalker.OppCtrl"
)]
#[case::attacking_player_unknown(
    "Mode$ AttackersDeclared | AttackingPlayer$ Player.withMostLife",
    "key:Trigger.AttackersDeclared.AttackingPlayer=Player.withMostLife"
)]
#[case::phase_player_unknown(
    "Mode$ Phase | Phase$ Upkeep | ValidPlayer$ You.descended",
    "key:Trigger.Phase.ValidPlayer=You.descended"
)]
#[case::door_without_this_door(
    "Mode$ UnlockDoor | ValidPlayer$ You | ValidCard$ Card.Self",
    "key:Trigger.UnlockDoor.ThisDoor"
)]
#[case::crew_amount(
    "Mode$ BecomesCrewed | ValidCard$ Card.Self | ValidCrewAmount$ 2",
    "key:Trigger.BecomesCrewed.ValidCrewAmount"
)]
#[case::attached_static(
    "Mode$ Attached | ValidSource$ Card.Self | Static$ True",
    "key:Trigger.Attached.Static"
)]
fn a_word_the_table_does_not_know_stays_a_named_gap(#[case] trigger: &str, #[case] name: &str) {
    assert_eq!(gap_of(trigger), [name]);
}

/// The card text of an enchantment holding the replacement line `line` and the given `SVar` lines.
fn replacement(line: &str, svars: &str) -> String {
    format!("Name:T\nTypes:Enchantment\nR:{line}\n{svars}")
}

#[rstest]
#[case::double_life(
    "Event$ GainLife | ActiveZones$ Battlefield | ValidPlayer$ You | ReplaceWith$ Twice | Description$ x",
    "SVar:Twice:DB$ ReplaceEffect | VarName$ LifeGained | VarValue$ X\nSVar:X:ReplaceCount$LifeGained/Twice\n",
    json!({"Replacement": {"when": {"LifeGain": {}}, "by": {"Multiply": 2}}})
)]
#[case::one_more_life(
    "Event$ GainLife | ActiveZones$ Battlefield | ValidPlayer$ You | ReplaceWith$ More | AILogic$ DoubleLife",
    "SVar:More:DB$ ReplaceEffect | VarName$ LifeGained | VarValue$ X\nSVar:X:ReplaceCount$LifeGained/Plus.1\n",
    json!({"Replacement": {"when": {"LifeGain": {}}, "by": {"Add": 1}}})
)]
#[case::draw_two(
    "Event$ Draw | ActiveZones$ Battlefield | ValidPlayer$ You | NotFirstCardInDrawStep$ True | ReplaceWith$ DrawTwo",
    "SVar:DrawTwo:DB$ Draw | Defined$ You | NumCards$ 2\n",
    json!({"Replacement": {"when": {"Draw": {"except_draw_step": 1}}, "by": {"Multiply": 2}}})
)]
#[case::skip_draws(
    "Event$ Draw | ActiveZones$ Battlefield | ValidPlayer$ Player | Prevent$ True",
    "",
    json!({"Replacement": {"when": {"Draw": {"who": "EachPlayer"}}, "by": "Prevent"}})
)]
#[case::double_p1p1(
    "Event$ AddCounter | ActiveZones$ Battlefield | ValidCard$ Creature.YouCtrl | ValidCounterType$ P1P1 | ReplaceWith$ Double",
    "SVar:Double:DB$ ReplaceCounter | ValidCounterType$ P1P1 | Amount$ X\nSVar:X:ReplaceCount$CounterNum/Twice\n",
    json!({"Replacement": {"when": {"CountersPut": {"on": {"Each": [{"Type": "Creature"}, {"ControlledBy": "You"}]},
        "counter": "+1/+1"}}, "by": {"Multiply": 2}}})
)]
#[case::one_more_energy(
    "Event$ AddCounter | ActiveZones$ Battlefield | ValidPlayer$ You | ValidCounterType$ ENERGY | ReplaceWith$ One",
    "SVar:One:DB$ ReplaceCounter | ValidCounterType$ ENERGY | ChooseCounter$ True | Amount$ X\nSVar:X:ReplaceCount$CounterNum/Plus.1\n",
    json!({"Replacement": {"when": {"CountersPut": {"on": "You", "counter": {"Other": "energy"}}}, "by": {"Add": 1}}})
)]
fn amount_replacements_read_as_the_change_they_print(
    #[case] line: &str,
    #[case] svars: &str,
    #[case] expected: Value,
) {
    let mut found = abilities(&replacement(line, svars));
    assert_eq!(found.len(), 1, "{:?}", gaps(&replacement(line, svars)));
    assert_eq!(found.remove(0), expected);
}

#[rstest]
#[case::life_lost_instead(
    "Event$ GainLife | ActiveZones$ Battlefield | ValidPlayer$ Opponent | ReplaceWith$ Lose",
    "SVar:Lose:DB$ LoseLife | LifeAmount$ X | Defined$ ReplacedPlayer\nSVar:X:ReplaceCount$LifeGained\n",
    "key:Replacement.GainLife.LifeAmount"
)]
#[case::life_halved_is_unknown(
    "Event$ GainLife | ActiveZones$ Battlefield | ValidPlayer$ You | ReplaceWith$ Half",
    "SVar:Half:DB$ ReplaceEffect | VarName$ LifeGained | VarValue$ X\nSVar:X:ReplaceCount$LifeGained/HalfDown\n",
    "amount:ReplaceCount$LifeGained/HalfDown"
)]
#[case::draw_while_hellbent(
    "Event$ Draw | ActiveZones$ Battlefield | ValidPlayer$ You | Hellbent$ True | ReplaceWith$ DrawTwo",
    "SVar:DrawTwo:DB$ Draw | Defined$ You | NumCards$ 2\n",
    "key:Replacement.Draw.Hellbent"
)]
#[case::opponent_draw_doubled_is_not_yours(
    "Event$ Draw | ActiveZones$ Battlefield | ValidPlayer$ Opponent | ReplaceWith$ DrawTwo",
    "SVar:DrawTwo:DB$ Draw | Defined$ You | NumCards$ 2\n",
    "replacement:Draw-instead"
)]
#[case::counters_put_by_a_source(
    "Event$ AddCounter | ActiveZones$ Battlefield | ValidSource$ You | ValidObject$ Permanent,Player | ReplaceWith$ Double",
    "SVar:Double:DB$ ReplaceCounter | Amount$ X\nSVar:X:ReplaceCount$CounterNum/Twice\n",
    "key:Replacement.AddCounter.ValidSource"
)]
#[case::one_more_of_any_kind(
    "Event$ AddCounter | ActiveZones$ Battlefield | ValidCard$ Creature.YouCtrl | ReplaceWith$ More",
    "SVar:More:DB$ ReplaceCounter | ChooseCounter$ True | Amount$ X\nSVar:X:ReplaceCount$CounterNum/Plus.1\n",
    "replacement:AddCounter-plus-without-a-kind"
)]
fn an_amount_replacement_that_is_not_known_stays_a_named_gap(
    #[case] line: &str,
    #[case] svars: &str,
    #[case] name: &str,
) {
    assert_eq!(gaps(&replacement(line, svars)), [name]);
}

#[rstest]
#[case::all_creatures(
    "Event$ Untap | ActiveZones$ Battlefield | ValidCard$ Creature | ValidStepTurnToController$ You | Layer$ CantHappen",
    json!({"Static": {"affected": {"Each": [{"Type": "Creature"}]}, "mods": ["DoesntUntap"]}})
)]
#[case::flyers(
    "Event$ Untap | ActiveZones$ Battlefield | ValidCard$ Creature.withFlying | ValidStepTurnToController$ You | Layer$ CantHappen",
    json!({"Static": {"affected": {"Each": [{"Type": "Creature"}, {"HasKeyword": "Flying"}]}, "mods": ["DoesntUntap"]}})
)]
#[case::enchanted_permanent(
    "Event$ Untap | ActiveZones$ Battlefield | ValidCard$ Permanent.EnchantedBy | ValidStepTurnToController$ You | Layer$ CantHappen",
    json!({"Static": {"affected": "Attached", "mods": ["DoesntUntap"]}})
)]
#[case::equipped_creature(
    "Event$ Untap | ValidCard$ Creature.EquippedBy | ValidStepTurnToController$ You | Layer$ CantHappen",
    json!({"Static": {"affected": "Attached", "mods": ["DoesntUntap"]}})
)]
fn a_permanent_that_does_not_untap_reads_who_it_is(#[case] line: &str, #[case] expected: Value) {
    let mut found = abilities(&replacement(line, ""));
    assert_eq!(found.len(), 1, "{:?}", gaps(&replacement(line, "")));
    assert_eq!(found.remove(0), expected);
}

#[rstest]
#[case::chosen_player_in_the_command_zone(
    "Event$ Untap | ActiveZones$ Command | ValidCard$ Permanent.ChosenCtrl | ValidStepTurnToController$ You | Layer$ CantHappen",
    "replacement:Untap-zone"
)]
#[case::only_with_a_counter_on_itself(
    "Event$ Untap | ActiveZones$ Battlefield | ValidCard$ Card.Self+counters_GE1_DEPLETION | ValidStepTurnToController$ You | Layer$ CantHappen",
    "replacement:Untap-affected"
)]
fn an_untap_rule_that_is_not_known_stays_a_named_gap(#[case] line: &str, #[case] name: &str) {
    assert_eq!(gaps(&replacement(line, "")), [name]);
}

#[rstest]
#[case::second_main(
    "Mode$ Phase | Phase$ Main | PhaseCount$ 2 | ValidPlayer$ You",
    "Main2"
)]
#[case::first_main(
    "Mode$ Phase | Phase$ Main | PhaseCount$ 1 | ValidPlayer$ You",
    "Main1"
)]
fn a_counted_main_phase_is_one_step(#[case] trigger: &str, #[case] step: &str) {
    assert_eq!(
        when(trigger),
        json!({"StepBegins": {"step": step, "whose": "You"}})
    );
}

#[rstest]
#[case::third_main("Mode$ Phase | Phase$ Main | PhaseCount$ 3 | ValidPlayer$ You")]
#[case::any_main("Mode$ Phase | Phase$ Main | ValidPlayer$ You")]
#[case::count_on_another_step("Mode$ Phase | Phase$ Upkeep | PhaseCount$ 2 | ValidPlayer$ You")]
fn a_phase_count_that_is_not_one_or_two_stays_a_gap(#[case] trigger: &str) {
    assert_eq!(gap_of(trigger), ["key:Trigger.Phase.PhaseCount"]);
}

/// The gaps of a permanent whose only ability is the static line `line`.
fn static_gaps(line: &str) -> Vec<String> {
    gaps(&format!("Name:T\nTypes:Enchantment\nS:{line}\n"))
}

#[rstest]
#[case::label_is_text_for_people(
    "Mode$ Continuous | Affected$ Creature.YouOwn+nonLand | MayPlay$ True | MayPlayLimit$ 1 | MayPlayText$ Creature | EffectZone$ Battlefield | AffectedZone$ Graveyard",
    &[]
)]
#[case::pay_zero_is_not_free(
    "Mode$ Continuous | Affected$ Card.nonLand | MayPlay$ True | MayPlayAltManaCost$ 0 | MayPlayDontGrantZonePermissions$ True | AffectedZone$ Exile",
    &["key:Static.MayPlay.MayPlayAltManaCost"]
)]
#[case::any_color_is_not_dropped(
    "Mode$ Continuous | Affected$ Card.nonLand | MayPlay$ True | MayPlayIgnoreColor$ True | AffectedZone$ Exile",
    &["key:Static.MayPlay.MayPlayIgnoreColor"]
)]
#[case::any_type_is_not_dropped(
    "Mode$ Continuous | Affected$ Card.nonLand | MayPlay$ True | MayPlayIgnoreType$ True | AffectedZone$ Exile",
    &["key:Static.MayPlay.MayPlayIgnoreType"]
)]
#[case::the_owner_is_not_you(
    "Mode$ Continuous | Affected$ Card.nonLand | MayPlay$ True | MayPlayPlayer$ CardOwner | AffectedZone$ Exile",
    &["key:Static.MayPlay.MayPlayPlayer=CardOwner"]
)]
fn a_may_play_word_that_changes_the_cast_is_read_or_refused(
    #[case] line: &str,
    #[case] expected: &[&str],
) {
    assert_eq!(static_gaps(line), expected);
}

/// What a `DelayedTrigger` line reads as its event, and whether it waits for this turn only.
fn delayed(line: &str) -> Result<(Value, bool), String> {
    let world = super::world();
    let script = Script::parse("Name:T\nTypes:Creature Elf\nPT:1/1\n");
    let face = script.faces.into_iter().next().expect("one face");
    let ctx = Ctx::new(&face, &world);
    delayed_when(&ctx, &Params::parse(line)).map_err(|gap| gap.0)
}

#[rstest]
#[case::step(
    "Mode$ Phase | Phase$ End of Turn | ValidPlayer$ You",
    Ok((json!({"StepBegins": {"step": "End", "whose": "You"}}), false))
)]
#[case::this_turn(
    "Mode$ SpellCast | ValidActivatingPlayer$ You | ThisTurn$ True",
    Ok((json!({"SpellCast": {"by": "You"}}), true))
)]
#[case::a_dying_creature(
    "Mode$ ChangesZone | ValidCard$ Creature.YouCtrl | Origin$ Battlefield | Destination$ Graveyard",
    Ok((json!({"ZoneChange": {"what": [{"Type": "Creature"}, {"ControlledBy": "You"}],
        "from": "Battlefield", "to": "Graveyard"}}), false))
)]
#[case::next_turn_is_a_step_apart(
    "Mode$ Phase | Phase$ Upkeep | ValidPlayer$ You | NextTurn$ True",
    Err("key:DelayedTrigger.NextTurn".to_owned())
)]
#[case::unknown_mode("Mode$ Nonsense", Err("trigger:Nonsense".to_owned()))]
#[case::unread_key(
    "Mode$ Phase | Phase$ Upkeep | Odd$ True",
    Err("key:DelayedTrigger.Phase.Odd".to_owned())
)]
fn a_delayed_trigger_reads_the_trigger_pattern(
    #[case] line: &str,
    #[case] expected: Result<(Value, bool), String>,
) {
    assert_eq!(delayed(line), expected);
}

/// The first ability of a permanent whose only ability is the static line `line`.
fn static_ability(line: &str) -> Value {
    abilities(&format!("Name:T\nTypes:Enchantment\nS:{line}\n"))
        .first()
        .cloned()
        .unwrap_or(Value::Null)
}

#[rstest]
#[case::must_block_self(
    "Mode$ MustBlock | ValidCreature$ Card.Self",
    json!({"Rule": {"rule": {"MustBlock": "This"}}})
)]
#[case::must_block_all(
    "Mode$ MustBlock | ValidCreature$ Creature",
    json!({"Rule": {"rule": {"MustBlock": {"Each": [{"Type": "Creature"}]}}}})
)]
#[case::must_attack_still(
    "Mode$ MustAttack | ValidCreature$ Card.Self",
    json!({"Rule": {"rule": {"MustAttack": "This"}}})
)]
fn a_must_rule_reads_who_it_binds(#[case] line: &str, #[case] expected: Value) {
    assert_eq!(static_ability(line), expected);
}

#[rstest]
#[case::you(
    "Mode$ CantAttack | ValidCard$ Creature | Target$ You",
    json!({"Static": {"affected": {"Each": [{"Type": "Creature"}]}, "mods": [{"CantAttackYou": "Player"}]}})
)]
#[case::you_and_walkers(
    "Mode$ CantAttack | ValidCard$ Creature.EnchantedBy | Target$ You,Planeswalker.YouCtrl",
    json!({"Static": {"affected": "Attached", "mods": [{"CantAttackYou": "Both"}]}})
)]
#[case::walkers_only(
    "Mode$ CantAttack | ValidCard$ Creature | Target$ Planeswalker.YouCtrl",
    json!({"Static": {"affected": {"Each": [{"Type": "Creature"}]}, "mods": [{"CantAttackYou": "Planeswalkers"}]}})
)]
fn a_creature_that_cant_attack_you_keeps_what_it_protects(
    #[case] line: &str,
    #[case] expected: Value,
) {
    assert_eq!(static_ability(line), expected);
}

#[rstest]
#[case::you_hexproof(
    "Mode$ Continuous | Affected$ You | AddKeyword$ Hexproof",
    json!({"Static": {"affected": "You", "mods": [{"AddKeyword": "Hexproof"}]}})
)]
#[case::each_player_shroud(
    "Mode$ Continuous | Affected$ Player | AddKeyword$ Shroud",
    json!({"Static": {"affected": "EachPlayer", "mods": [{"AddKeyword": "Shroud"}]}})
)]
fn a_player_with_a_keyword_a_player_can_have_is_affected(
    #[case] line: &str,
    #[case] expected: Value,
) {
    assert_eq!(static_ability(line), expected);
}

#[rstest]
#[case::flying(
    "Mode$ Continuous | Affected$ You | AddKeyword$ Flying",
    "static-affected:You"
)]
#[case::power(
    "Mode$ Continuous | Affected$ Player | AddPower$ 1",
    "static-affected:Player"
)]
fn a_player_given_what_a_player_cannot_have_stays_a_named_gap(
    #[case] line: &str,
    #[case] name: &str,
) {
    assert_eq!(static_gaps(line), [name]);
}

#[rstest]
#[case::own_spell_with_a_target(
    "Mode$ ReduceCost | ValidCard$ Card.Self | Type$ Spell | Amount$ 2 | EffectZone$ All | ValidTarget$ Creature.tapped",
    json!({"CostModifier": {"applies_to": ["This", {"Targeting": [{"Type": "Creature"}, "Tapped"]}], "change": "Reduce", "mana": "{2}"}})
)]
#[case::spells_that_target_this(
    "Mode$ RaiseCost | ValidTarget$ Card.Self | Activator$ Opponent | Type$ Spell | Amount$ 2",
    json!({"CostModifier": {"applies_to": ["Spell", {"ControlledBy": "Opponents"}, {"Targeting": ["This"]}], "change": "Increase", "mana": "{2}"}})
)]
fn a_cost_change_for_spells_with_a_target_reads_the_target(
    #[case] line: &str,
    #[case] expected: Value,
) {
    assert_eq!(static_ability(line), expected);
}

#[test]
fn an_attack_target_that_is_not_known_stays_a_named_gap() {
    assert_eq!(
        static_gaps(
            "Mode$ CantAttack | ValidCard$ Card.Self | Target$ Player.attackedBySourceThisTurn"
        ),
        ["key:Static.CantAttack.Target=Player.attackedBySourceThisTurn"]
    );
}
