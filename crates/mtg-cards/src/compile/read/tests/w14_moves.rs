//! Goal: the choice of a card or permanent and its move, as the translator binds it ("picked", numbered names, zones in any order, "up to one" as the player's option), reads back as "return a creature you control to its owner's hand", "put a land card from your hand onto the battlefield", "return a creature card from your graveyard to your hand" and "put a card from your hand on top of your library", and a rule of another zone, owner, count, option, tapped state or kind never reads back as the clause.
use super::*;

/// Whether some wording the checker accepts for the rules is the clause.
fn written(json: &str, clause: &str) -> bool {
    written_variants(&ability(json))
        .iter()
        .any(|wording| same_text(wording, clause))
}

fn spell(from: &str, count: &str, name: &str, to: &str) -> String {
    format!(
        r#"{{"Spell":{{"effects":[{{"Choose":{{"as":"{name}","from":{{"Each":{from}}}{count}}}}},{{"Move":{{"what":{{"Bound":"{name}"}},"to":{to}}}}}]}}}}"#
    )
}

const OPTIONAL: &str = r#","count":{"max":1,"min":0}"#;
const TWO: &str = r#","count":{"max":2,"min":2}"#;
const UP_TO_TWO: &str = r#","count":{"max":2,"min":0}"#;
const THREE: &str = r#","count":{"max":3,"min":3}"#;
const MINE: &str = r#"[{"Type":"Creature"},{"ControlledBy":"You"}]"#;
const THEIRS: &str = r#"[{"Type":"Creature"},{"ControlledBy":"Opponents"}]"#;
const OTHER: &str = r#"["Other",{"Type":"Creature"},{"ControlledBy":"You"}]"#;
const HAND: &str = r#"[{"Type":"Land"},{"In":"Hand"},{"OwnedBy":"You"}]"#;
const THEIR_HAND: &str = r#"[{"Type":"Land"},{"In":"Hand"},{"OwnedBy":"Opponents"}]"#;
const YARD: &str = r#"[{"Type":"Creature"},{"In":"Graveyard"},{"OwnedBy":"You"}]"#;
const THEIR_YARD: &str = r#"[{"Type":"Creature"},{"In":"Graveyard"},{"OwnedBy":"Opponents"}]"#;
const ZONE_FIRST: &str = r#"[{"In":"Hand"},{"OwnedBy":"You"},{"Type":"Land"}]"#;
const BF: &str = r#"{"Battlefield":{}}"#;
const BF_TAPPED: &str = r#"{"Battlefield":{"tapped":true}}"#;
const TOP: &str = r#"{"Library":{"place":"Top"}}"#;

#[rstest]
#[case::bounce(
    spell(MINE, "", "picked", r#""Hand""#),
    "Return a creature you control to its owner's hand.",
    true
)]
#[case::numbered_name(
    spell(MINE, "", "picked2", r#""Hand""#),
    "Return a creature you control to its owner's hand.",
    true
)]
#[case::another(
    spell(OTHER, "", "picked", r#""Hand""#),
    "Return another creature you control to its owner's hand.",
    true
)]
#[case::an_opponent_s(
    spell(THEIRS, "", "picked", r#""Hand""#),
    "Return a creature you control to its owner's hand.",
    false
)]
#[case::to_the_graveyard(
    spell(MINE, "", "picked", r#""Graveyard""#),
    "Return a creature you control to its owner's hand.",
    false
)]
#[case::on_the_battlefield(
    spell(MINE, "", "picked", BF),
    "Return a creature you control to its owner's hand.",
    false
)]
#[case::optional_is_not_forced(
    spell(MINE, OPTIONAL, "picked", r#""Hand""#),
    "Return a creature you control to its owner's hand.",
    false
)]
#[case::two_is_not_one(
    spell(MINE, TWO, "picked", r#""Hand""#),
    "Return a creature you control to its owner's hand.",
    false
)]
#[case::another_is_not_a(
    spell(MINE, "", "picked", r#""Hand""#),
    "Return another creature you control to its owner's hand.",
    false
)]
#[case::two(
    spell(MINE, TWO, "picked", r#""Hand""#),
    "Return two creatures you control to their owner's hand.",
    true
)]
#[case::up_to_two(
    spell(MINE, UP_TO_TWO, "picked", r#""Hand""#),
    "Return up to two creatures you control to their owner's hand.",
    true
)]
#[case::three_for_two(
    spell(MINE, THREE, "picked", r#""Hand""#),
    "Return two creatures you control to their owner's hand.",
    false
)]
#[case::two_for_up_to_two(
    spell(MINE, TWO, "picked", r#""Hand""#),
    "Return up to two creatures you control to their owner's hand.",
    false
)]
#[case::two_with_its(
    spell(MINE, TWO, "picked", r#""Hand""#),
    "Return two creatures you control to its owner's hand.",
    false
)]
fn w14_bounce_of_a_chosen_permanent(
    #[case] json: String,
    #[case] clause: &str,
    #[case] reads: bool,
) {
    assert_eq!(written(&json, clause), reads, "{json}");
}

#[rstest]
#[case::put_a_land(
    spell(HAND, "", "picked", BF),
    "Put a land card from your hand onto the battlefield.",
    true
)]
#[case::tapped(
    spell(HAND, "", "picked", BF_TAPPED),
    "Put a land card from your hand onto the battlefield tapped.",
    true
)]
#[case::you_may(
    spell(HAND, OPTIONAL, "picked", BF),
    "You may put a land card from your hand onto the battlefield.",
    true
)]
#[case::zone_first(
    spell(ZONE_FIRST, OPTIONAL, "picked", BF),
    "You may put a land card from your hand onto the battlefield.",
    true
)]
#[case::missing_may(
    spell(HAND, "", "picked", BF),
    "You may put a land card from your hand onto the battlefield.",
    false
)]
#[case::invented_may(
    spell(HAND, OPTIONAL, "picked", BF),
    "Put a land card from your hand onto the battlefield.",
    false
)]
#[case::untapped_rule(
    spell(HAND, OPTIONAL, "picked", BF),
    "You may put a land card from your hand onto the battlefield tapped.",
    false
)]
#[case::tapped_rule(
    spell(HAND, OPTIONAL, "picked", BF_TAPPED),
    "You may put a land card from your hand onto the battlefield.",
    false
)]
#[case::other_player_s_hand(
    spell(THEIR_HAND, OPTIONAL, "picked", BF),
    "You may put a land card from your hand onto the battlefield.",
    false
)]
#[case::up_to_two(
    spell(HAND, r#","count":{"max":2,"min":0}"#, "picked", BF),
    "You may put a land card from your hand onto the battlefield.",
    false
)]
#[case::to_hand_instead(
    spell(HAND, OPTIONAL, "picked", r#""Hand""#),
    "You may put a land card from your hand onto the battlefield.",
    false
)]
#[case::any_number(
    spell(HAND, r#","count":{"max":null,"min":0}"#, "picked", BF),
    "Put any number of land cards from your hand onto the battlefield.",
    true
)]
#[case::any_number_for_one(
    spell(HAND, r#","count":{"max":null,"min":0}"#, "picked", BF),
    "Put a land card from your hand onto the battlefield.",
    false
)]
fn w14_put_onto_the_battlefield(#[case] json: String, #[case] clause: &str, #[case] reads: bool) {
    assert_eq!(written(&json, clause), reads, "{json}");
}

#[rstest]
#[case::to_hand(
    spell(YARD, "", "picked", r#""Hand""#),
    "Return a creature card from your graveyard to your hand.",
    true
)]
#[case::you_may(
    spell(YARD, OPTIONAL, "picked", r#""Hand""#),
    "You may return a creature card from your graveyard to your hand.",
    true
)]
#[case::to_the_battlefield(
    spell(YARD, OPTIONAL, "picked", BF),
    "You may return a creature card from your graveyard to the battlefield.",
    true
)]
#[case::tapped(
    spell(YARD, "", "picked", BF_TAPPED),
    "Return a creature card from your graveyard to the battlefield tapped.",
    true
)]
#[case::their_graveyard(
    spell(THEIR_YARD, "", "picked", r#""Hand""#),
    "Return a creature card from your graveyard to your hand.",
    false
)]
#[case::missing_may(
    spell(YARD, "", "picked", r#""Hand""#),
    "You may return a creature card from your graveyard to your hand.",
    false
)]
#[case::invented_may(
    spell(YARD, OPTIONAL, "picked", r#""Hand""#),
    "Return a creature card from your graveyard to your hand.",
    false
)]
#[case::hand_for_battlefield(
    spell(YARD, "", "picked", r#""Hand""#),
    "Return a creature card from your graveyard to the battlefield.",
    false
)]
#[case::untapped_for_tapped(
    spell(YARD, "", "picked", BF),
    "Return a creature card from your graveyard to the battlefield tapped.",
    false
)]
#[case::two_cards(
    spell(YARD, TWO, "picked", r#""Hand""#),
    "Return a creature card from your graveyard to your hand.",
    false
)]
#[case::up_to_two(
    spell(YARD, UP_TO_TWO, "picked", r#""Hand""#),
    "Return up to two creature cards from your graveyard to your hand.",
    true
)]
#[case::exactly_two(
    spell(YARD, TWO, "picked", BF),
    "Return two creature cards from your graveyard to the battlefield.",
    true
)]
#[case::up_to_two_for_one(
    spell(YARD, UP_TO_TWO, "picked", r#""Hand""#),
    "Return a creature card from your graveyard to your hand.",
    false
)]
#[case::three_for_two(
    spell(YARD, THREE, "picked", r#""Hand""#),
    "Return up to two creature cards from your graveyard to your hand.",
    false
)]
fn w14_return_from_the_graveyard(#[case] json: String, #[case] clause: &str, #[case] reads: bool) {
    assert_eq!(written(&json, clause), reads, "{json}");
}

const CARD_OF_HAND: &str = r#"[{"In":"Hand"},{"OwnedBy":"You"}]"#;
const CARD_OF_THEIR_HAND: &str = r#"[{"In":"Hand"},{"OwnedBy":"Opponents"}]"#;
const BOTTOM: &str = r#"{"Library":{"place":"Bottom"}}"#;

#[rstest]
#[case::on_top(
    spell(CARD_OF_HAND, "", "picked", TOP),
    "Put a card from your hand on top of your library.",
    true
)]
#[case::numbered(
    spell(CARD_OF_HAND, "", "picked3", TOP),
    "Put a card from your hand on top of your library.",
    true
)]
#[case::their_hand(
    spell(CARD_OF_THEIR_HAND, "", "picked", TOP),
    "Put a card from your hand on top of your library.",
    false
)]
#[case::bottom_for_top(
    spell(CARD_OF_HAND, "", "picked", BOTTOM),
    "Put a card from your hand on top of your library.",
    false
)]
#[case::optional(
    spell(CARD_OF_HAND, OPTIONAL, "picked", TOP),
    "Put a card from your hand on top of your library.",
    false
)]
fn w14_put_back_a_card_of_the_hand(
    #[case] json: String,
    #[case] clause: &str,
    #[case] reads: bool,
) {
    assert_eq!(written(&json, clause), reads, "{json}");
}

#[rstest]
#[case::then_a_sentence(
    r#"{"Spell":{"effects":[{"Mill":{"count":2}},{"Choose":{"as":"picked","from":{"Each":[{"Type":"Creature"},{"In":"Graveyard"},{"OwnedBy":"You"}]},"count":{"max":1,"min":0}}},{"Move":{"what":{"Bound":"picked"},"to":"Hand"}}]}}"#,
    "Mill two cards. Then you may return a creature card from your graveyard to your hand.",
    true
)]
#[case::then_a_comma(
    r#"{"Spell":{"effects":[{"Mill":{"count":2}},{"Choose":{"as":"picked","from":{"Each":[{"Type":"Creature"},{"In":"Graveyard"},{"OwnedBy":"You"}]},"count":{"max":1,"min":0}}},{"Move":{"what":{"Bound":"picked"},"to":"Hand"}}]}}"#,
    "Mill two cards, then you may return a creature card from your graveyard to your hand.",
    true
)]
#[case::forced_for_may(
    r#"{"Spell":{"effects":[{"Mill":{"count":2}},{"Choose":{"as":"picked","from":{"Each":[{"Type":"Creature"},{"In":"Graveyard"},{"OwnedBy":"You"}]}}},{"Move":{"what":{"Bound":"picked"},"to":"Hand"}}]}}"#,
    "Mill two cards. Then you may return a creature card from your graveyard to your hand.",
    false
)]
#[case::three_for_two(
    r#"{"Spell":{"effects":[{"Mill":{"count":3}},{"Choose":{"as":"picked","from":{"Each":[{"Type":"Creature"},{"In":"Graveyard"},{"OwnedBy":"You"}]},"count":{"max":1,"min":0}}},{"Move":{"what":{"Bound":"picked"},"to":"Hand"}}]}}"#,
    "Mill two cards. Then you may return a creature card from your graveyard to your hand.",
    false
)]
fn w14_mill_then_return(#[case] json: &str, #[case] clause: &str, #[case] reads: bool) {
    assert_eq!(written(json, clause), reads, "{json}");
}

fn guarded(condition: &str, then: &str) -> String {
    format!(r#"{{"Spell":{{"effects":[{{"If":{{"condition":{condition},"then":[{then}]}}}}]}}}}"#)
}

fn looked(top: u32, name: &str, count: &str, kind: &str, to: &str, rest: &str) -> String {
    format!(
        r#"{{"Triggered":{{"when":{{"ZoneChange":{{"to":"Battlefield","what":["This"]}}}},"effects":[{{"LookAtTop":{{"as":"top","count":{top}}}}},{{"Choose":{{"as":"{name}"{count},"from":{{"Each":[{{"In":"Library"}},{{"Is":{{"Bound":"top"}}}},{{"Type":"{kind}"}}]}}}}}},{{"Move":{{"to":{to},"what":{{"Bound":"{name}"}}}}}},{{"Move":{{"to":{rest},"what":{{"Bound":"top"}}}}}}]}}}}"#
    )
}

const ROOT_BOTTOM: &str = r#"{"Library":{"order":"Random","place":"Bottom"}}"#;
const ROOT_TOP: &str = r#"{"Library":{"order":"Random","place":"Top"}}"#;
const LOOK_FIVE: &str = "When this creature enters, look at the top five cards of your library. You may put a creature card from among them onto the battlefield. Put the rest on the bottom of your library in a random order.";

#[rstest]
#[case::numbered_name(
    looked(5, "kept2", OPTIONAL, "Creature", BF, ROOT_BOTTOM),
    LOOK_FIVE,
    true
)]
#[case::plain_name(
    looked(5, "kept", OPTIONAL, "Creature", BF, ROOT_BOTTOM),
    LOOK_FIVE,
    true
)]
#[case::four_cards(
    looked(4, "kept2", OPTIONAL, "Creature", BF, ROOT_BOTTOM),
    LOOK_FIVE,
    false
)]
#[case::a_land(
    looked(5, "kept2", OPTIONAL, "Land", BF, ROOT_BOTTOM),
    LOOK_FIVE,
    false
)]
#[case::to_the_hand(
    looked(5, "kept2", OPTIONAL, "Creature", r#""Hand""#, ROOT_BOTTOM),
    LOOK_FIVE,
    false
)]
#[case::rest_on_top(
    looked(5, "kept2", OPTIONAL, "Creature", BF, ROOT_TOP),
    LOOK_FIVE,
    false
)]
#[case::forced(looked(5, "kept2", "", "Creature", BF, ROOT_BOTTOM), LOOK_FIVE, false)]
fn w14_look_at_the_top_and_put_one_onto_the_battlefield(
    #[case] json: String,
    #[case] clause: &str,
    #[case] reads: bool,
) {
    assert_eq!(written(&json, clause), reads, "{json}");
}

#[rstest]
#[case::leading(
    guarded(r#"{"Compare":{"left":{"Count":[{"Type":"Creature"},{"ControlledBy":"You"}]},"op":"Ge","right":2}}"#, r#"{"Draw":{"count":1}}"#),
    "If you control two or more creatures, draw a card.",
    true
)]
#[case::trailing(
    guarded(r#"{"Compare":{"left":{"Count":[{"Type":"Creature"},{"ControlledBy":"You"}]},"op":"Ge","right":2}}"#, r#"{"Draw":{"count":1}}"#),
    "Draw a card if you control two or more creatures.",
    true
)]
#[case::other_count(
    guarded(r#"{"Compare":{"left":{"Count":[{"Type":"Creature"},{"ControlledBy":"You"}]},"op":"Ge","right":3}}"#, r#"{"Draw":{"count":1}}"#),
    "Draw a card if you control two or more creatures.",
    false
)]
#[case::other_effect(
    guarded(r#"{"Compare":{"left":{"Count":[{"Type":"Creature"},{"ControlledBy":"You"}]},"op":"Ge","right":2}}"#, r#"{"Draw":{"count":2}}"#),
    "Draw a card if you control two or more creatures.",
    false
)]
#[case::other_owner(
    guarded(r#"{"Compare":{"left":{"Count":[{"Type":"Creature"},{"ControlledBy":"Opponents"}]},"op":"Ge","right":2}}"#, r#"{"Draw":{"count":1}}"#),
    "Draw a card if you control two or more creatures.",
    false
)]
fn w14_a_condition_before_or_after_its_effect(
    #[case] json: String,
    #[case] clause: &str,
    #[case] reads: bool,
) {
    assert_eq!(written(&json, clause), reads, "{json}");
}

fn grazer(count: &str, to: &str) -> String {
    format!(
        r#"{{"Triggered":{{"when":{{"ZoneChange":{{"what":["This"],"to":"Battlefield"}}}},"effects":[{{"May":{{"effects":[{{"Choose":{{"from":{{"Each":[{{"In":"Hand"}},{{"OwnedBy":"You"}},{{"Type":"Land"}}]}},"as":"land"{count}}}}},{{"Move":{{"what":{{"Bound":"land"}},"to":{to}}}}}]}}}}]}}}}"#
    )
}

const GRAZER_TEXT: &str = "When this creature enters, you may put a land card from your hand onto the battlefield tapped.";

#[rstest]
#[case::pick_and_option(grazer(OPTIONAL, BF_TAPPED), GRAZER_TEXT, true)]
#[case::option_only(grazer("", BF_TAPPED), GRAZER_TEXT, true)]
#[case::untapped_rule(grazer(OPTIONAL, BF), GRAZER_TEXT, false)]
#[case::no_option_in_text(
    grazer(OPTIONAL, BF_TAPPED),
    "When this creature enters, put a land card from your hand onto the battlefield tapped.",
    false
)]
#[case::to_hand(grazer(OPTIONAL, r#""Hand""#), GRAZER_TEXT, false)]
fn w14_an_option_and_a_pick_that_may_take_none(
    #[case] json: String,
    #[case] clause: &str,
    #[case] reads: bool,
) {
    assert_eq!(written(&json, clause), reads, "{json}");
}
