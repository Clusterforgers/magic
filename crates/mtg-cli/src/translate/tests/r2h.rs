//! Goal: the smaller named mechanics (each-damage, empower, endure, bargain and teamwork reads, extra entering counters, the
//! keyword forms with two kickers, an equip restriction or awaken) translate to nodes that parse, and a shape beyond them
//! stays a named gap.
use rstest::rstest;
use serde_json::{Value, json};

use super::{abilities, gaps, normal};

fn spell(lines: &str) -> String {
    format!("Name:T\nTypes:Sorcery\n{lines}")
}

#[rstest]
#[case::to_itself(
    "A:SP$ EachDamage | ValidCards$ Creature | EachToItself$ True | NumDmg$ Count$CardPower\n",
    json!({"RepeatEach": {"over": {"Each": [{"Type": "Creature"}]}, "as": "damager", "effects": [
        {"DealDamage": {"source": {"Bound": "damager"}, "to": {"Bound": "damager"},
            "amount": {"Stat": {"of": {"Bound": "damager"}, "stat": "Power"}}}}]}})
)]
#[case::each_you_control_hits_a_target(
    "A:SP$ EachDamage | ValidCards$ Creature.YouCtrl | ValidTgts$ Creature.OppCtrl | NumDmg$ X\nSVar:X:Count$CardToughness\n",
    json!({"RepeatEach": {"over": {"Each": [{"Type": "Creature"}, {"ControlledBy": "You"}]}, "as": "damager", "effects": [
        {"DealDamage": {"source": {"Bound": "damager"}, "to": {"Target": 0},
            "amount": {"Stat": {"of": {"Bound": "damager"}, "stat": "Toughness"}}}}]}})
)]
fn each_damage_reads_the_stat_of_each_damager(#[case] lines: &str, #[case] node: Value) {
    let script = spell(lines);
    let found = abilities(&script);
    assert_eq!(found.len(), 1, "{:?}", gaps(&script));
    let effects = &found[0]["Spell"]["effects"];
    assert_eq!(effects[0], node);
}

#[rstest]
#[case::no_recipient(
    "A:SP$ EachDamage | ValidCards$ Creature | NumDmg$ 2\n",
    "key:EachDamage.recipient"
)]
#[case::source_stat(
    "A:SP$ EachDamage | ValidCards$ Creature | EachToItself$ True | NumDmg$ X\nSVar:X:Count$CardManaCost\n",
    "amount:EachDamage-of-the-source"
)]
fn each_damage_beyond_the_shape_is_a_gap(#[case] lines: &str, #[case] gap: &str) {
    let script = spell(lines);
    assert_eq!(abilities(&script), Vec::<Value>::new());
    assert_eq!(gaps(&script), vec![gap.to_owned()]);
}

#[rstest]
#[case::empower("A:SP$ Empower | Type$ Jace | Num$ 2\n", "Jace")]
#[case::endure("A:SP$ Endure | Num$ 3\n", "+1/+1")]
fn the_keyword_actions_expand_with_their_number(#[case] lines: &str, #[case] word: &str) {
    let script = spell(lines);
    let found = abilities(&script);
    assert_eq!(found.len(), 1, "{:?}", gaps(&script));
    let text = found[0].to_string();
    assert!(
        text.contains(word) && text.contains('3') | text.contains('2'),
        "{text}"
    );
}

#[test]
fn empower_of_another_kind_is_a_gap() {
    let script = spell("A:SP$ Empower | Type$ Nissa | Num$ 2\n");
    assert_eq!(gaps(&script), vec!["key:Empower.Type".to_owned()]);
}

#[rstest]
#[case::count(
    "K:Bargain\nA:SP$ DealDamage | ValidTgts$ Any | NumDmg$ Z\nSVar:Z:Count$Bargained.4.2\n",
    json!({"When": {"condition": "AdditionalCostPaid", "then": 4, "otherwise": 2}})
)]
#[case::teamwork_count(
    "K:Teamwork:2\nA:SP$ DealDamage | ValidTgts$ Any | NumDmg$ Z\nSVar:Z:Count$Teamwork.4.2\n",
    json!({"When": {"condition": "AdditionalCostPaid", "then": 4, "otherwise": 2}})
)]
fn a_cost_paid_count_reads_the_additional_cost(#[case] lines: &str, #[case] amount: Value) {
    let script = spell(lines);
    let found = abilities(&script);
    assert_eq!(found.len(), 2, "{:?}", gaps(&script));
    assert_eq!(
        found[1]["Spell"]["effects"][0]["DealDamage"]["amount"],
        amount
    );
}

#[rstest]
#[case::condition("Condition$ Bargain")]
#[case::present("ConditionZone$ Stack | ConditionPresent$ Card.Self+Teamwork")]
fn a_line_of_a_cost_paid_spell_has_the_additional_cost_condition(#[case] condition: &str) {
    let script = spell(&format!(
        "K:Bargain\nA:SP$ GainLife | LifeAmount$ 3 | SubAbility$ More\nSVar:More:DB$ Draw | {condition}\n"
    ));
    let found = abilities(&script);
    assert_eq!(found.len(), 2, "{:?}", gaps(&script));
    assert_eq!(
        found[1]["Spell"]["effects"][1],
        json!({"If": {"condition": "AdditionalCostPaid", "then": [{"Draw": {}}]}})
    );
}

#[test]
fn a_bargain_cost_reduction_is_a_condition_of_the_spell_itself() {
    let script = spell(
        "K:Bargain\nS:Mode$ ReduceCost | ValidSpell$ Spell.Bargain | Type$ Spell | ValidCard$ Card.Self | Amount$ 2 | EffectZone$ All\n",
    );
    let found = abilities(&script);
    assert_eq!(found.len(), 2, "{:?}", gaps(&script));
    assert_eq!(
        found[1]["CostModifier"]["condition"],
        json!("AdditionalCostPaid")
    );
}

#[rstest]
#[case::battlefield(
    "K:ETBReplacement:Other:Extra:Mandatory:Battlefield:Creature.Wizard+Other+YouCtrl\nSVar:Extra:DB$ PutCounter | ETB$ True | Defined$ ReplacedCard | CounterType$ P1P1 | CounterNum$ 1\n",
    None
)]
#[case::graveyard(
    "K:ETBReplacement:Other:Extra:Mandatory:Graveyard:Creature.Elf+YouCtrl\nSVar:Extra:DB$ PutCounter | ETB$ True | Defined$ ReplacedCard | CounterType$ P1P1 | CounterNum$ 1\n",
    Some("Graveyard")
)]
fn other_creatures_enter_with_extra_counters(#[case] lines: &str, #[case] from: Option<&str>) {
    let script = format!("Name:T\nTypes:Creature Elf\nPT:1/1\n{lines}");
    let found = abilities(&script);
    assert_eq!(found.len(), 1, "{:?}", gaps(&script));
    let replacement = &found[0]["Replacement"];
    assert_eq!(replacement["by"]["WithCounters"]["count"], json!(1));
    assert_eq!(replacement["from"].as_str(), from);
    assert_ne!(replacement["when"]["Enters"]["what"], json!(["This"]));
}

#[rstest]
#[case::two_kickers("K:Kicker:1 U:B\n", 2)]
#[case::awaken("K:Awaken:3:4 U\n", 1)]
#[case::equip_legendary("K:Equip:3:Creature.YouCtrl+Legendary:legendary creature\n", 1)]
fn keyword_forms_with_a_second_field(#[case] lines: &str, #[case] count: usize) {
    let script = format!("Name:T\nTypes:Artifact Equipment\n{lines}");
    let found = abilities(&script);
    assert_eq!(found.len(), count, "{:?}", gaps(&script));
    for ability in found {
        normal(ability);
    }
}

#[rstest]
#[case::number("K:Reinforce:2:1 G\n", json!(2), "{1}{G}")]
#[case::x("K:Reinforce:X:X W W\n", json!("X"), "{X}{W}{W}")]
fn reinforce_is_an_ability_of_the_hand_that_discards_the_card(
    #[case] line: &str,
    #[case] count: Value,
    #[case] mana: &str,
) {
    let script = format!("Name:T\nTypes:Instant\n{line}");
    let found = abilities(&script);
    assert_eq!(found.len(), 1, "{:?}", gaps(&script));
    let ability = &found[0]["Activated"];
    assert_eq!(ability["from"], json!("Hand"));
    assert_eq!(ability["cost"], json!([{"Mana": mana}, "DiscardSelf"]));
    assert_eq!(ability["effects"][0]["PutCounters"]["count"], count);
}

#[test]
fn detain_stops_attack_block_and_activation_until_the_next_turn() {
    let script =
        spell("A:SP$ Detain | ValidTgts$ Creature.OppCtrl | TargetMin$ 0 | TargetMax$ 2\n");
    let found = abilities(&script);
    assert_eq!(found.len(), 1, "{:?}", gaps(&script));
    let effects = &found[0]["Spell"]["effects"];
    assert_eq!(
        effects[0]["Continuous"]["mods"],
        json!(["CantAttack", "CantBlock"])
    );
    assert_eq!(
        effects[0]["Continuous"]["duration"],
        json!("UntilYourNextTurn")
    );
    assert_eq!(
        effects[1]["Rule"]["rule"]["Cant"]["action"],
        json!({"ActivateKind": {"kind": "Every", "sources": [{"Is": {"Target": 0}}]}})
    );
}

#[test]
fn a_descended_end_step_trigger_checks_the_tally_as_an_intervening_if() {
    let script = "Name:T\nTypes:Creature Elf\nPT:1/1\nT:Mode$ Phase | Phase$ End of Turn | ValidPlayer$ You.descended | TriggerZones$ Battlefield | Execute$ Draw\nSVar:Draw:DB$ Draw\n";
    let found = abilities(script);
    assert_eq!(found.len(), 1, "{:?}", gaps(script));
    let ability = &found[0]["Triggered"];
    assert_eq!(ability["when"]["StepBegins"]["whose"], json!("You"));
    assert_eq!(
        ability["if"],
        json!({"Compare": {"left": {"Tallied": {"who": "You", "what": "Descended"}}, "op": "Ge", "right": 1}})
    );
}

#[test]
fn the_descended_count_reads_the_tally() {
    let script =
        spell("A:SP$ DealDamage | ValidTgts$ Any | NumDmg$ X\nSVar:X:Count$YouDescendedThisTurn\n");
    let found = abilities(&script);
    assert_eq!(found.len(), 1, "{:?}", gaps(&script));
    assert_eq!(
        found[0]["Spell"]["effects"][0]["DealDamage"]["amount"],
        json!({"Tallied": {"who": "You", "what": "Descended"}})
    );
}

#[test]
fn equip_to_a_noncreature_is_a_gap() {
    let script = "Name:T\nTypes:Artifact Equipment\nK:Equip:3:Planeswalker.YouCtrl:planeswalker\n";
    assert_eq!(abilities(script), Vec::<Value>::new());
    assert_eq!(
        gaps(script),
        vec!["keyword:Equip-to-a-noncreature".to_owned()]
    );
}
