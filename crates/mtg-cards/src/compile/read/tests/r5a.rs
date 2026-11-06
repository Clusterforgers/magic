//! Goal: a zone move whose selectors name the owner before the zone or an adjective after the card type, whose
//! bindings carry numbers ("kept2") or whose "or" lists card types out of alphabetical order is written as the readers
//! write the same rule, and the Oracle wordings "an additional +1/+1 counter" and "their hand" read back; another
//! zone, owner, count, "may", "up to", tapped state or an inconsistent binding never reads back.
use rstest::rstest;

use super::*;

fn written(json: &str, clause: &str) -> bool {
    written_variants(&ability(json))
        .iter()
        .any(|wording| same_text(wording, clause))
}

/// "Return target <what> to <to>." with the target selector as given.
fn returned(selector: &str, bounds: &str, to: &str) -> String {
    format!(
        r#"{{"Spell":{{"effects":[{{"Move":{{"to":{to},"what":{{"Target":0}}}}}}],"targets":[{{{bounds}"what":{{"Object":[{selector}]}}}}]}}}}"#
    )
}

const OWNER_FIRST: &str = r#"{"Type":"Creature"},{"OwnedBy":"You"},{"In":"Graveyard"}"#;
const TO_HAND: &str = "Return target creature card from your graveyard to your hand.";

#[rstest]
#[case::owner_first(OWNER_FIRST, "", r#""Hand""#, TO_HAND)]
#[case::owner_before_a_value(
    r#"{"Type":"Creature"},{"OwnedBy":"You"},{"Compare":{"op":"Le","stat":"ManaValue","to":3}},{"In":"Graveyard"}"#,
    "",
    r#"{"Battlefield":{}}"#,
    "Return target creature card with mana value 3 or less from your graveyard to the battlefield."
)]
#[case::up_to_two(
    OWNER_FIRST,
    r#""bounds":{"min":0,"max":2},"#,
    r#""Hand""#,
    "Return up to two target creature cards from your graveyard to your hand."
)]
#[case::types_in_order(
    r#"{"AnyOf":[{"Type":"Creature"},{"Type":"Artifact"}]},{"OwnedBy":"You"},{"In":"Graveyard"}"#,
    "",
    r#""Hand""#,
    "Return target artifact or creature card from your graveyard to your hand."
)]
#[case::additional_counter(
    OWNER_FIRST,
    "",
    r#"{"Battlefield":{"counters":[["+1/+1",1]]}}"#,
    "Return target creature card from your graveyard to the battlefield with an additional +1/+1 counter on it."
)]
#[case::two_additional_counters(
    OWNER_FIRST,
    "",
    r#"{"Battlefield":{"counters":[["+1/+1",2]]}}"#,
    "Return target creature card from your graveyard to the battlefield with two additional +1/+1 counters on it."
)]
#[case::their_hand(
    r#"{"OwnedBy":"Opponents"},{"In":"Graveyard"}"#,
    "",
    r#""Hand""#,
    "Return target card from an opponent's graveyard to their hand."
)]
fn a_move_in_another_order_reads_back(
    #[case] selector: &str,
    #[case] bounds: &str,
    #[case] to: &str,
    #[case] clause: &str,
) {
    assert!(written(&returned(selector, bounds, to), clause), "{clause}");
}

#[rstest]
#[case::other_zone(OWNER_FIRST, "", r#"{"Battlefield":{}}"#, TO_HAND)]
#[case::other_owner(
    r#"{"Type":"Creature"},{"OwnedBy":"Opponents"},{"In":"Graveyard"}"#,
    "",
    r#""Hand""#,
    TO_HAND
)]
#[case::up_to_one(OWNER_FIRST, r#""bounds":{"min":0,"max":1},"#, r#""Hand""#, TO_HAND)]
#[case::other_count(
    OWNER_FIRST,
    r#""bounds":{"min":0,"max":3},"#,
    r#""Hand""#,
    "Return up to two target creature cards from your graveyard to your hand."
)]
#[case::other_types(
    r#"{"AnyOf":[{"Type":"Creature"},{"Type":"Enchantment"}]},{"OwnedBy":"You"},{"In":"Graveyard"}"#,
    "",
    r#""Hand""#,
    "Return target artifact or creature card from your graveyard to your hand."
)]
#[case::other_counters(
    OWNER_FIRST,
    "",
    r#"{"Battlefield":{"counters":[["+1/+1",2]]}}"#,
    "Return target creature card from your graveyard to the battlefield with an additional +1/+1 counter on it."
)]
#[case::their_hand_from_yours(
    r#"{"OwnedBy":"You"},{"In":"Graveyard"}"#,
    "",
    r#""Hand""#,
    "Return target card from an opponent's graveyard to their hand."
)]
fn a_move_in_another_order_with_another_meaning_does_not_read_back(
    #[case] selector: &str,
    #[case] bounds: &str,
    #[case] to: &str,
    #[case] clause: &str,
) {
    assert!(
        !written(&returned(selector, bounds, to), clause),
        "{clause}"
    );
}

/// A search for a card the filter names, moved where `to` says, under "you may" when `may`.
fn search(filter: &str, to: &str, may: bool) -> String {
    let inner = format!(
        r#"{{"Search":{{"as":"found","filter":[{filter}]}}}},{{"Move":{{"to":{to},"what":{{"Bound":"found"}}}}}},{{"Shuffle":{{}}}}"#
    );
    let effects = if may {
        format!(r#"{{"May":{{"effects":[{inner}]}}}}"#)
    } else {
        inner
    };
    format!(r#"{{"Spell":{{"effects":[{effects}]}}}}"#)
}

const BASIC_LAND: &str = r#"{"Type":"Land"},{"Supertype":"Basic"}"#;
const TAPPED: &str = r#"{"Battlefield":{"tapped":true}}"#;
const FETCH: &str =
    "Search your library for a basic land card, put it onto the battlefield tapped, then shuffle.";

#[rstest]
#[case::basic_land(BASIC_LAND, TAPPED, false, FETCH, true)]
#[case::black_creature(
    r#"{"Type":"Creature"},{"Color":"Black"}"#,
    r#"{"Battlefield":{}}"#,
    false,
    "Search your library for a black creature card, put it onto the battlefield, then shuffle.",
    true
)]
#[case::may(
    BASIC_LAND,
    TAPPED,
    true,
    "You may search your library for a basic land card, put it onto the battlefield tapped, then shuffle.",
    true
)]
#[case::untapped(BASIC_LAND, r#"{"Battlefield":{}}"#, false, FETCH, false)]
#[case::to_the_hand(BASIC_LAND, r#""Hand""#, false, FETCH, false)]
#[case::any_land(r#"{"Type":"Land"}"#, TAPPED, false, FETCH, false)]
#[case::missing_may(
    BASIC_LAND,
    TAPPED,
    false,
    "You may search your library for a basic land card, put it onto the battlefield tapped, then shuffle.",
    false
)]
fn a_search_with_the_adjective_after_the_type_reads_only_its_own_clause(
    #[case] filter: &str,
    #[case] to: &str,
    #[case] may: bool,
    #[case] clause: &str,
    #[case] reads: bool,
) {
    assert_eq!(written(&search(filter, to, may), clause), reads, "{clause}");
}

/// Look at the top four, take one of them under `kept`, put the rest (`rest`, moved as `moved`) on the bottom.
fn look(kept: &str, rest: &str, moved: &str, order: &str) -> String {
    format!(
        r#"{{"Spell":{{"effects":[{{"LookAtTop":{{"as":"top","count":4}}}},{{"Choose":{{"as":"{kept}","from":{{"Bound":"top"}},"rest_as":"{rest}"}}}},{{"Move":{{"to":"Hand","what":{{"Bound":"{kept}"}}}}}},{{"Move":{{"to":{{"Library":{{"order":"{order}","place":"Bottom"}}}},"what":{{"Bound":"{moved}"}}}}}}]}}}}"#
    )
}

const LOOK: &str = "Look at the top four cards of your library. Put one of them into your hand and the rest on the bottom of your library in a random order.";

#[rstest]
#[case::numbered("kept2", "rest3", "rest3", "Random", true)]
#[case::plain("kept", "rest", "rest", "Random", true)]
#[case::other_order("kept2", "rest3", "rest3", "Chosen", false)]
#[case::moves_another_binding("kept2", "rest3", "rest", "Random", false)]
#[case::moves_the_kept_card("kept2", "rest3", "kept2", "Random", false)]
fn numbered_bindings_read_as_the_names_of_the_readers_only_when_renamed_alike(
    #[case] kept: &str,
    #[case] rest: &str,
    #[case] moved: &str,
    #[case] order: &str,
    #[case] reads: bool,
) {
    assert_eq!(written(&look(kept, rest, moved, order), LOOK), reads);
}
