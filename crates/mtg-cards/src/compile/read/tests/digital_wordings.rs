//! Goal: conjure and seek are written as the cards word them, for the zones and kinds the wording says, and a shape the wording does not cover is not written.
use super::*;

#[rstest]
#[case::hand(
    r#"{"Spell":{"effects":[{"Conjure":{"name":"Lightning Bolt","zone":"Hand"}}]}}"#,
    "Conjure a card named Lightning Bolt into your hand."
)]
#[case::graveyard_twice(
    r#"{"Spell":{"effects":[{"Conjure":{"name":"Ponder","zone":"Graveyard","count":2}}]}}"#,
    "Conjure two cards named Ponder into your graveyard."
)]
#[case::exile(
    r#"{"Spell":{"effects":[{"Conjure":{"name":"Mox Opal","zone":"Exile"}}]}}"#,
    "Conjure a card named Mox Opal into exile."
)]
#[case::library(
    r#"{"Spell":{"effects":[{"Conjure":{"name":"Island","zone":"Library"}}]}}"#,
    "Conjure a card named Island into your library."
)]
#[case::seek_creature(
    r#"{"Spell":{"effects":[{"Seek":{"filter":[{"Type":"Creature"}]}}]}}"#,
    "Seek a creature card."
)]
#[case::seek_land(
    r#"{"Spell":{"effects":[{"Seek":{"filter":[{"Type":"Land"}]}}]}}"#,
    "Seek a land card."
)]
fn digital_effects_are_written_as_the_cards_word_them(#[case] json: &str, #[case] text: &str) {
    assert_eq!(render_ability(&ability(json)).as_deref(), Some(text));
}

#[rstest]
#[case::seek_two(r#"{"Spell":{"effects":[{"Seek":{"filter":[{"Type":"Land"}],"count":2}}]}}"#)]
#[case::conjure_to_the_battlefield(
    r#"{"Spell":{"effects":[{"Conjure":{"name":"Ponder","zone":"Battlefield"}}]}}"#
)]
fn a_shape_the_wording_does_not_cover_is_not_written(#[case] json: &str) {
    assert_eq!(render_ability(&ability(json)), None);
}
