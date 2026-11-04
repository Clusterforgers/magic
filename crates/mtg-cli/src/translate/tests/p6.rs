//! Goal: piece P6 of the rules plan translates its constructs, one rstest table per rule.
#![allow(clippy::unwrap_used)]

use rstest::rstest;

use super::{abilities, gaps};

/// The translation of a sorcery with the given spell line (and its `SVar` lines) as compact JSON, and its gaps.
fn sorcery(lines: &str) -> (String, Vec<String>) {
    let text = format!("Name:Subject\nTypes:Sorcery\n{lines}\n");
    (
        serde_json::to_string(&abilities(&text)).unwrap(),
        gaps(&text),
    )
}

#[rstest]
#[case::defined_once(
    "A:SP$ ChangeZone | Origin$ Graveyard | Destination$ Exile | ChangeType$ Card.YouOwn | RememberChanged$ True | SubAbility$ P\nSVar:P:DB$ Play | Defined$ Remembered | WithoutManaCost$ True | Optional$ True | ValidSA$ Spell",
    &["CastFromZone", "\"free\":true"],
    &["RepeatEach", "\"play\""]
)]
#[case::all_exiled_with_source(
    "A:SP$ Draw | SubAbility$ P\nSVar:P:DB$ Play | Valid$ Card.YouOwn | ValidSA$ Spell | ValidZone$ Exile | WithoutManaCost$ True | Amount$ All | Controller$ You | Optional$ True",
    &["RepeatEach", "CastFromZone"],
    &["\"Choose\"", "\"play\""]
)]
#[case::up_to_three_from_hand(
    "A:SP$ Draw | SubAbility$ P\nSVar:P:DB$ Play | Valid$ Card.YouCtrl+nonLand | ValidZone$ Hand | Controller$ You | WithoutManaCost$ True | Amount$ 3 | Optional$ True",
    &["\"Choose\"", "\"max\":3", "RepeatEach"],
    &[]
)]
#[case::one_chosen_from_graveyard(
    "A:SP$ Draw | SubAbility$ P\nSVar:P:DB$ Play | Valid$ Card.YouOwn | ValidZone$ Graveyard | ValidSA$ Spell | WithoutManaCost$ True | Optional$ True",
    &["\"Choose\"", "\"max\":1", "CastFromZone"],
    &["RepeatEach"]
)]
#[case::copy_of_exiled(
    "A:SP$ Draw | SubAbility$ P\nSVar:P:DB$ Play | Valid$ Card.YouOwn | ValidZone$ Exile | WithoutManaCost$ True | ValidSA$ Spell | Optional$ True | CopyCard$ True",
    &["CopyCard", "CastFromZone"],
    &[]
)]
fn play_words(#[case] lines: &str, #[case] has: &[&str], #[case] lacks: &[&str]) {
    let (json, gaps) = sorcery(lines);
    assert!(
        gaps.iter().all(|gap| !gap.contains("Play")),
        "gaps: {gaps:?}"
    );
    for word in has {
        assert!(json.contains(word), "{word} missing in {json} ({gaps:?})");
    }
    for word in lacks {
        assert!(!json.contains(word), "{word} present in {json}");
    }
}

#[rstest]
#[case::mandatory(
    "DB$ Play | Defined$ Remembered | WithoutManaCost$ True",
    "key:Play.Optional"
)]
#[case::total_cmc(
    "DB$ Play | Valid$ Card.YouOwn | ValidZone$ Exile | WithTotalCMC$ 10 | Optional$ True | Amount$ All",
    "key:Play.WithTotalCMC"
)]
#[case::copy_outside_exile(
    "DB$ Play | Defined$ Remembered | CopyCard$ True | Optional$ True",
    "key:Play.CopyCard"
)]
#[case::spell_filter(
    "DB$ Play | Defined$ Remembered | ValidSA$ Spell.cmcLEX | Optional$ True",
    "key:Play.ValidSA"
)]
#[case::amount_variable(
    "DB$ Play | Defined$ Remembered | Amount$ X | Optional$ True",
    "key:Play.Amount"
)]
fn play_unknowns_stay_gaps(#[case] line: &str, #[case] gap: &str) {
    let (_, found) = sorcery(&format!("A:SP$ Draw | SubAbility$ P\nSVar:P:{line}"));
    assert!(found.iter().any(|g| g == gap), "{found:?}");
}

#[rstest]
#[case::library_and_graveyard(
    "A:SP$ ChangeZone | Origin$ Library,Graveyard | Destination$ Hand | ChangeType$ Creature.YouOwn | Optional$ True | SpellDescription$ You may search your library and/or graveyard for a creature card.",
    &["\"also\":[\"Graveyard\"]", "Searched"]
)]
#[case::library_hand_graveyard(
    "A:SP$ ChangeZone | Origin$ Graveyard,Hand,Library | Destination$ Battlefield | ChangeType$ Aura.YouOwn | Hidden$ True",
    &["\"also\":[\"Graveyard\",\"Hand\"]"]
)]
fn zone_lists_with_the_library_search(#[case] line: &str, #[case] has: &[&str]) {
    let (json, gaps) = sorcery(line);
    assert!(gaps.is_empty(), "{gaps:?}");
    for word in has {
        assert!(json.contains(word), "{word} missing in {json}");
    }
}

#[rstest]
#[case::stack("Stack", "zone:Origin=Stack")]
#[case::stack_with_battlefield("Battlefield,Stack", "zone:Origin=Battlefield,Stack")]
#[case::command("Command", "zone:Origin=Command")]
#[case::repeated("Graveyard,Graveyard", "zone:Origin=Graveyard,Graveyard")]
fn zones_without_a_node_stay_gaps(#[case] origin: &str, #[case] gap: &str) {
    let (_, found) = sorcery(&format!(
        "A:SP$ ChangeZone | Origin$ {origin} | Destination$ Exile | Defined$ Targeted"
    ));
    assert_eq!(found, [gap]);
}
