//! Goal: the writers give the oracle wording of counters put on targets that differ by who controls them, of cards drawn for what each of two players controls, of a switch of power and toughness, of damage to a player or planeswalker and the creatures its controller controls, and of the cards in the hand of the player an event is about, and a wrong count, kind, target or player never reads back.
use super::*;

/// Whether some wording the checker accepts for the rules is the clause.
fn written(json: &str, clause: &str) -> bool {
    written_variants(&ability(json))
        .iter()
        .any(|wording| same_text(wording, clause))
}

const YOURS: &str = r#"{"what":{"Object":[{"Type":"Creature"},{"ControlledBy":"You"}]}}"#;
const THEIRS: &str = r#"{"what":{"Object":[{"Type":"Creature"},{"ControlledBy":"Opponents"}]}}"#;
const UPKEEP: &str = r#"{"StepBegins":{"step":"Upkeep","whose":"You"}}"#;

fn put(at: u32, counter: &str, count: u32) -> String {
    format!(r#"{{"PutCounters":{{"on":{{"Target":{at}}},"counter":"{counter}","count":{count}}}}}"#)
}

/// An upkeep trigger that puts counters on two targets.
fn counters_on_two(targets: [&str; 2], counts: [u32; 2]) -> String {
    format!(
        r#"{{"Triggered":{{"when":{UPKEEP},"targets":[{},{}],"effects":[{},{}]}}}}"#,
        targets[0],
        targets[1],
        put(0, "+1/+1", counts[0]),
        put(1, "+1/+1", counts[1])
    )
}

#[rstest]
#[case::the_same_number(
    [3, 3],
    "At the beginning of your upkeep, put three +1/+1 counters on target creature you control and three +1/+1 counters on target creature an opponent controls."
)]
#[case::two_numbers(
    [1, 2],
    "At the beginning of your upkeep, put a +1/+1 counter on target creature you control and two +1/+1 counters on target creature an opponent controls."
)]
fn counters_on_two_targets_that_differ_are_one_sentence_without_another(
    #[case] counts: [u32; 2],
    #[case] clause: &str,
) {
    assert!(written(&counters_on_two([YOURS, THEIRS], counts), clause));
}

#[rstest]
#[case::another_number(
    "At the beginning of your upkeep, put three +1/+1 counters on target creature you control and two +1/+1 counters on target creature an opponent controls."
)]
#[case::the_targets_swapped(
    "At the beginning of your upkeep, put three +1/+1 counters on target creature an opponent controls and three +1/+1 counters on target creature you control."
)]
#[case::another_kind(
    "At the beginning of your upkeep, put three -1/-1 counters on target creature you control and three -1/-1 counters on target creature an opponent controls."
)]
#[case::an_ordinal_for_a_target_that_differs(
    "At the beginning of your upkeep, put three +1/+1 counters on target creature you control and three +1/+1 counters on another target creature an opponent controls."
)]
fn counters_on_two_targets_that_differ_do_not_read_as_other_counts_kinds_or_targets(
    #[case] clause: &str,
) {
    assert!(!written(&counters_on_two([YOURS, THEIRS], [3, 3]), clause));
}

#[test]
fn counters_on_two_targets_with_the_same_words_need_another() {
    let same = counters_on_two([YOURS, YOURS], [3, 3]);
    let clause = "At the beginning of your upkeep, put three +1/+1 counters on target creature you control and three +1/+1 counters on target creature you control.";
    assert!(!written(&same, clause));
    let another = "At the beginning of your upkeep, put three +1/+1 counters on target creature you control and three +1/+1 counters on another target creature you control.";
    assert!(!written(&same, another), "no Other in the second target");
}

fn mastery(second: u32) -> String {
    format!(
        r#"{{"Spell":{{"targets":[{{"what":"Player"}},{{"what":"Player"}}],"effects":[{{"Draw":{{"count":{{"Count":[{{"Type":"Creature"}},{{"ControlledBy":{{"Target":0}}}}]}}}}}},{{"Draw":{{"count":{{"Count":[{{"Type":"Creature"}},{{"ControlledBy":{{"Target":{second}}}}}]}}}}}}]}}}}"#
    )
}

#[test]
fn a_draw_for_what_each_of_two_players_controls_is_written_as_the_oracle_words_it() {
    let clause = "Draw a card for each creature target player controls, then draw a card for each creature another target player controls.";
    assert!(written(&mastery(1), clause));
}

#[rstest]
#[case::a_land_for_a_creature(
    "Draw a card for each land target player controls, then draw a card for each creature another target player controls."
)]
#[case::the_players_swapped(
    "Draw a card for each creature another target player controls, then draw a card for each creature target player controls."
)]
#[case::the_same_player_twice(
    "Draw a card for each creature target player controls, then draw a card for each creature target player controls."
)]
#[case::your_own_creatures(
    "Draw a card for each creature you control, then draw a card for each creature another target player controls."
)]
#[case::one_draw_only("Draw a card for each creature target player controls.")]
fn a_draw_for_what_two_players_control_does_not_read_as_another_count_or_player(
    #[case] clause: &str,
) {
    assert!(!written(&mastery(1), clause));
}

#[test]
fn a_draw_for_both_counts_of_the_first_target_is_not_the_other_player() {
    let clause = "Draw a card for each creature target player controls, then draw a card for each creature another target player controls.";
    assert!(!written(&mastery(0), clause));
}

const SWITCH: &str =
    r#"{"Continuous":{"affected":{"Target":0},"mods":["SwitchPT"],"duration":"UntilEndOfTurn"}}"#;

fn switch_of(bounds: &str) -> String {
    format!(
        r#"{{"Spell":{{"targets":[{{"what":{{"Object":[{{"Type":"Creature"}}]}}{bounds}}}],"effects":[{SWITCH}]}}}}"#
    )
}

#[rstest]
#[case::one_target("", "Switch target creature's power and toughness until end of turn.")]
#[case::any_number(
    r#","bounds":{"min":0,"max":null}"#,
    "Switch the power and toughness of each of any number of target creatures until end of turn."
)]
#[case::up_to_three(
    r#","bounds":{"min":0,"max":3}"#,
    "Switch the power and toughness of each of up to three target creatures until end of turn."
)]
fn a_switch_of_power_and_toughness_is_written_as_the_oracle_words_it(
    #[case] bounds: &str,
    #[case] clause: &str,
) {
    assert!(written(&switch_of(bounds), clause), "{clause}");
}

#[rstest]
#[case::one_target_for_any_number(
    r#","bounds":{"min":0,"max":null}"#,
    "Switch target creature's power and toughness until end of turn."
)]
#[case::any_number_for_one_target(
    "",
    "Switch the power and toughness of each of any number of target creatures until end of turn."
)]
#[case::up_to_one_target(
    r#","bounds":{"min":0,"max":1}"#,
    "Switch the power and toughness of each of up to one target creature until end of turn."
)]
#[case::exactly_two_targets(
    r#","bounds":{"min":2,"max":2}"#,
    "Switch the power and toughness of each of two target creatures until end of turn."
)]
fn a_switch_of_power_and_toughness_does_not_read_as_another_number_of_targets(
    #[case] bounds: &str,
    #[case] clause: &str,
) {
    assert!(!written(&switch_of(bounds), clause), "{clause}");
}

#[test]
fn a_switch_for_a_longer_time_is_not_a_switch_for_the_turn() {
    let json = r#"{"Spell":{"targets":[{"what":{"Object":[{"Type":"Creature"}]}}],"effects":[{"Continuous":{"affected":{"Target":0},"mods":["SwitchPT"],"duration":"UntilYourNextTurn"}}]}}"#;
    assert!(!written(
        json,
        "Switch target creature's power and toughness until end of turn."
    ));
}

const LAVALANCHE: &str = r#"{"Spell":{"targets":[{"what":{"Either":["Player",{"Object":[{"Type":"Planeswalker"}]}]}}],"effects":[{"DealDamage":{"to":{"Target":0},"amount":"X"}},{"DealDamage":{"to":{"Each":[{"Type":"Creature"},{"AnyOf":[{"ControlledBy":{"Target":0}},{"ControlledBy":{"ControllerOf":{"Target":0}}}]}]},"amount":"X"}}]}}"#;

#[test]
fn damage_to_a_player_or_planeswalker_and_the_creatures_of_its_controller_is_written_as_the_oracle_words_it()
 {
    assert!(written(
        LAVALANCHE,
        "~ deals X damage to target player or planeswalker and each creature that player or that planeswalker's controller controls."
    ));
}

#[rstest]
#[case::the_player_only(
    "~ deals X damage to target player or planeswalker and each creature target player controls."
)]
#[case::your_creatures(
    "~ deals X damage to target player or planeswalker and each creature you control."
)]
#[case::the_player_who_is_hit(
    "~ deals X damage to target player or planeswalker and each creature that player controls."
)]
#[case::another_amount(
    "~ deals 3 damage to target player or planeswalker and each creature that player or that planeswalker's controller controls."
)]
fn damage_to_a_player_or_planeswalker_does_not_read_as_another_group(#[case] clause: &str) {
    assert!(!written(LAVALANCHE, clause), "{clause}");
}

fn muse(who: &str, zone: &str, owner: &str) -> String {
    format!(
        r#"{{"Triggered":{{"when":{{"StepBegins":{{"step":"Upkeep","whose":"EachPlayer"}}}},"effects":[{{"Mill":{{"who":"{who}","count":{{"Count":[{{"In":"{zone}"}},{{"OwnedBy":"{owner}"}}]}}}}}}]}}}}"#
    )
}

#[rstest]
#[case::where_x(
    "At the beginning of each player's upkeep, that player mills X cards, where X is the number of cards in their hand."
)]
#[case::for_each(
    "At the beginning of each player's upkeep, that player mills a card for each card in their hand."
)]
fn the_cards_in_the_hand_of_the_player_of_an_event_are_written_as_their_hand(#[case] clause: &str) {
    assert!(written(&muse("Triggering", "Hand", "Triggering"), clause));
}

#[rstest]
#[case::your_hand(
    "At the beginning of each player's upkeep, that player mills X cards, where X is the number of cards in your hand."
)]
#[case::their_graveyard(
    "At the beginning of each player's upkeep, that player mills X cards, where X is the number of cards in their graveyard."
)]
#[case::you_mill(
    "At the beginning of each player's upkeep, you mill X cards, where X is the number of cards in their hand."
)]
fn the_cards_in_a_hand_do_not_read_as_another_zone_or_player(#[case] clause: &str) {
    assert!(!written(&muse("Triggering", "Hand", "Triggering"), clause));
}

#[test]
fn the_cards_in_the_graveyard_of_the_player_of_an_event_are_not_their_hand() {
    assert!(!written(
        &muse("Triggering", "Graveyard", "Triggering"),
        "At the beginning of each player's upkeep, that player mills X cards, where X is the number of cards in their hand."
    ));
}
