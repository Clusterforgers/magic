//! Goal: the mana wordings that count a tally of the turn (cards drawn, lands played) or name the permanents controlled read in an activated mana ability, each writes back as the card's text, and a near-miss stays with the table.
use super::*;

#[rstest]
#[case::cards_drawn(
    "{T}: Add {C} for each card you've drawn this turn.",
    r#"{"Activated":{"cost":["Tap"],"effects":[{"AddMana":{"mana":{"Fixed":["Colorless"]},"times":{"Tallied":{"who":"You","what":"Drawn"}}}}]}}"#
)]
#[case::a_land_was_played(
    "{T}: Add {U}. If you played a land this turn, add {B} instead.",
    r#"{"Activated":{"cost":["Tap"],"effects":[{"AddMana":{"mana":{"Instead":{"base":{"Fixed":["Blue"]},"when":{"Compare":{"left":{"Tallied":{"who":"You","what":"LandsPlayed"}},"op":"Ge","right":1}},"instead":{"Fixed":["Black"]}}}}}]}}"#
)]
#[case::two_named_lands(
    "{T}: Add {C}. If you control an Urza's Power-Plant and an Urza's Tower, add {C}{C} instead.",
    r#"{"Activated":{"cost":["Tap"],"effects":[{"AddMana":{"mana":{"Instead":{"base":{"Fixed":["Colorless"]},"when":{"All":[{"Compare":{"left":{"Count":[{"Subtype":"Urza's Power-Plant"},{"ControlledBy":"You"}]},"op":"Ge","right":1}},{"Compare":{"left":{"Count":[{"Subtype":"Urza's Tower"},{"ControlledBy":"You"}]},"op":"Ge","right":1}}]},"instead":{"Fixed":["Colorless","Colorless"]}}}}}]}}"#
)]
fn tallied_mana_and_named_lands_read_and_write_back(#[case] text: &str, #[case] json: &str) {
    reads_as(text, ClauseKind::Activated, false, json);
}

#[rstest]
#[case::drawn_by_an_opponent("{T}: Add {C} for each card an opponent has drawn this turn.")]
#[case::lands_by_a_player("{T}: Add {U}. If a player played a land this turn, add {B} instead.")]
#[case::no_condition_word("{T}: Add {U}. You played a land this turn, add {B} instead.")]
fn near_misses_stay_with_the_table(#[case] text: &str) {
    assert!(
        clause(text, ClauseKind::Activated, false).is_none(),
        "{text}"
    );
}

#[rstest]
#[case::the_number_of_cards_drawn(
    "Creatures you control get +X/+X, where X is the number of cards you've drawn this turn.",
    ClauseKind::Static,
    r#"{"Static":{"affected":{"Each":[{"Type":"Creature"},{"ControlledBy":"You"}]},"mods":[{"ModifyPT":{"power":{"Tallied":{"who":"You","what":"Drawn"}},"toughness":{"Tallied":{"who":"You","what":"Drawn"}}}}]}}"#
)]
#[case::a_bonus_for_each_card_drawn(
    "Draw a card. Until end of turn, target creature gains trample and gets +1/+0 for each card you've drawn this turn.",
    ClauseKind::Spell,
    r#"{"Spell":{"targets":[{"what":{"Object":[{"Type":"Creature"}]}}],"effects":[{"Draw":{}},{"Continuous":{"affected":{"Target":0},"mods":[{"AddKeyword":"Trample"},{"ModifyPT":{"power":{"Tallied":{"who":"You","what":"Drawn"}},"toughness":0}}],"duration":"UntilEndOfTurn"}}]}}"#
)]
fn amounts_of_cards_drawn_read_and_write_back(
    #[case] text: &str,
    #[case] kind: ClauseKind,
    #[case] json: &str,
) {
    reads_as(text, kind, false, json);
}
