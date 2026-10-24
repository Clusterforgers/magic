//! Goal: the writers of the proposal checker give the oracle wording of the effects of loyalty abilities that no template reads (a counter on "up to one target creature", a look at the top of a target player's library, the exile of a library and the shuffle of a hand, a search whose card is called by its type, a group that becomes creatures), and a wrong player, place, count, size or type never reads back.
use super::*;

/// Whether some wording the checker accepts for the rules is the clause.
fn written(json: &str, clause: &str) -> bool {
    written_variants(&ability(json))
        .iter()
        .any(|wording| same_text(wording, clause))
}

fn loyalty(cost: &str, targets: &str, effects: &str) -> String {
    format!(r#"{{"Loyalty":{{"cost":{cost},"targets":{targets},"effects":{effects}}}}}"#)
}

fn counter_on_up_to_one(max: u32, kind: &str) -> String {
    loyalty(
        r#"{"Plus":1}"#,
        &format!(
            r#"[{{"what":{{"Object":[{{"Type":"{kind}"}}]}},"bounds":{{"min":0,"max":{max}}}}}]"#
        ),
        r#"[{"PutCounters":{"on":{"Target":0},"counter":"+1/+1"}}]"#,
    )
}

#[rstest]
#[case::a_creature(
    counter_on_up_to_one(1, "Creature"),
    "+1: Put a +1/+1 counter on up to one target creature."
)]
#[case::an_artifact(
    counter_on_up_to_one(1, "Artifact"),
    "+1: Put a +1/+1 counter on up to one target artifact."
)]
fn a_counter_goes_on_up_to_one_target(#[case] json: String, #[case] clause: &str) {
    assert!(written(&json, clause), "{clause}");
}

#[rstest]
#[case::exactly_one("+1: Put a +1/+1 counter on target creature.")]
#[case::another_kind("+1: Put a +1/+1 counter on up to one target artifact.")]
#[case::another_counter("+1: Put a -1/-1 counter on up to one target creature.")]
#[case::up_to_two("+1: Put a +1/+1 counter on up to two target creatures.")]
fn a_counter_on_up_to_one_target_does_not_read_as_another_target(#[case] clause: &str) {
    assert!(
        !written(&counter_on_up_to_one(1, "Creature"), clause),
        "{clause}"
    );
}

fn peek(place: &str, count: u32, who: &str) -> String {
    loyalty(
        r#"{"Plus":2}"#,
        r#"[{"what":"Player"}]"#,
        &format!(
            r#"[{{"LookAtTop":{{"who":{who},"count":{count},"as":"top"}}}},{{"May":{{"effects":[{{"Move":{{"what":{{"Bound":"top"}},"to":{{"Library":{{"place":"{place}"}}}}}}}}]}}}}]"#
        ),
    )
}

#[rstest]
#[case::the_bottom(
    "Bottom",
    1,
    "+2: Look at the top card of target player's library. You may put that card on the bottom of that player's library."
)]
#[case::the_top(
    "Top",
    1,
    "+2: Look at the top card of target player's library. You may put that card on top of that player's library."
)]
#[case::three_cards(
    "Bottom",
    3,
    "+2: Look at the top three cards of target player's library. You may put that card on the bottom of that player's library."
)]
fn a_look_at_the_library_of_a_target_player_is_written_with_its_choice(
    #[case] place: &str,
    #[case] count: u32,
    #[case] clause: &str,
) {
    assert!(
        written(&peek(place, count, r#"{"Target":0}"#), clause),
        "{clause}"
    );
}

#[rstest]
#[case::your_library(
    "+2: Look at the top card of your library. You may put that card on the bottom of that player's library."
)]
#[case::the_other_place(
    "+2: Look at the top card of target player's library. You may put that card on top of that player's library."
)]
#[case::no_choice("+2: Look at the top card of target player's library.")]
#[case::a_creature_target(
    "+2: Look at the top card of target creature's library. You may put that card on the bottom of that player's library."
)]
fn a_look_at_the_library_of_a_target_player_does_not_read_as_another_place(#[case] clause: &str) {
    assert!(
        !written(&peek("Bottom", 1, r#"{"Target":0}"#), clause),
        "{clause}"
    );
}

#[test]
fn a_look_at_a_library_that_is_not_the_target_players_is_not_written_with_one() {
    assert!(!written(
        &peek("Bottom", 1, r#""You""#),
        "+2: Look at the top card of target player's library. You may put that card on the bottom of that player's library."
    ));
}

const THEIR_LIBRARY: &str =
    r#"{"Exile":{"what":{"Each":[{"In":"Library"},{"OwnedBy":{"Target":0}}]}}}"#;
const THEIR_HAND: &str = r#"{"Move":{"what":{"Each":[{"In":"Hand"},{"OwnedBy":{"Target":0}}]},"to":{"Library":{"place":"Shuffled"}}}}"#;

#[rstest]
#[case::both(format!("[{THEIR_LIBRARY},{THEIR_HAND}]"), "\u{2212}12: Exile all cards from target player's library, then that player shuffles their hand into their library.")]
#[case::the_exile(format!("[{THEIR_LIBRARY}]"), "\u{2212}12: Exile all cards from target player's library.")]
#[case::the_shuffle(format!("[{THEIR_HAND}]"), "\u{2212}12: Target player shuffles their hand into their library.")]
fn a_library_exiled_and_a_hand_shuffled_are_written_for_the_target_player(
    #[case] effects: String,
    #[case] clause: &str,
) {
    let json = loyalty(r#"{"Minus":12}"#, r#"[{"what":"Player"}]"#, &effects);
    assert!(written(&json, clause), "{clause}");
}

#[rstest]
#[case::your_library(
    "\u{2212}12: Exile all cards from your library, then that player shuffles their hand into their library."
)]
#[case::a_graveyard(
    "\u{2212}12: Exile all cards from target player's graveyard, then that player shuffles their hand into their library."
)]
#[case::the_other_order(
    "\u{2212}12: That player shuffles their hand into their library, then exile all cards from target player's library."
)]
#[case::the_graveyard_shuffled(
    "\u{2212}12: Exile all cards from target player's library, then that player shuffles their graveyard into their library."
)]
#[case::the_player_of_each(
    "\u{2212}12: Exile all cards from target player's library, then each player shuffles their hand into their library."
)]
fn a_library_exiled_and_a_hand_shuffled_do_not_read_as_other_zones_or_players(
    #[case] clause: &str,
) {
    let json = loyalty(
        r#"{"Minus":12}"#,
        r#"[{"what":"Player"}]"#,
        &format!("[{THEIR_LIBRARY},{THEIR_HAND}]"),
    );
    assert!(!written(&json, clause), "{clause}");
}

fn search(bind: &str, filter: &str, to: &str) -> String {
    loyalty(
        r#""MinusX""#,
        "[]",
        &format!(
            r#"[{{"Search":{{"filter":{filter},"as":"{bind}"}}}},{{"Move":{{"what":{{"Bound":"{bind}"}},"to":{to}}}}},{{"Shuffle":{{}}}}]"#
        ),
    )
}

const ARTIFACT_VALUE_X: &str =
    r#"[{"Type":"Artifact"},{"Compare":{"stat":"ManaValue","op":"Le","to":"X"}}]"#;
const BATTLEFIELD: &str = r#"{"Battlefield":{}}"#;

#[rstest]
#[case::called_by_its_type("artifact")]
#[case::called_found("found")]
#[case::called_card("card")]
fn a_search_is_written_whatever_the_card_it_finds_is_called(#[case] bind: &str) {
    let json = search(bind, ARTIFACT_VALUE_X, BATTLEFIELD);
    assert!(
        written(
            &json,
            "\u{2212}X: Search your library for an artifact card with mana value X or less, put it onto the battlefield, then shuffle."
        ),
        "{bind}"
    );
}

#[rstest]
#[case::another_type(
    "\u{2212}X: Search your library for a creature card with mana value X or less, put it onto the battlefield, then shuffle."
)]
#[case::another_limit(
    "\u{2212}X: Search your library for an artifact card with mana value X or more, put it onto the battlefield, then shuffle."
)]
#[case::another_place(
    "\u{2212}X: Search your library for an artifact card with mana value X or less, put it into your hand, then shuffle."
)]
#[case::no_shuffle(
    "\u{2212}X: Search your library for an artifact card with mana value X or less, put it onto the battlefield."
)]
#[case::their_library(
    "\u{2212}X: Search their library for an artifact card with mana value X or less, put it onto the battlefield, then shuffle."
)]
fn a_search_does_not_read_as_another_card_place_or_limit(#[case] clause: &str) {
    assert!(
        !written(&search("artifact", ARTIFACT_VALUE_X, BATTLEFIELD), clause),
        "{clause}"
    );
}

fn become_creatures(group: &str, mods: &str, duration: &str) -> String {
    loyalty(
        r#"{"Minus":5}"#,
        "[]",
        &format!(
            r#"[{{"Continuous":{{"affected":{{"Each":{group}}},"mods":{mods},"duration":"{duration}"}}}}]"#
        ),
    )
}

const ARTIFACTS_YOU_CONTROL: &str = r#"[{"Type":"Artifact"},{"ControlledBy":"You"}]"#;
const BECOME_5_5: &str = r#"[{"AddType":"Creature"},{"SetPT":{"power":5,"toughness":5}}]"#;

#[rstest]
#[case::artifacts(
    ARTIFACTS_YOU_CONTROL,
    BECOME_5_5,
    "\u{2212}5: Artifacts you control become artifact creatures with base power and toughness 5/5 until end of turn."
)]
#[case::the_size_first(
    ARTIFACTS_YOU_CONTROL,
    r#"[{"SetPT":{"power":5,"toughness":5}},{"AddType":"Creature"}]"#,
    "\u{2212}5: Artifacts you control become artifact creatures with base power and toughness 5/5 until end of turn."
)]
#[case::lands(
    r#"[{"Type":"Land"},{"ControlledBy":"You"}]"#,
    r#"[{"AddType":"Creature"},{"SetPT":{"power":0,"toughness":0}}]"#,
    "\u{2212}5: Lands you control become land creatures with base power and toughness 0/0 until end of turn."
)]
fn a_group_that_becomes_creatures_is_written_with_the_type_it_keeps(
    #[case] group: &str,
    #[case] mods: &str,
    #[case] clause: &str,
) {
    assert!(
        written(&become_creatures(group, mods, "UntilEndOfTurn"), clause),
        "{clause}"
    );
}

#[rstest]
#[case::another_size(
    "\u{2212}5: Artifacts you control become artifact creatures with base power and toughness 4/4 until end of turn."
)]
#[case::another_type(
    "\u{2212}5: Artifacts you control become enchantment creatures with base power and toughness 5/5 until end of turn."
)]
#[case::no_type(
    "\u{2212}5: Artifacts you control become creatures with base power and toughness 5/5 until end of turn."
)]
#[case::another_group(
    "\u{2212}5: Creatures you control become artifact creatures with base power and toughness 5/5 until end of turn."
)]
#[case::forever(
    "\u{2212}5: Artifacts you control become artifact creatures with base power and toughness 5/5."
)]
fn a_group_that_becomes_creatures_does_not_read_as_another_size_type_or_group(
    #[case] clause: &str,
) {
    assert!(
        !written(
            &become_creatures(ARTIFACTS_YOU_CONTROL, BECOME_5_5, "UntilEndOfTurn"),
            clause
        ),
        "{clause}"
    );
}

#[test]
fn a_group_that_becomes_creatures_for_good_is_not_worded_until_end_of_turn() {
    let json = become_creatures(ARTIFACTS_YOU_CONTROL, BECOME_5_5, "Forever");
    assert!(!written(
        &json,
        "\u{2212}5: Artifacts you control become artifact creatures with base power and toughness 5/5 until end of turn."
    ));
}
