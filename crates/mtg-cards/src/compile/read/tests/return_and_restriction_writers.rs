//! Goal: the writers of the proposal checker give the oracle wording of a restriction that lasts the turn ("Target creature can't attack or block this turn."), of the top card of the library exiled whatever a later step calls it, of a card returned from the graveyard to the battlefield with its counters, alone or after a surveil, and of counters removed from each object of a group, and a wrong subject, duration, count, zone or counter never reads back.
use super::*;

/// Whether some wording the checker accepts for the rules is the clause.
fn written(json: &str, clause: &str) -> bool {
    written_variants(&ability(json))
        .iter()
        .any(|wording| same_text(wording, clause))
}

fn restriction(group: &str, mods: &str, duration: &str) -> String {
    format!(
        r#"{{"Spell":{{"targets":[{{"what":{{"Object":[{{"Type":"Creature"}}]}}}}],"effects":[{{"Continuous":{{"affected":{group},"mods":{mods},"duration":"{duration}"}}}}]}}}}"#
    )
}

const TARGET: &str = r#"{"Target":0}"#;

#[rstest]
#[case::attack_and_block(
    TARGET,
    r#"["CantAttack","CantBlock"]"#,
    "Target creature can't attack or block this turn."
)]
#[case::attack(TARGET, r#"["CantAttack"]"#, "Target creature can't attack this turn.")]
#[case::block(TARGET, r#"["CantBlock"]"#, "Target creature can't block this turn.")]
fn a_restriction_of_the_turn_is_written_with_this_turn(
    #[case] group: &str,
    #[case] mods: &str,
    #[case] clause: &str,
) {
    assert!(
        written(&restriction(group, mods, "UntilEndOfTurn"), clause),
        "{clause}"
    );
}

#[rstest]
#[case::until_end_of_turn("Target creature can't attack or block until end of turn.")]
#[case::only_attacking("Target creature can't attack this turn.")]
#[case::another_restriction("Target creature can't be blocked this turn.")]
#[case::forever("Target creature can't attack or block.")]
#[case::each_creature("Each creature can't attack or block this turn.")]
fn a_restriction_of_the_turn_does_not_read_as_another(#[case] clause: &str) {
    let json = restriction(TARGET, r#"["CantAttack","CantBlock"]"#, "UntilEndOfTurn");
    assert!(!written(&json, clause), "{clause}");
}

#[test]
fn a_restriction_that_lasts_until_the_next_turn_is_not_worded_this_turn() {
    let json = restriction(TARGET, r#"["CantAttack","CantBlock"]"#, "UntilYourNextTurn");
    assert!(!written(
        &json,
        "Target creature can't attack or block this turn."
    ));
}

fn exile_top(who: &str, count: u32, bind: &str) -> String {
    format!(
        r#"{{"Triggered":{{"when":{{"Sacrificed":{{"what":[{{"Not":"Token"}}],"by":"You"}}}},"effects":[{{"ExileTop":{{"who":{who},"count":{count},"as":"{bind}"}}}}]}}}}"#
    )
}

#[rstest]
#[case::one_card(
    "exiled",
    1,
    "Whenever you sacrifice a nontoken permanent, exile the top card of your library."
)]
#[case::called_by_a_later_step(
    "impulse",
    1,
    "Whenever you sacrifice a nontoken permanent, exile the top card of your library."
)]
#[case::three_cards(
    "exiled",
    3,
    "Whenever you sacrifice a nontoken permanent, exile the top three cards of your library."
)]
fn the_top_of_the_library_is_exiled_whatever_the_cards_are_called(
    #[case] bind: &str,
    #[case] count: u32,
    #[case] clause: &str,
) {
    assert!(
        written(&exile_top(r#""You""#, count, bind), clause),
        "{clause}"
    );
}

#[rstest]
#[case::two_cards(
    "Whenever you sacrifice a nontoken permanent, exile the top two cards of your library."
)]
#[case::the_bottom(
    "Whenever you sacrifice a nontoken permanent, exile the bottom card of your library."
)]
#[case::a_graveyard(
    "Whenever you sacrifice a nontoken permanent, exile the top card of your graveyard."
)]
#[case::a_token("Whenever you sacrifice a permanent, exile the top card of your library.")]
fn the_top_of_the_library_does_not_read_as_another_count_or_place(#[case] clause: &str) {
    assert!(
        !written(&exile_top(r#""You""#, 1, "exiled"), clause),
        "{clause}"
    );
}

const FROM_YOUR_GRAVEYARD: &str =
    r#"{"Each":[{"In":"Graveyard"},{"OwnedBy":"You"},{"Type":"Creature"}]}"#;

fn returned(to: &str, first: &str) -> String {
    format!(
        r#"{{"Spell":{{"effects":[{first}{{"Choose":{{"from":{FROM_YOUR_GRAVEYARD},"as":"card"}}}},{{"Move":{{"what":{{"Bound":"card"}},"to":{to}}}}}]}}}}"#
    )
}

const ENTERS: &str = r#"{"Battlefield":{}}"#;
const WITH_FINALITY: &str = r#"{"Battlefield":{"counters":[[{"Other":"finality"},1]]}}"#;
const WITH_TWO_FINALITY: &str = r#"{"Battlefield":{"counters":[[{"Other":"finality"},2]]}}"#;
const TAPPED: &str = r#"{"Battlefield":{"tapped":true}}"#;
const SURVEIL: &str = r#"{"Surveil":{"count":3}},"#;

#[rstest]
#[case::plain(
    ENTERS,
    "",
    "Return a creature card from your graveyard to the battlefield."
)]
#[case::tapped(
    TAPPED,
    "",
    "Return a creature card from your graveyard to the battlefield tapped."
)]
#[case::with_a_counter(
    WITH_FINALITY,
    "",
    "Return a creature card from your graveyard to the battlefield with a finality counter on it."
)]
#[case::with_two_counters(
    WITH_TWO_FINALITY,
    "",
    "Return a creature card from your graveyard to the battlefield with two finality counters on it."
)]
#[case::after_a_surveil(
    WITH_FINALITY,
    SURVEIL,
    "Surveil 3, then return a creature card from your graveyard to the battlefield with a finality counter on it."
)]
fn a_card_chosen_from_the_graveyard_is_returned_to_the_battlefield(
    #[case] to: &str,
    #[case] first: &str,
    #[case] clause: &str,
) {
    assert!(written(&returned(to, first), clause), "{clause}");
}

#[rstest]
#[case::the_hand(
    "Return a creature card from your graveyard to your hand with a finality counter on it."
)]
#[case::another_counter(
    "Return a creature card from your graveyard to the battlefield with a stun counter on it."
)]
#[case::two_counters(
    "Return a creature card from your graveyard to the battlefield with two finality counters on it."
)]
#[case::no_counter("Return a creature card from your graveyard to the battlefield.")]
#[case::tapped(
    "Return a creature card from your graveyard to the battlefield tapped with a finality counter on it."
)]
#[case::another_card(
    "Return a land card from your graveyard to the battlefield with a finality counter on it."
)]
#[case::their_graveyard(
    "Return a creature card from their graveyard to the battlefield with a finality counter on it."
)]
#[case::a_target(
    "Return target creature card from your graveyard to the battlefield with a finality counter on it."
)]
#[case::after_a_surveil(
    "Surveil 3, then return a creature card from your graveyard to the battlefield with a finality counter on it."
)]
fn a_returned_card_does_not_read_as_another_zone_counter_or_card(#[case] clause: &str) {
    assert!(!written(&returned(WITH_FINALITY, ""), clause), "{clause}");
}

fn remove_from(group: &str, counter: &str, count: u32) -> String {
    format!(
        r#"{{"Spell":{{"effects":[{{"RemoveCounters":{{"from":{{"Each":{group}}},"counter":"{counter}","count":{count}}}}}]}}}}"#
    )
}

#[rstest]
#[case::one(
    r#"[{"Type":"Creature"}]"#,
    "+1/+1",
    1,
    "Remove a +1/+1 counter from each creature."
)]
#[case::two(
    r#"[{"Type":"Creature"},{"ControlledBy":"You"}]"#,
    "-1/-1",
    2,
    "Remove two -1/-1 counters from each creature you control."
)]
fn counters_are_removed_from_each_object_of_a_group(
    #[case] group: &str,
    #[case] counter: &str,
    #[case] count: u32,
    #[case] clause: &str,
) {
    assert!(
        written(&remove_from(group, counter, count), clause),
        "{clause}"
    );
}

#[rstest]
#[case::the_other_kind("Remove a -1/-1 counter from each creature.")]
#[case::the_other_count("Remove two +1/+1 counters from each creature.")]
#[case::one_creature("Remove a +1/+1 counter from target creature.")]
#[case::your_creatures("Remove a +1/+1 counter from each creature you control.")]
fn counters_removed_from_a_group_do_not_read_as_another_kind_count_or_group(#[case] clause: &str) {
    assert!(
        !written(&remove_from(r#"[{"Type":"Creature"}]"#, "+1/+1", 1), clause),
        "{clause}"
    );
}

#[test]
fn a_surveil_before_a_return_is_not_dropped() {
    assert!(!written(
        &returned(WITH_FINALITY, SURVEIL),
        "Return a creature card from your graveyard to the battlefield with a finality counter on it."
    ));
}
