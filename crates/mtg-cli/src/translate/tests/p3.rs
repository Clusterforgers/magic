//! Goal: piece P3 (memory): a slot read on another line is the source's memory, one read on its own line is a bound name,
//! a choice made as the permanent enters is read as such, and an unread `Defined$` word stays a gap named after the word.
use rstest::rstest;
use serde_json::{Value, json};

use super::{abilities, gaps};

#[test]
fn unknown_defined_word_is_a_gap() {
    let found = gaps("Name:T\nTypes:Sorcery\nA:SP$ Pump | Defined$ Nonsense | NumAtt$ +1\n");
    assert_eq!(found, ["defined:Nonsense"]);
}

const ENTERS: &str = "T:Mode$ ChangesZone | Origin$ Any | Destination$ Battlefield | ValidCard$ Card.Self | Execute$ Go | TriggerDescription$ x\n";

fn card(types: &str, lines: &str) -> String {
    format!("Name:T\nTypes:{types}\n{lines}")
}

/// The effects of the first ability whose key is `kind`.
fn effects_of(text: &str, kind: &str) -> Vec<Value> {
    let found = abilities(text);
    let ability = found.iter().find_map(|a| a.get(kind)).cloned();
    ability
        .and_then(|a| a.get("effects").cloned())
        .and_then(|e| e.as_array().cloned())
        .unwrap_or_default()
}

#[rstest]
#[case::imprint("Imprint$ True", "Defined$ Imprinted", "imprinted")]
#[case::remember_changed("RememberChanged$ True", "Defined$ Remembered", "remembered")]
fn a_slot_read_on_another_line_is_the_memory_of_the_source(
    #[case] write: &str,
    #[case] read: &str,
    #[case] slot: &str,
) {
    let text = card(
        "Artifact",
        &format!(
            "{ENTERS}SVar:Go:DB$ ChangeZone | Origin$ Hand | Destination$ Exile | ChangeType$ Instant | ChangeNum$ 1 | {write}\n\
             A:AB$ Pump | Cost$ T | {read} | NumAtt$ +1\n"
        ),
    );
    assert_eq!(gaps(&text), Vec::<String>::new());
    let remembers = effects_of(&text, "Triggered");
    assert_eq!(
        remembers.last(),
        Some(&json!({"Remember": {"slot": slot, "what": {"Bound": "exiled2"}}}))
    );
    let read = effects_of(&text, "Activated");
    assert_eq!(
        read[0]["Continuous"]["affected"],
        json!({"Remembered": {"on": "This", "slot": slot}})
    );
}

#[test]
fn a_slot_read_in_the_same_chain_is_a_bound_name_and_the_source_remembers_nothing() {
    let text = card(
        "Creature",
        &format!(
            "{ENTERS}SVar:Go:DB$ ChangeZone | Origin$ Hand | Destination$ Exile | ChangeType$ Instant | ChangeNum$ 1 | Imprint$ True | SubAbility$ Pump\n\
             SVar:Pump:DB$ Pump | Defined$ Imprinted | NumAtt$ +1\n"
        ),
    );
    assert_eq!(gaps(&text), Vec::<String>::new());
    let found = effects_of(&text, "Triggered");
    assert!(
        found.iter().all(|e| e.get("Remember").is_none()),
        "{found:?}"
    );
    assert_eq!(
        found[2]["Continuous"]["affected"],
        json!({"Bound": "exiled2"})
    );
}

#[test]
fn the_remember_verb_stores_its_object_for_the_other_lines_and_cleanup_forgets_it() {
    let text = card(
        "Enchantment",
        "T:Mode$ Attacks | ValidCard$ Creature | Execute$ Go | TriggerDescription$ x\n\
         SVar:Go:DB$ Remember | Defined$ TriggeredAttacker | SubAbility$ Done\n\
         SVar:Done:DB$ Cleanup | ClearRemembered$ True\n\
         S:Mode$ Continuous | Affected$ Creature.IsRemembered | AddKeyword$ Flying | Description$ x\n",
    );
    let found = effects_of(&text, "Triggered");
    assert_eq!(found[0]["Remember"]["slot"], json!("remembered"));
    assert_eq!(found[1], json!({"Forget": {"slot": "remembered"}}));
}

#[test]
fn a_slot_nobody_writes_stays_a_gap_where_it_is_read() {
    let text = card(
        "Creature",
        "A:AB$ Pump | Cost$ T | Defined$ Remembered | NumAtt$ +1\n",
    );
    assert_eq!(gaps(&text), ["defined:Remembered-without-a-change"]);
}

#[rstest]
#[case::color(
    "ChooseColor",
    "DB$ ChooseColor | Defined$ You",
    "Creature.ChosenColor",
    json!("ChosenColor")
)]
#[case::creature_type(
    "ChooseCT",
    "DB$ ChooseType | Defined$ You | Type$ Creature",
    "Creature.ChosenType",
    json!("ChosenSubtype")
)]
#[case::card_name(
    "DBNameCard",
    "DB$ NameCard | Defined$ You",
    "Card.NamedCard",
    json!("ChosenName")
)]
fn a_choice_made_as_it_enters_is_read_from_the_source(
    #[case] part: &str,
    #[case] body: &str,
    #[case] valid: &str,
    #[case] pred: Value,
) {
    let text = card(
        "Enchantment",
        &format!(
            "K:ETBReplacement:Other:{part}\nSVar:{part}:{body}\n\
             S:Mode$ Continuous | Affected$ {valid} | AddKeyword$ Flying | Description$ x\n"
        ),
    );
    assert_eq!(gaps(&text), Vec::<String>::new());
    let found = abilities(&text);
    assert!(
        json!(found).to_string().contains(&pred.to_string()),
        "{found:?}"
    );
}

#[test]
fn a_choice_of_a_land_type_is_not_a_creature_type_choice() {
    let text = card(
        "Enchantment",
        "K:ETBReplacement:Other:ChooseCT\nSVar:ChooseCT:DB$ ChooseType | Defined$ You | Type$ Basic Land\n\
         S:Mode$ Continuous | Affected$ Creature.ChosenType | AddKeyword$ Flying | Description$ x\n",
    );
    assert_eq!(
        gaps(&text),
        [
            "keyword:ETBReplacement-ChooseType",
            "valid:ChosenType-without-a-choice"
        ]
    );
}
