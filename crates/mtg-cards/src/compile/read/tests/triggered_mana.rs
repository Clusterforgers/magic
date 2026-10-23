//! Goal: a triggered mana ability (CR 605.1b) reads whole and writes back as the same text, whoever taps (you, any player), what is tapped (a land, a Swamp, the land an Aura enchants) and what is added (a fixed mana, any color, the chosen color, a type the land produced); a wording that adds mana for another player than the one who tapped, or does more than add mana, stays with the table.
use super::*;
use crate::mana_trigger::mana_trigger;

#[rstest]
#[case::any_type_the_land_produced(
    "Whenever you tap a land for mana, add one mana of any type that land produced.",
    r#"{"Triggered":{"when":{"TappedForMana":{"what":[{"Type":"Land"}],"by":"You"}},"effects":[{"AddMana":{"mana":"OfTapped"}}]}}"#
)]
#[case::a_player_taps(
    "Whenever a player taps a land for mana, that player adds one mana of any type that land produced.",
    r#"{"Triggered":{"when":{"TappedForMana":{"what":[{"Type":"Land"}]}},"effects":[{"AddMana":{"who":"Triggering","mana":"OfTapped"}}]}}"#
)]
#[case::an_additional_mana_of_the_land_type(
    "Whenever you tap a Swamp for mana, add an additional {B}.",
    r#"{"Triggered":{"when":{"TappedForMana":{"what":[{"Subtype":"Swamp"}],"by":"You"}},"effects":[{"AddMana":{"mana":{"Fixed":["Black"]}}}]}}"#
)]
#[case::the_enchanted_land(
    "Whenever enchanted land is tapped for mana, its controller adds an additional {G}.",
    r#"{"Triggered":{"when":{"TappedForMana":{"what":[{"Is":"Attached"},{"Type":"Land"}]}},"effects":[{"AddMana":{"who":"Triggering","mana":{"Fixed":["Green"]}}}]}}"#
)]
#[case::two_mana_of_one_type(
    "Whenever enchanted land is tapped for mana, its controller adds an additional {G}{G}.",
    r#"{"Triggered":{"when":{"TappedForMana":{"what":[{"Is":"Attached"},{"Type":"Land"}]}},"effects":[{"AddMana":{"who":"Triggering","mana":{"Fixed":["Green","Green"]}}}]}}"#
)]
#[case::any_color(
    "Whenever enchanted land is tapped for mana, its controller adds an additional one mana of any color.",
    r#"{"Triggered":{"when":{"TappedForMana":{"what":[{"Is":"Attached"},{"Type":"Land"}]}},"effects":[{"AddMana":{"who":"Triggering","mana":{"AnyColor":1}}}]}}"#
)]
#[case::the_chosen_color_of_an_enchanted_forest(
    "Whenever enchanted Forest is tapped for mana, its controller adds an additional one mana of the chosen color.",
    r#"{"Triggered":{"when":{"TappedForMana":{"what":[{"Is":"Attached"},{"Subtype":"Forest"}]}},"effects":[{"AddMana":{"who":"Triggering","mana":{"ChosenColor":1}}}]}}"#
)]
fn the_wordings_of_the_triggered_mana_abilities_read_and_write_back(
    #[case] text: &str,
    #[case] json: &str,
) {
    let read = golden(text, ClauseKind::Triggered, false);
    assert_eq!(read, ability(json), "{text}");
    assert!(mana_trigger(&read).is_some(), "{text}: not a mana ability");
}

#[rstest]
#[case::a_player_adds_for_an_opponents_tap(
    "Whenever an opponent taps a land for mana, add an additional {G}."
)]
#[case::its_controller_of_a_land_that_is_not_enchanted(
    "Whenever a player taps a land for mana, its controller adds an additional {G}."
)]
#[case::that_player_for_the_enchanted_land(
    "Whenever enchanted land is tapped for mana, that player adds an additional {G}."
)]
#[case::the_land_produced_by_an_additional(
    "Whenever you tap a land for mana, add an additional one mana of any type that land produced."
)]
#[case::more_than_mana("Whenever you tap a land for mana, add an additional {G} and draw a card.")]
#[case::the_passive_without_mana(
    "Whenever enchanted land is tapped, its controller adds an additional {G}."
)]
#[case::another_mana(
    "Whenever enchanted land is tapped for mana, its controller adds an additional {G} or {W}."
)]
fn near_misses_of_the_triggered_mana_abilities_stay_with_the_table(#[case] text: &str) {
    let read = clause(text, ClauseKind::Triggered, false);
    let mana = read
        .as_ref()
        .is_some_and(|read| mana_trigger(read).is_some());
    assert!(!mana, "{text}: {read:?}");
}
