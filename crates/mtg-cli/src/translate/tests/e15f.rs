//! Goal: pack e15f keeps the names of modes and the words a permanent chooses as it enters in the rules: a `Charm` mode
//! takes its name from the words before the dash of its description (or the `PrecostDesc$` of a Tiered mode), a mode with
//! no such words has none, `GenericChoice | SetChosenMode$ True` as it enters is a choice of the printed words, and a line
//! of the card under `Affected$ Card.ChosenMode<word>` is its added ability under that word; the neighbouring shapes stay
//! gaps by name.
#![allow(clippy::unwrap_used)]

use rstest::rstest;

use super::{abilities, gaps};

/// The translation of a card with the given type line, script lines and Oracle text, as compact JSON, and its gaps.
fn card(types: &str, lines: &str, oracle: &str) -> (String, Vec<String>) {
    let text = format!("Name:Subject\nTypes:{types}\n{lines}\nOracle:{oracle}\n");
    (
        serde_json::to_string(&abilities(&text)).unwrap(),
        gaps(&text),
    )
}

const CHARM: &str = "A:SP$ Charm | Choices$ First,Second";

#[rstest]
#[case::a_named_mode(
    "SVar:First:DB$ Draw | SpellDescription$ Charge Them \u{2014} Draw a card.",
    Some("Charge Them")
)]
#[case::a_name_with_small_words(
    "SVar:First:DB$ Draw | SpellDescription$ Hire a Mercenary \u{2014} Draw a card.",
    Some("Hire a Mercenary")
)]
#[case::a_tiered_name(
    "SVar:First:DB$ Draw | PrecostDesc$ Cross-Slash | SpellDescription$ Draw a card.",
    Some("Cross-Slash")
)]
#[case::no_name("SVar:First:DB$ Draw | SpellDescription$ Draw a card.", None)]
#[case::a_sentence_before_the_dash(
    "SVar:First:DB$ Draw | SpellDescription$ Draw a card. Then \u{2014} more.",
    None
)]
#[case::the_card_name_before_the_dash(
    "SVar:First:DB$ Draw | SpellDescription$ CARDNAME Rises \u{2014} Draw a card.",
    None
)]
#[case::a_cost_before_the_dash(
    "SVar:First:DB$ Draw | SpellDescription$ + {1} \u{2014} Draw a card.",
    None
)]
fn e15f_a_mode_keeps_its_name(#[case] first: &str, #[case] label: Option<&str>) {
    let lines = format!(
        "{CHARM}\n{first}\nSVar:Second:DB$ GainLife | LifeAmount$ 2 | SpellDescription$ You gain 2 life."
    );
    let (json, gaps) = card(
        "Instant",
        &lines,
        "Choose one \u{2014}\n\u{2022} Draw a card.\n\u{2022} You gain 2 life.",
    );
    assert!(gaps.is_empty(), "{gaps:?}");
    match label {
        Some(label) => assert!(json.contains(&format!(r#""label":"{label}""#)), "{json}"),
        None => assert!(!json.contains("\"label\""), "{json}"),
    }
}

const SIEGE: &str = "K:ETBReplacement:Other:SiegeChoice
SVar:SiegeChoice:DB$ GenericChoice | Choices$ Khans,Dragons | Defined$ You | SetChosenMode$ True | AILogic$ Khans | ShowChoice$ ExceptSelf | SpellDescription$ As CARDNAME enters, choose Khans or Dragons.
SVar:Khans:DB$ Pump | SpellDescription$ Khans
SVar:Dragons:DB$ Pump | SpellDescription$ Dragons";

const SIEGE_ORACLE: &str = "As Subject enters, choose Khans or Dragons.\\n\u{2022} Khans \u{2014} At the beginning of your upkeep, draw a card.\\n\u{2022} Dragons \u{2014} Creatures you control get +1/+0.";

#[rstest]
#[case::a_trigger_and_a_static(
    "S:Mode$ Continuous | AffectedDefined$ Self | Affected$ Card.ChosenModeKhans | AddTrigger$ KhansTrigger | Description$ x
SVar:KhansTrigger:Mode$ Phase | Phase$ Upkeep | ValidPlayer$ You | TriggerZones$ Battlefield | Execute$ TrigDraw | Secondary$ True | TriggerDescription$ x
SVar:TrigDraw:DB$ Draw
S:Mode$ Continuous | AffectedDefined$ Self | Affected$ Card.ChosenModeDragons | AddStaticAbility$ DragonsStatic | Description$ x
SVar:DragonsStatic:Mode$ Continuous | Affected$ Creature.YouCtrl | AddPower$ 1 | Description$ x",
    &[
        r#"{"Replacement":{"by":{"ChooseWord":["Khans","Dragons"]},"when":{"Enters":{"what":["This"]}}}}"#,
        r#""if":{"EnteredAs":"Khans"}"#,
        r#""condition":{"EnteredAs":"Dragons"}"#,
    ]
)]
fn e15f_a_siege_keeps_its_words(#[case] lines: &str, #[case] parts: &[&str]) {
    let (json, gaps) = card("Enchantment", &format!("{SIEGE}\n{lines}"), SIEGE_ORACLE);
    assert!(gaps.is_empty(), "{gaps:?}");
    for part in parts {
        assert!(json.contains(part), "{part} in {json}");
    }
}

#[rstest]
#[case::a_word_with_a_sentence(
    "SVar:Khans:DB$ Pump | SpellDescription$ Khans. Then more",
    "key:ETBReplacement.GenericChoice-words"
)]
#[case::another_chooser(
    "SVar:SiegeChoice:DB$ GenericChoice | Choices$ Khans,Dragons | Defined$ Opponent | SetChosenMode$ True",
    "key:ETBReplacement.Defined"
)]
#[case::an_ability_and_a_static(
    "S:Mode$ Continuous | AffectedDefined$ Self | Affected$ Card.ChosenModeKhans | AddTrigger$ T | AddStaticAbility$ S | Description$ x
SVar:T:Mode$ Phase | Phase$ Upkeep | ValidPlayer$ You | TriggerZones$ Battlefield | Execute$ D
SVar:D:DB$ Draw
SVar:S:Mode$ Continuous | Affected$ Creature.YouCtrl | AddPower$ 1",
    "key:Static.ChosenMode-shape"
)]
#[case::a_keyword_under_the_word(
    "S:Mode$ Continuous | AffectedDefined$ Self | Affected$ Card.ChosenModeKhans | AddKeyword$ Flying | Description$ x",
    "key:Static.ChosenMode.AddKeyword"
)]
fn e15f_near_misses_stay_gaps(#[case] lines: &str, #[case] gap: &str) {
    // A later SVar of the same name replaces the earlier one.
    let (_, gaps) = card("Enchantment", &format!("{SIEGE}\n{lines}"), SIEGE_ORACLE);
    assert!(gaps.iter().any(|found| found == gap), "{gaps:?}");
}
