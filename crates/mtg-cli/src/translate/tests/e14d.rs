//! Goal: pack e14d's mechanics read from Forge lines into the engine's one model of each: recruit expands its keyword action, protection with an exception names what it spares, suspecting is the engine's designation, and a line with a key the mechanic does not read stays a gap.
use rstest::rstest;
use serde_json::{Value, json};

use super::{abilities, gaps};

fn plain(line: &str) -> String {
    format!("Name:T\nTypes:Sorcery\nA:SP$ {line}\n")
}

fn effects(text: &str) -> Vec<Value> {
    abilities(text)[0]["Spell"]["effects"]
        .as_array()
        .cloned()
        .unwrap_or_default()
}

#[test]
fn recruit_is_the_keyword_action_of_the_table() {
    let steps = mtg_cards::macros::action_effects("recruit", 0, &mtg_cards::refs::Ref::You)
        .unwrap_or_default();
    let expected: Vec<Value> = steps.iter().map(|step| json!(step)).collect();
    assert_eq!(gaps(&plain("Recruit")), Vec::<String>::new());
    assert_eq!(effects(&plain("Recruit")), expected);
}

fn ward(exception: &str) -> String {
    format!(
        "Name:T\nTypes:Enchantment Aura\nK:Enchant:Creature\nS:Mode$ Continuous | AffectedDefined$ Enchanted | AddKeyword$ Protection:Card.White:white:{exception} | Description$ T.\n"
    )
}

#[rstest]
#[case::this_aura("Card.CardUID_HostCardUID", json!(["This"]))]
#[case::auras("Aura", json!([{"Subtype": "Aura"}]))]
#[case::your_auras_and_equipment(
    "Aura.ControlledBy Player.PlayerUID_HostCardControllerUID,Equipment.ControlledBy Player.PlayerUID_HostCardControllerUID:SBA",
    json!([{"AnyOf": [{"Subtype": "Aura"}, {"Subtype": "Equipment"}]}, {"ControlledBy": "You"}])
)]
fn protection_with_an_exception_names_what_it_spares(
    #[case] exception: &str,
    #[case] spares: Value,
) {
    let text = ward(exception);
    assert_eq!(gaps(&text), Vec::<String>::new());
    let found = abilities(&text);
    let given = found
        .iter()
        .find_map(|ability| ability["Static"]["mods"].as_array().cloned())
        .unwrap_or_default();
    assert_eq!(
        given,
        [
            json!({"AddKeyword": {"ProtectionSparing": {"from": [{"Color": "White"}], "spares": spares}}})
        ]
    );
}

#[rstest]
#[case::another_card("Card.Creature")]
#[case::equipment_alone("Equipment")]
#[case::yours_without_the_state_check(
    "Aura.ControlledBy Player.PlayerUID_HostCardControllerUID,Equipment.ControlledBy Player.PlayerUID_HostCardControllerUID"
)]
fn an_unknown_exception_stays_a_gap(#[case] exception: &str) {
    assert_eq!(
        gaps(&ward(exception)),
        ["keyword:Protection-with-exception"]
    );
}

#[rstest]
#[case::itself("AlterAttribute | Defined$ Self | Attributes$ Suspected", json!([{"Suspect": {"what": "This"}}]))]
#[case::a_target(
    "AlterAttribute | ValidTgts$ Creature | Attributes$ Suspected",
    json!([{"Suspect": {"what": {"Target": 0}}}])
)]
#[case::no_longer(
    "AlterAttribute | Defined$ Self | Attributes$ Suspected | Activate$ False",
    json!([{"Unsuspect": {"what": "This"}}])
)]
fn suspect_reads_as_the_engine_designation(#[case] line: &str, #[case] expected: Value) {
    assert_eq!(gaps(&plain(line)), Vec::<String>::new());
    assert_eq!(Value::Array(effects(&plain(line))), expected);
}

#[rstest]
#[case::after_a_move(
    "AlterAttribute | Defined$ TriggeredNewCardLKICopy | Attributes$ Suspected",
    "key:AlterAttribute.Defined-after-a-move"
)]
#[case::plotted(
    "AlterAttribute | Defined$ Self | Attributes$ Plotted",
    "key:AlterAttribute.Attributes=Plotted"
)]
#[case::unprepared(
    "AlterAttribute | Defined$ Self | Attributes$ Prepared | Activate$ False",
    "key:AlterAttribute.Activate"
)]
fn other_attribute_changes_stay_gaps(#[case] line: &str, #[case] gap: &str) {
    assert_eq!(gaps(&plain(line)), [gap]);
}

#[test]
fn a_suspected_creature_is_a_valid_atom() {
    let text = plain("Destroy | ValidTgts$ Creature.IsSuspected");
    assert_eq!(gaps(&text), Vec::<String>::new());
    assert_eq!(
        abilities(&text)[0]["Spell"]["targets"][0]["what"],
        json!({"Object": [{"Type": "Creature"}, "Suspected"]})
    );
}

#[rstest]
#[case::a_number("Recruit | Num$ 2")]
#[case::another_player("Recruit | Defined$ Opponent")]
fn recruit_with_a_key_it_does_not_read_is_a_gap(#[case] line: &str) {
    assert_ne!(gaps(&plain(line)).len(), 0);
}
