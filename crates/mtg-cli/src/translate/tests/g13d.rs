//! Goal: combat requirements, restrictions and "can't" statics translate as the rules they print: `Effect` statics named
//! through the remembered player or objects, the player a group of attackers attacks, attachment bans, life totals that
//! can't change, targeting bans by the opponents, limits on the attackers at you and defender conditions.
use rstest::rstest;
use serde_json::{Value, json};

use super::{abilities, gaps};

fn sorcery(line: &str, statics: &str) -> String {
    format!(
        "Name:T\nTypes:Sorcery\nA:SP$ Effect | {line} | StaticAbilities$ St\nSVar:St:{statics}\n"
    )
}

fn effects(script: &str) -> Value {
    abilities(script)[0]["Spell"]["effects"].clone()
}

#[rstest]
#[case::creatures_that_player_controls_attack(
    "ValidTgts$ Player | RememberObjects$ Targeted",
    "Mode$ MustAttack | ValidCreature$ Creature.RememberedPlayerCtrl",
    json!([{"Rule": {"duration": "UntilEndOfTurn", "rule": {"MustAttack": {"Each": [
        {"Type": "Creature"}, {"ControlledBy": {"Target": 0}}]}}}}])
)]
#[case::creatures_that_player_controls_cant_block(
    "ValidTgts$ Player | RememberObjects$ Targeted",
    "Mode$ CantBlock | ValidCard$ Creature.RememberedPlayerCtrl",
    json!([{"Continuous": {"duration": "UntilEndOfTurn", "mods": ["CantBlock"], "affected": {"Each": [
        {"Type": "Creature"}, {"ControlledBy": {"Target": 0}}]}}}])
)]
#[case::other_creatures_cant_attack(
    "ValidTgts$ Creature | RememberObjects$ Targeted",
    "Mode$ CantAttack | ValidCard$ Creature.!IsRemembered",
    json!([{"Continuous": {"duration": "UntilEndOfTurn", "mods": ["CantAttack"], "affected": {"Each": [
        {"Type": "Creature"}, {"Not": {"Is": {"Target": 0}}}]}}}])
)]
#[case::it_attacks_you(
    "ValidTgts$ Creature | RememberObjects$ Targeted",
    "Mode$ MustAttack | ValidCreature$ Creature.IsRemembered | MustAttack$ You",
    json!([{"Rule": {"duration": "UntilEndOfTurn", "rule": {"MustAttackPlayers": {
        "who": {"Target": 0}, "players": "You"}}}}])
)]
#[case::it_attacks_despite_defender(
    "ValidTgts$ Creature | RememberObjects$ Targeted",
    "Mode$ CanAttackDefender | ValidCard$ Card.IsRemembered",
    json!([{"Rule": {"duration": "UntilEndOfTurn", "rule": {"Combat": {
        "who": {"Target": 0}, "quirk": "AttackDespiteDefender"}}}}])
)]
#[case::it_assigns_damage_by_toughness(
    "ValidTgts$ Creature | RememberObjects$ Targeted",
    "Mode$ CombatDamageToughness | ValidCard$ Card.IsRemembered",
    json!([{"Rule": {"duration": "UntilEndOfTurn", "rule": {"Combat": {
        "who": {"Target": 0}, "quirk": "AssignsByToughness"}}}}])
)]
#[case::it_cant_attack_block_or_activate(
    "ValidTgts$ Creature | RememberObjects$ Targeted | Duration$ UntilYourNextTurn",
    "Mode$ CantAttack,CantBlock,CantBeActivated | ValidCard$ Card.IsRemembered",
    json!([
        {"Continuous": {"duration": "UntilYourNextTurn", "mods": ["CantAttack", "CantBlock"], "affected": {"Target": 0}}},
        {"Rule": {"duration": "UntilYourNextTurn", "rule": {"Cant": {"who": "EachPlayer", "action": {"ActivateKind": {
            "kind": "Every", "sources": [{"Is": {"Target": 0}}]}}}}}},
    ])
)]
fn an_effect_static_is_about_what_the_effect_remembers(
    #[case] line: &str,
    #[case] statics: &str,
    #[case] expected: Value,
) {
    let script = sorcery(line, statics);
    assert_eq!(gaps(&script), Vec::<String>::new());
    assert_eq!(effects(&script), expected);
}

#[rstest]
#[case::another_player_to_attack(
    "ValidTgts$ Creature | RememberObjects$ Targeted",
    "Mode$ MustAttack | ValidCreature$ Creature.IsRemembered | MustAttack$ RememberedPlayer",
    "key:Effect.static.MustAttack=RememberedPlayer"
)]
#[case::a_player_attacked_by_a_group(
    "ValidTgts$ Player | RememberObjects$ Targeted",
    "Mode$ MustAttack | ValidCreature$ Creature.RememberedPlayerCtrl | MustAttack$ You",
    "key:Effect.static.MustAttack=You"
)]
#[case::not_about_the_remembered(
    "ValidTgts$ Player | RememberObjects$ Targeted",
    "Mode$ CantBlock | ValidCard$ Creature.withFlying",
    "valid:Effect.static-not-about-the-remembered=Creature.withFlying"
)]
#[case::an_unread_key(
    "ValidTgts$ Creature | RememberObjects$ Targeted",
    "Mode$ CanAttackDefender | ValidCard$ Card.IsRemembered | ValidTarget$ You",
    "key:Effect.static.ValidTarget"
)]
fn an_effect_static_beyond_these_words_stays_a_gap(
    #[case] line: &str,
    #[case] statics: &str,
    #[case] gap: &str,
) {
    assert!(
        gaps(&sorcery(line, statics)).contains(&gap.to_owned()),
        "{:?}",
        gaps(&sorcery(line, statics))
    );
}

fn creature(line: &str) -> String {
    format!("Name:T\nTypes:Creature\nPT:2/2\n{line}\n")
}

#[rstest]
#[case::a_creature_that_player_controls(
    "SVar:Hit:DB$ Pump | ValidTgts$ Creature | TargetsWithDefinedController$ TriggeredAttackedTarget | KW$ Haste",
    json!({"Object": [{"Type": "Creature"}, {"ControlledBy": "Against"}]})
)]
#[case::a_creature_attacking_that_player(
    "SVar:Hit:DB$ Pump | ValidTgts$ Creature.attacking TriggeredAttackedTarget | KW$ Haste",
    json!({"Object": [{"Type": "Creature"}, {"AttackingPlayer": "Against"}]})
)]
fn the_player_a_group_attacks_names_the_target(#[case] effect: &str, #[case] what: Value) {
    let script = creature(&format!(
        "T:Mode$ AttackersDeclaredOneTarget | ValidAttackers$ Creature.YouCtrl | AttackedTarget$ Player | \
         TriggerZones$ Battlefield | Execute$ Hit\n{effect}"
    ));
    assert_eq!(gaps(&script), Vec::<String>::new());
    assert_eq!(
        abilities(&script)[0]["Triggered"]["targets"][0]["what"],
        what
    );
}

#[test]
fn attacking_the_player_of_one_attacker_stays_a_gap() {
    let script = creature(
        "T:Mode$ Attacks | ValidCard$ Creature | Execute$ Hit\n\
         SVar:Hit:DB$ Pump | ValidTgts$ Creature.attacking TriggeredAttackedTarget | KW$ Haste",
    );
    assert!(!gaps(&script).is_empty());
}

fn rule(script: &str) -> Value {
    abilities(script)
        .into_iter()
        .find_map(|ability| ability.get("Rule").map(|rule| rule["rule"].clone()))
        .unwrap_or(Value::Null)
}

#[rstest]
#[case::life_cant_change(
    "Mode$ CantChangeLife | ValidPlayer$ You",
    json!({"LifeTotalCantChange": "You"})
)]
#[case::opponents_cant_target(
    "Mode$ CantTarget | ValidTarget$ Card.Self | Activator$ Opponent",
    json!({"Cant": {"who": "This", "action": {"BeTargeted": [{"ControlledBy": "Opponents"}]}}})
)]
#[case::opponents_spells_of_a_color_cant_target(
    "Mode$ CantTarget | ValidTarget$ Card.Self | ValidSource$ Card.Black | ValidSA$ Spell | Activator$ Opponent",
    json!({"Cant": {"who": "This", "action": {"BeTargeted": [
        {"Color": "Black"}, "Spell", {"ControlledBy": "Opponents"}]}}})
)]
#[case::cant_be_equipped(
    "Mode$ CantAttach | ValidCard$ Card.Equipment | Target$ Card.Self",
    json!({"Cant": {"who": "This", "action": {"BeAttachedBy": [{"Subtype": "Equipment"}]}}})
)]
#[case::attached_only_to_the_tough(
    "Mode$ CantAttach | ValidCard$ Card.Self | Target$ Creature.toughnessLT4",
    json!({"Cant": {"who": {"Each": [{"Type": "Creature"},
        {"Compare": {"stat": "Toughness", "op": "Lt", "to": 4}}]}, "action": {"BeAttachedBy": ["This"]}}})
)]
#[case::two_attackers_at_you(
    "Mode$ AttackRestrict | MaxAttackers$ 2 | ValidDefender$ You",
    json!({"AttackLimitAt": {"most": 2, "at": "You"}})
)]
#[case::fewer_creatures(
    "Mode$ CantAttack | ValidCard$ Card.Self | UnlessDefender$ hasFewerCreaturesInPlayThanYou",
    json!({"Cant": {"who": "This", "action": {"Attack": {"unless": {"Compare": {
        "left": {"Count": [{"Type": "Creature"}, {"ControlledBy": "You"}]}, "op": "Lt",
        "right": {"Count": [{"Type": "Creature"}, {"ControlledBy": {"ControllerOf": "This"}}]}}}}}}})
)]
#[case::poisoned(
    "Mode$ CantAttack | ValidCard$ Card.Self | UnlessDefender$ IsPoisoned",
    json!({"Cant": {"who": "This", "action": {"Attack": {"unless": {"Compare": {
        "left": {"Counters": {"on": "You", "counter": "poison"}}, "op": "Ge", "right": 1}}}}}})
)]
#[case::a_full_graveyard(
    "Mode$ CantAttack | ValidCard$ Card.Self | UnlessDefender$ HasCardsInGraveyard_Card_GE7",
    json!({"Cant": {"who": "This", "action": {"Attack": {"unless": {"Compare": {
        "left": {"Count": [{"OwnedBy": "You"}, {"In": "Graveyard"}]}, "op": "Ge", "right": 7}}}}}})
)]
fn a_static_line_is_the_rule_it_prints(#[case] line: &str, #[case] expected: Value) {
    let script = creature(&format!("S:{line}"));
    assert_eq!(gaps(&script), Vec::<String>::new());
    assert_eq!(rule(&script), expected);
}

#[rstest]
#[case::cant_target_for_you(
    "Mode$ CantTarget | ValidTarget$ Card.Self | Activator$ You",
    "key:Static.CantTarget.Activator=You"
)]
#[case::attach_without_a_host(
    "Mode$ CantAttach | ValidCard$ Card.Self",
    "key:Static.CantAttach.ValidCard-and-Target"
)]
#[case::attackers_at_a_planeswalker(
    "Mode$ AttackRestrict | MaxAttackers$ 1 | ValidDefender$ Planeswalker.YouCtrl",
    "key:Static.AttackRestrict.ValidDefender"
)]
#[case::no_attackers(
    "Mode$ AttackRestrict | MaxAttackers$ 0 | ValidDefender$ You",
    "key:Static.AttackRestrict.MaxAttackers"
)]
fn a_static_line_beyond_these_words_stays_a_gap(#[case] line: &str, #[case] gap: &str) {
    assert_eq!(gaps(&creature(&format!("S:{line}"))), vec![gap.to_owned()]);
}

#[rstest]
#[case::cant_block("CARDNAME can't block.", json!(["CantBlock"]))]
#[case::cant_attack("CARDNAME can't attack.", json!(["CantAttack"]))]
fn a_visible_keyword_of_a_pump_is_the_restriction(#[case] keyword: &str, #[case] mods: Value) {
    let script = format!(
        "Name:T\nTypes:Instant\nA:SP$ Pump | ValidTgts$ Creature | KW$ {keyword} | IsCurse$ True\n"
    );
    assert_eq!(gaps(&script), Vec::<String>::new());
    assert_eq!(effects(&script)[0]["Continuous"]["mods"], mods);
}
