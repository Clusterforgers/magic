//! Goal: revealing from the top of the library until a card matches reads with every filter the cards print and every place the card found and the rest go, alone or after another effect or under a cost, writes back as the same text, and leaves the wordings of another count, player or place unread.
use super::*;

fn passage(filter: &str, tail: &str) -> String {
    format!(
        r#"{{"Spell":{{"effects":[{{"RevealUntil":{{"filter":{filter},"as":"found","revealed_as":"revealed"}}}}{tail}]}}}}"#
    )
}

fn moves(found: &str, rest: &str) -> String {
    format!(
        r#",{{"Move":{{"what":{{"Bound":"found"}},"to":{found}}}}},{{"Move":{{"what":{{"Bound":"revealed"}},"to":{rest}}}}}"#
    )
}

const BOTTOM: &str = r#"{"Library":{"place":"Bottom","order":"Random"}}"#;

#[rstest]
#[case::a_creature_to_the_hand(
    "Reveal cards from the top of your library until you reveal a creature card. Put that card into your hand and the rest on the bottom of your library in a random order.",
    passage(r#"[{"Type":"Creature"}]"#, &moves(r#""Hand""#, BOTTOM))
)]
#[case::a_land_to_the_battlefield_tapped(
    "Reveal cards from the top of your library until you reveal a land card. Put that card onto the battlefield tapped and the rest on the bottom of your library in a random order.",
    passage(r#"[{"Type":"Land"}]"#, &moves(r#"{"Battlefield":{"tapped":true}}"#, BOTTOM))
)]
#[case::two_types(
    "Reveal cards from the top of your library until you reveal a creature or planeswalker card. Put that card onto the battlefield and the rest on the bottom of your library in a random order.",
    passage(r#"[{"AnyOf":[{"Type":"Creature"},{"Type":"Planeswalker"}]}]"#, &moves(r#"{"Battlefield":{}}"#, BOTTOM))
)]
#[case::an_artifact(
    "Reveal cards from the top of your library until you reveal an artifact card. Put that card into your hand and the rest on the bottom of your library in a random order.",
    passage(r#"[{"Type":"Artifact"}]"#, &moves(r#""Hand""#, BOTTOM))
)]
#[case::the_rest_to_the_graveyard(
    "Reveal cards from the top of your library until you reveal a creature card. Put that card into your hand and the rest into your graveyard.",
    passage(r#"[{"Type":"Creature"}]"#, &moves(r#""Hand""#, r#""Graveyard""#))
)]
#[case::in_any_order(
    "Reveal cards from the top of your library until you reveal a nonland card. Put that card into your hand and the rest on the bottom of your library in any order.",
    passage(r#"[{"Not":{"Type":"Land"}}]"#, &moves(r#""Hand""#, r#"{"Library":{"place":"Bottom","order":"Chosen"}}"#))
)]
#[case::with_a_mana_value(
    "Reveal cards from the top of your library until you reveal a nonland card with mana value 3 or less. Put that card into your hand and the rest on the bottom of your library in a random order.",
    passage(r#"[{"Not":{"Type":"Land"}},{"Compare":{"stat":"ManaValue","op":"Le","to":3}}]"#, &moves(r#""Hand""#, BOTTOM))
)]
#[case::all_of_them(
    "Reveal cards from the top of your library until you reveal a nonland card, then put all cards revealed this way into your hand.",
    passage(
        r#"[{"Not":{"Type":"Land"}}]"#,
        r#",{"Move":{"what":{"Bound":"revealed"},"to":"Hand"}}"#
    )
)]
#[case::nothing_after_it(
    "Reveal cards from the top of your library until you reveal a creature card.",
    passage(r#"[{"Type":"Creature"}]"#, "")
)]
fn revealing_until_a_card_reads_in_the_shape_of_the_hand_written_files(
    #[case] text: &str,
    #[case] json: String,
) {
    reads_as(text, ClauseKind::Spell, false, &json);
}

#[rstest]
#[case::entering(
    "When ~ enters, reveal cards from the top of your library until you reveal a land card. Put that card onto the battlefield tapped and the rest on the bottom of your library in a random order.",
    ClauseKind::Triggered
)]
#[case::attacking(
    "Whenever ~ attacks, reveal cards from the top of your library until you reveal a creature card. Put that card into your hand and the rest into your graveyard.",
    ClauseKind::Triggered
)]
#[case::at_the_upkeep(
    "At the beginning of your upkeep, reveal cards from the top of your library until you reveal a creature or planeswalker card. Put that card onto the battlefield and the rest on the bottom of your library in a random order.",
    ClauseKind::Triggered
)]
#[case::sacrificing(
    "{G}, Sacrifice a creature: Reveal cards from the top of your library until you reveal a creature card. Put that card into your hand and the rest on the bottom of your library in a random order.",
    ClauseKind::Activated
)]
fn the_passage_under_a_trigger_or_a_cost_reads_and_writes_back(
    #[case] text: &str,
    #[case] kind: ClauseKind,
) {
    golden(text, kind, false);
}

#[rstest]
#[case::another_count(
    "Reveal cards from the top of your library until you reveal two creature cards. Put those cards into your hand and the rest on the bottom of your library in a random order."
)]
#[case::the_card_on_top(
    "Reveal cards from the top of your library until you reveal a creature card. Put that card on top of your library and the rest on the bottom of your library in a random order."
)]
#[case::another_library(
    "Reveal cards from the top of their library until you reveal a creature card. Put that card into your hand and the rest on the bottom of your library in a random order."
)]
#[case::the_exile_of_it(
    "Reveal cards from the top of your library until you reveal a creature card. Exile that card and the rest on the bottom of your library in a random order."
)]
fn a_wording_of_another_count_player_or_place_does_not_read(#[case] text: &str) {
    assert_eq!(clause(text, ClauseKind::Spell, false), None, "{text}");
}

#[rstest]
#[case::lands_tapped(
    "Reveal cards from the top of your library until you reveal X land cards. Put those land cards onto the battlefield tapped and the rest on the bottom of your library in a random order.",
    r#"[{"Type":"Land"}]"#,
    r#"{"Battlefield":{"tapped":true}}"#
)]
#[case::creatures_to_the_hand(
    "Reveal cards from the top of your library until you reveal X creature cards. Put those creature cards into your hand and the rest on the bottom of your library in a random order.",
    r#"[{"Type":"Creature"}]"#,
    r#""Hand""#
)]
fn revealing_until_x_cards_reads_with_those_cards_and_writes_back(
    #[case] text: &str,
    #[case] filter: &str,
    #[case] found: &str,
) {
    let json = format!(
        r#"{{"Spell":{{"effects":[{{"RevealUntil":{{"filter":{filter},"count":"X","as":"found","revealed_as":"revealed"}}}}{}]}}}}"#,
        moves(found, BOTTOM)
    );
    reads_as(text, ClauseKind::Spell, true, &json);
}

#[test]
fn a_card_that_may_be_put_onto_the_battlefield_leaves_the_rest_of_the_cards_revealed_below() {
    let text = "Reveal cards from the top of your library until you reveal a creature card. You may put that card onto the battlefield. Then put all cards revealed this way that weren't put onto the battlefield on the bottom of your library in a random order.";
    let json = format!(
        r#"{{"Spell":{{"effects":[{{"RevealUntil":{{"filter":[{{"Type":"Creature"}}],"as":"found","revealed_as":"revealed"}}}},{{"May":{{"effects":[{{"Move":{{"what":{{"Bound":"found"}},"to":{{"Battlefield":{{}}}}}}}}]}}}},{{"Move":{{"what":{{"Bound":"revealed"}},"to":{BOTTOM}}}}}]}}}}"#
    );
    reads_as(text, ClauseKind::Spell, false, &json);
}

#[rstest]
#[case::x_without_the_x_of_the_spell(
    "Reveal cards from the top of your library until you reveal X creature cards. Put those creature cards into your hand and the rest on the bottom of your library in a random order.",
    false
)]
#[case::those_cards_of_another_kind(
    "Reveal cards from the top of your library until you reveal X land cards. Put those creature cards onto the battlefield and the rest on the bottom of your library in a random order.",
    true
)]
#[case::x_cards_all_into_the_hand(
    "Reveal cards from the top of your library until you reveal X land cards, then put all cards revealed this way into your hand.",
    true
)]
fn x_cards_with_a_tail_that_does_not_fit_stay_unread(#[case] text: &str, #[case] x_free: bool) {
    assert_eq!(clause(text, ClauseKind::Spell, x_free), None, "{text}");
}
