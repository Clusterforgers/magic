//! Goal: piece P4 (ability header): keys shared by every API are read once, before the verb's mapper, and a value they do not know stays a named gap.
use rstest::rstest;
use serde_json::{Value, json};

use super::{abilities, gaps};

fn spell(line: &str) -> String {
    format!("Name:T\nTypes:Instant\nA:{line}\n")
}

/// The first ability of a spell line.
fn first(line: &str) -> Value {
    abilities(&spell(line))
        .into_iter()
        .next()
        .unwrap_or_default()
}

#[test]
fn unknown_trigger_mode_is_a_gap() {
    let found = gaps(
        "Name:T\nTypes:Creature Elf\nT:Mode$ Nonsense | ValidCard$ Card.Self | Execute$ Go\nSVar:Go:DB$ Draw\n",
    );
    assert_eq!(found, ["trigger:Nonsense"]);
}

#[rstest]
#[case::graveyard_owned(
    "SP$ Pump | ValidTgts$ Creature.YouCtrl | TgtZone$ Graveyard | NumAtt$ +1",
    json!([{"Type": "Creature"}, {"OwnedBy": "You"}, {"In": "Graveyard"}])
)]
#[case::two_zones(
    "SP$ Pump | ValidTgts$ Creature | TgtZone$ Graveyard,Exile | NumAtt$ +1",
    json!([{"Type": "Creature"}, {"AnyOf": [{"In": "Graveyard"}, {"In": "Exile"}]}])
)]
#[case::spell_of_an_opponent(
    "SP$ Pump | ValidTgts$ Card | TgtZone$ Stack | TargetType$ Spell.OppCtrl | NumAtt$ +1",
    json!([{"ControlledBy": "Opponents"}, {"In": "Stack"}])
)]
fn a_target_zone_becomes_a_property_of_the_target(#[case] line: &str, #[case] selector: Value) {
    let found = first(line);
    assert_eq!(
        found["Spell"]["targets"][0]["what"],
        json!({"Object": selector})
    );
}

#[rstest]
#[case::ante("SP$ Pump | ValidTgts$ Creature | TgtZone$ Ante | NumAtt$ +1")]
#[case::permanent("SP$ Pump | ValidTgts$ Permanent | TgtZone$ Graveyard | NumAtt$ +1")]
#[case::player("SP$ Pump | ValidTgts$ Player | TgtZone$ Graveyard | NumAtt$ +1")]
#[case::not_controlled(
    "SP$ Pump | ValidTgts$ Creature.YouDontCtrl | TgtZone$ Graveyard | NumAtt$ +1"
)]
#[case::unknown_target_type(
    "SP$ Pump | ValidTgts$ Card | TgtZone$ Stack | TargetType$ Spell.singleTarget | NumAtt$ +1"
)]
#[case::mixed_with_open("SP$ Pump | ValidTgts$ Creature | TgtZone$ Battlefield,Exile | NumAtt$ +1")]
fn a_target_zone_the_selector_cannot_state_stays_a_gap(#[case] line: &str) {
    assert!(gaps(&spell(line)).into_iter().next().is_some());
}

/// A spell that draws, then gains 2 life under the extra keys.
fn chain(extra: &str) -> String {
    format!(
        "Name:T\nTypes:Instant\nA:SP$ Draw | NumCards$ 1 | SubAbility$ B\nSVar:B:DB$ GainLife | LifeAmount$ 2 | {extra}\n"
    )
}

#[rstest]
#[case::your_turn("ConditionPlayerTurn$ True", json!("YourTurn"))]
#[case::not_your_turn("ConditionPlayerTurn$ False", json!({"Not": "YourTurn"}))]
#[case::mana_spent("ConditionManaSpent$ W", json!({"ManaSpent": "White"}))]
#[case::both_colors_spent(
    "ConditionManaSpent$ W U",
    json!({"All": [{"ManaSpent": "White"}, {"ManaSpent": "Blue"}]})
)]
#[case::mana_not_spent("ConditionManaNotSpent$ G", json!({"Not": {"ManaSpent": "Green"}}))]
#[case::fewer_life(
    "ConditionLifeTotal$ You | ConditionLifeAmount$ LT3",
    json!({"Compare": {"left": {"Life": "You"}, "op": "Lt", "right": 3}})
)]
#[case::none_present(
    "ConditionNotPresent$ Creature.token",
    json!({"Compare": {"left": {"Count": [{"Type": "Creature"}, "Token"]}, "op": "Eq", "right": 0}})
)]
#[case::second_clause(
    "ConditionPresent$ Artifact.YouCtrl | ConditionPresent2$ Enchantment.YouCtrl",
    json!({"All": [
        {"Compare": {"left": {"Count": [{"Type": "Artifact"}, {"ControlledBy": "You"}]}, "op": "Ge", "right": 1}},
        {"Compare": {"left": {"Count": [{"Type": "Enchantment"}, {"ControlledBy": "You"}]}, "op": "Ge", "right": 1}}]})
)]
#[case::kicked("Condition$ Kicked", json!("Kicked"))]
#[case::blessing("Condition$ Blessing", json!({"CitysBlessing": "You"}))]
#[case::monarch("Condition$ Monarch", json!({"Monarch": "You"}))]
#[case::night("Condition$ Night", json!("IsNight"))]
fn a_condition_key_of_a_chain_line_gates_the_effect(#[case] keys: &str, #[case] expected: Value) {
    let found = first_of(&chain(keys));
    assert_eq!(found["Spell"]["effects"][1]["If"]["condition"], expected);
}

fn first_of(text: &str) -> Value {
    abilities(text).into_iter().next().unwrap_or_default()
}

/// An activated ability of a creature under extra keys.
fn activated(extra: &str) -> Value {
    let text =
        format!("Name:T\nTypes:Creature Elf\nA:AB$ Draw | Cost$ 1 | NumCards$ 1 | {extra}\n");
    first_of(&text)
}

#[rstest]
#[case::upkeep("ActivationPhases$ Upkeep", json!({"Step": "Upkeep"}))]
#[case::range(
    "ActivationPhases$ Declare Blockers->EndCombat",
    json!({"Any": [{"Step": "DeclareBlockers"}, {"Step": "FirstStrikeDamage"}, {"Step": "CombatDamage"}, {"Step": "EndCombat"}]})
)]
#[case::combat("ActivationPhases$ BeginCombat->EndCombat", json!("InCombat"))]
#[case::your_end_step(
    "ActivationPhases$ End of Turn | PlayerTurn$ True",
    json!({"All": ["YourTurn", {"Step": "End"}]})
)]
#[case::boast(
    "Boast$ True",
    json!({"Matches": {"subject": "This", "selector": ["AttackedThisTurn"]}})
)]
#[case::forecast("Forecast$ True", json!({"All": ["YourTurn", {"Step": "Upkeep"}]}))]
fn activation_words_are_a_condition_of_the_ability(#[case] keys: &str, #[case] expected: Value) {
    assert_eq!(activated(keys)["Activated"]["condition"], expected);
}

#[rstest]
#[case::boast("Boast$ True", Some("OncePerTurn"), None)]
#[case::forecast("Forecast$ True", Some("OncePerTurn"), Some("Hand"))]
fn boast_and_forecast_limit_the_ability(
    #[case] keys: &str,
    #[case] limit: Option<&str>,
    #[case] from: Option<&str>,
) {
    let found = activated(keys);
    assert_eq!(found["Activated"]["limit"].as_str(), limit);
    assert_eq!(found["Activated"]["from"].as_str(), from);
}

#[rstest]
#[case::unknown_phase("ActivationPhases$ Dusk")]
#[case::backwards("ActivationPhases$ EndCombat->Upkeep")]
#[case::boast_false("Boast$ False")]
#[case::forecast_elsewhere("Forecast$ True | ActivationZone$ Graveyard")]
fn an_activation_word_the_table_does_not_know_stays_a_gap(#[case] keys: &str) {
    let text = format!("Name:T\nTypes:Creature Elf\nA:AB$ Draw | Cost$ 1 | NumCards$ 1 | {keys}\n");
    assert!(gaps(&text).into_iter().next().is_some());
}

#[rstest]
#[case::your_turn(
    "PlayerTurn$ True",
    json!({"CastOnly": {"condition": "YourTurn"}})
)]
#[case::combat_on_your_turn(
    "PlayerTurn$ True | ActivationPhases$ BeginCombat->EndCombat",
    json!({"CastOnly": {"condition": {"All": ["YourTurn", "InCombat"]}}})
)]
fn a_spell_that_can_only_be_cast_at_some_times_says_so(
    #[case] keys: &str,
    #[case] expected: Value,
) {
    let found = abilities(&spell(&format!("SP$ Draw | NumCards$ 1 | {keys}")));
    assert_eq!(found[0], expected);
    assert!(found[1].get("Spell").is_some());
}

/// A trigger of the card entering, under extra keys.
fn trigger(extra: &str) -> String {
    format!(
        "Name:T\nTypes:Creature Elf\nT:Mode$ ChangesZone | Origin$ Any | Destination$ Battlefield | ValidCard$ Card.Self | Execute$ Go | {extra}\nSVar:Go:DB$ Draw\n"
    )
}

#[rstest]
#[case::your_turn("PlayerTurn$ True", json!({"AtTrigger": "YourTurn"}))]
#[case::opponents_turn("OpponentTurn$ True", json!({"AtTrigger": {"Not": "YourTurn"}}))]
#[case::first_combat("FirstCombat$ True", json!("FirstCombat"))]
#[case::delirium(
    "Delirium$ True",
    json!({"Compare": {"left": {"CardTypes": [{"In": "Graveyard"}, {"OwnedBy": "You"}]}, "op": "Ge", "right": 4}})
)]
fn a_gating_word_of_a_trigger_is_its_if(#[case] keys: &str, #[case] expected: Value) {
    let found = first_of(&trigger(keys));
    assert_eq!(found["Triggered"]["if"], expected);
}

#[rstest]
#[case::revolt("Revolt$ True")]
#[case::not_true("PlayerTurn$ False")]
fn a_gating_word_the_table_does_not_know_stays_a_gap(#[case] keys: &str) {
    assert!(gaps(&trigger(keys)).into_iter().next().is_some());
}

#[rstest]
#[case::turn_word("ConditionPlayerTurn$ Maybe")]
#[case::two_colors_not_spent("ConditionManaNotSpent$ R G")]
#[case::other_player_life("ConditionLifeTotal$ TargetedOrController | ConditionLifeAmount$ GT3")]
#[case::life_without_amount("ConditionLifeTotal$ You")]
#[case::unknown_word("Condition$ MaxSpeed")]
#[case::not_present_beside_present("ConditionNotPresent$ Card | ConditionPresent$ Land")]
fn a_condition_value_the_table_does_not_know_stays_a_gap(#[case] keys: &str) {
    assert!(gaps(&chain(keys)).into_iter().next().is_some());
}

#[rstest]
#[case::exhaust("Exhaust$ True", Some("OncePerGame"))]
#[case::once_a_game("GameActivationLimit$ 1", Some("OncePerGame"))]
#[case::beside_once_a_turn("ActivationLimit$ 1 | GameActivationLimit$ 1", Some("OncePerGame"))]
#[case::once_a_turn("ActivationLimit$ 1", Some("OncePerTurn"))]
#[case::unlimited("Defined$ You", None)]
fn an_ability_used_once_a_game_has_that_limit(#[case] keys: &str, #[case] limit: Option<&str>) {
    assert_eq!(activated(keys)["Activated"]["limit"].as_str(), limit);
}

fn each_player(valid: &str, bounds: &str) -> String {
    spell(&format!(
        "SP$ Destroy | ValidTgts$ {valid} | {bounds} | TargetsForEachPlayer$ True"
    ))
}

#[rstest]
#[case::each_opponent(
    "Artifact.OppCtrl",
    json!({"PerPlayer": {"players": "Opponents", "what": [{"Type": "Artifact"}]}})
)]
#[case::kept_property(
    "Creature.OppCtrl+nonToken",
    json!({"PerPlayer": {"players": "Opponents", "what": [{"Type": "Creature"}, {"Not": "Token"}]}})
)]
#[case::each_player(
    "Creature",
    json!({"PerPlayer": {"players": "EachPlayer", "what": [{"Type": "Creature"}]}})
)]
fn up_to_one_target_for_each_player_is_a_per_player_target(
    #[case] valid: &str,
    #[case] expected: Value,
) {
    let text = each_player(valid, "TargetMin$ 0 | TargetMax$ OneEach");
    assert_eq!(gaps(&text), Vec::<String>::new());
    let found = first_of(&text);
    assert_eq!(found["Spell"]["targets"][0]["what"], expected);
}

#[rstest]
#[case::exactly_one_each("Artifact.OppCtrl", "TargetMin$ OneEach | TargetMax$ OneEach")]
#[case::two_at_most("Artifact.OppCtrl", "TargetMin$ 0 | TargetMax$ 2")]
#[case::no_bounds("Artifact.OppCtrl", "Defined$ You")]
#[case::you_control("Artifact.YouCtrl", "TargetMin$ 0 | TargetMax$ OneEach")]
#[case::mixed_control("Artifact.OppCtrl,Creature", "TargetMin$ 0 | TargetMax$ OneEach")]
fn for_each_player_bounds_the_table_does_not_know_stay_a_gap(
    #[case] valid: &str,
    #[case] bounds: &str,
) {
    assert!(
        gaps(&each_player(valid, bounds))
            .into_iter()
            .next()
            .is_some()
    );
}

/// A spell that pumps `first`, then pumps `second` (a `ValidTgts$` plus extra keys) in a sub-ability.
fn two_pumps(first: &str, second: &str) -> String {
    format!(
        "Name:T\nTypes:Sorcery\nA:SP$ Pump | ValidTgts$ {first} | NumAtt$ +1 | SubAbility$ B\nSVar:B:DB$ Pump | ValidTgts$ {second} | KW$ Flying\n"
    )
}

#[rstest]
#[case::one_earlier(
    "Creature",
    "Creature | TargetUnique$ True",
    json!([{"Type": "Creature"}, {"Not": {"Is": {"Target": 0}}}])
)]
#[case::with_properties(
    "Creature.YouCtrl",
    "Creature.OppCtrl | TargetUnique$ True",
    json!([{"Type": "Creature"}, {"ControlledBy": "Opponents"}, {"Not": {"Is": {"Target": 0}}}])
)]
#[case::player_before_is_no_object(
    "Player",
    "Creature | TargetUnique$ True",
    json!([{"Type": "Creature"}])
)]
#[case::not_unique("Creature", "Creature | TargetUnique$ False", json!([{"Type": "Creature"}]))]
fn a_unique_target_is_none_of_the_earlier_ones(
    #[case] first: &str,
    #[case] second: &str,
    #[case] selector: Value,
) {
    let found = first_of(&two_pumps(first, second));
    assert_eq!(
        found["Spell"]["targets"][1]["what"],
        json!({"Object": selector})
    );
}

#[test]
fn a_unique_target_with_nothing_before_it_adds_nothing() {
    let found = first_of(&spell(
        "SP$ Pump | ValidTgts$ Creature | NumAtt$ +1 | TargetUnique$ True",
    ));
    assert_eq!(
        found["Spell"]["targets"][0]["what"],
        json!({"Object": [{"Type": "Creature"}]})
    );
}

#[rstest]
#[case::player_after_player("Player", "Player | TargetUnique$ True")]
#[case::any_after_any("Any", "Any | TargetUnique$ True")]
#[case::player_alternative_after_any("Any", "Creature,Player | TargetUnique$ True")]
#[case::unknown_value("Creature", "Creature | TargetUnique$ Maybe")]
fn a_unique_target_that_could_be_a_player_stays_a_gap(#[case] first: &str, #[case] second: &str) {
    assert!(gaps(&two_pumps(first, second)).into_iter().next().is_some());
}

#[rstest]
#[case::twice_a_game("GameActivationLimit$ 2")]
#[case::exhaust_false("Exhaust$ False")]
#[case::exhaust_beside_two_a_turn("Exhaust$ True | ActivationLimit$ 2")]
fn a_game_limit_the_table_does_not_know_stays_a_gap(#[case] keys: &str) {
    let text = format!("Name:T\nTypes:Creature Elf\nA:AB$ Draw | Cost$ 1 | NumCards$ 1 | {keys}\n");
    assert!(gaps(&text).into_iter().next().is_some());
}
