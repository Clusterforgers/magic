//! Goal: the writers give the oracle wording of an event that is one of several things a player does to a card, of steps for different players after a payment, of a group of cards named by several subtypes, of a pump that gives an ability in quotes and of a copy of the creature that was sacrificed, and a wrong player, event, group, amount or kind never reads back.
use super::*;

/// Whether some wording the checker accepts for the rules is the clause.
fn written(json: &str, clause: &str) -> bool {
    written_variants(&ability(json))
        .iter()
        .any(|wording| same_text(wording, clause))
}

fn on_cards(events: &str) -> String {
    format!(
        r#"{{"Triggered":{{"when":{{"AnyOf":[{events}]}},"effects":[{{"Pay":{{"cost":"{{1}}","paid":[{{"LoseLife":{{"who":"Opponents","amount":2}}}},{{"GainLife":{{"amount":2}}}}]}}}}]}}}}"#
    )
}

const CYCLED: &str = r#"{"Cycled":{"what":[],"by":"You"}}"#;
const DISCARDED: &str = r#"{"Discarded":{"by":"You"}}"#;

#[rstest]
#[case::cycle_or_discard(
    format!("{CYCLED},{DISCARDED}"),
    "Whenever you cycle or discard a card, you may pay {1}. If you do, each opponent loses 2 life and you gain 2 life."
)]
#[case::discard_or_cycle(
    format!("{DISCARDED},{CYCLED}"),
    "Whenever you discard or cycle a card, you may pay {1}. If you do, each opponent loses 2 life and you gain 2 life."
)]
#[case::steps_in_a_row(
    format!("{CYCLED},{DISCARDED}"),
    "Whenever you cycle or discard a card, you may pay {1}. If you do, each opponent loses 2 life, then you gain 2 life."
)]
fn one_of_two_things_a_player_does_to_a_card_is_written_as_the_oracle_words_it(
    #[case] events: String,
    #[case] clause: &str,
) {
    assert!(written(&on_cards(&events), clause), "{clause}");
}

#[rstest]
#[case::an_opponent(
    "Whenever an opponent cycles or discards a card, you may pay {1}. If you do, each opponent loses 2 life and you gain 2 life."
)]
#[case::a_draw_for_a_cycle(
    "Whenever you draw or discard a card, you may pay {1}. If you do, each opponent loses 2 life and you gain 2 life."
)]
#[case::one_event(
    "Whenever you cycle a card, you may pay {1}. If you do, each opponent loses 2 life and you gain 2 life."
)]
#[case::another_card(
    "Whenever you cycle or discard another card, you may pay {1}. If you do, each opponent loses 2 life and you gain 2 life."
)]
#[case::another_amount(
    "Whenever you cycle or discard a card, you may pay {1}. If you do, each opponent loses 3 life and you gain 2 life."
)]
#[case::the_players_swapped(
    "Whenever you cycle or discard a card, you may pay {1}. If you do, you gain 2 life and each opponent loses 2 life."
)]
#[case::another_cost(
    "Whenever you cycle or discard a card, you may pay {2}. If you do, each opponent loses 2 life and you gain 2 life."
)]
fn one_of_two_things_a_player_does_does_not_read_as_another_player_event_or_step(
    #[case] clause: &str,
) {
    let json = on_cards(&format!("{CYCLED},{DISCARDED}"));
    assert!(!written(&json, clause), "{clause}");
}

#[test]
fn two_events_of_two_players_are_not_one_player_event() {
    let json = on_cards(&format!(r#"{CYCLED},{{"Discarded":{{"by":"Opponents"}}}}"#));
    for clause in [
        "Whenever you cycle or discard a card, you may pay {1}. If you do, each opponent loses 2 life and you gain 2 life.",
        "Whenever an opponent cycles or discards a card, you may pay {1}. If you do, each opponent loses 2 life and you gain 2 life.",
    ] {
        assert!(!written(&json, clause), "{clause}");
    }
}

fn historian(subtypes: &str, keyword: &str) -> String {
    format!(
        r#"{{"Static":{{"affected":{{"Each":[{{"In":"Graveyard"}},{{"OwnedBy":"You"}},{{"AnyOf":[{subtypes}]}}]}},"mods":[{{"AddKeyword":"{keyword}"}}]}}}}"#
    )
}

const MERFOLK_OR_DRUID: &str = r#"{"Subtype":"Merfolk"},{"Subtype":"Druid"}"#;

#[rstest]
#[case::and(
    MERFOLK_OR_DRUID,
    "Merfolk and Druid cards in your graveyard have retrace."
)]
#[case::or(
    MERFOLK_OR_DRUID,
    "Merfolk or Druid cards in your graveyard have retrace."
)]
#[case::three(
    r#"{"Subtype":"Merfolk"},{"Subtype":"Druid"},{"Subtype":"Wizard"}"#,
    "Merfolk, Druid, and Wizard cards in your graveyard have retrace."
)]
fn several_subtypes_of_cards_in_a_graveyard_are_written_as_the_oracle_words_them(
    #[case] subtypes: &str,
    #[case] clause: &str,
) {
    assert!(written(&historian(subtypes, "Retrace"), clause), "{clause}");
}

#[rstest]
#[case::another_subtype("Merfolk and Elf cards in your graveyard have retrace.")]
#[case::creatures("Merfolk and Druid creatures in your graveyard have retrace.")]
#[case::another_graveyard("Merfolk and Druid cards in a graveyard have retrace.")]
#[case::another_keyword("Merfolk and Druid cards in your graveyard have flashback.")]
#[case::one_subtype("Merfolk cards in your graveyard have retrace.")]
fn several_subtypes_of_cards_do_not_read_as_another_group_or_keyword(#[case] clause: &str) {
    assert!(
        !written(&historian(MERFOLK_OR_DRUID, "Retrace"), clause),
        "{clause}"
    );
}

const PROWESS: &str = r#"{"Spell":{"targets":[{"what":{"Object":[{"Type":"Creature"}]}}],"effects":[{"Continuous":{"affected":{"Target":0},"mods":[{"ModifyPT":{"power":3,"toughness":3}},{"AddKeyword":"Trample"},{"GrantAbility":{"Triggered":{"when":{"DealsDamage":{"source":["This"],"to":"EachPlayer","combat":true}},"effects":[{"Draw":{"count":"Triggering"}}]}}}],"duration":"UntilEndOfTurn"}}]}}"#;

#[test]
fn a_pump_that_gains_a_keyword_and_an_ability_in_quotes_is_written_as_the_oracle_words_it() {
    assert!(written(
        PROWESS,
        "Until end of turn, target creature gets +3/+3 and gains trample and \"Whenever ~ deals combat damage to a player, draw that many cards.\""
    ));
}

#[test]
fn a_gain_of_an_ability_in_quotes_alone_is_written_as_the_oracle_words_it() {
    let json = r#"{"Spell":{"targets":[{"what":{"Object":[{"Type":"Creature"}]}}],"effects":[{"Continuous":{"affected":{"Target":0},"mods":[{"GrantAbility":{"Triggered":{"when":{"DealsDamage":{"source":["This"],"to":"EachPlayer","combat":true}},"effects":[{"Draw":{"count":"Triggering"}}]}}}],"duration":"UntilEndOfTurn"}}]}}"#;
    assert!(written(
        json,
        "Until end of turn, target creature gains \"Whenever ~ deals combat damage to a player, draw that many cards.\""
    ));
}

#[rstest]
#[case::another_size(
    "Until end of turn, target creature gets +2/+2 and gains trample and \"Whenever ~ deals combat damage to a player, draw that many cards.\""
)]
#[case::another_keyword(
    "Until end of turn, target creature gets +3/+3 and gains flying and \"Whenever ~ deals combat damage to a player, draw that many cards.\""
)]
#[case::another_ability(
    "Until end of turn, target creature gets +3/+3 and gains trample and \"Whenever ~ deals combat damage to a player, draw a card.\""
)]
#[case::no_ability("Until end of turn, target creature gets +3/+3 and gains trample.")]
#[case::another_event(
    "Until end of turn, target creature gets +3/+3 and gains trample and \"Whenever ~ deals damage to a player, draw that many cards.\""
)]
fn a_pump_with_an_ability_in_quotes_does_not_read_as_another_pump(#[case] clause: &str) {
    assert!(!written(PROWESS, clause), "{clause}");
}

fn skaab(sacrificed: &str, count: u32) -> String {
    format!(
        r#"{{"Activated":{{"cost":[{{"Mana":"{{3}}"}},"Tap",{{"Sacrifice":{{"filter":[{sacrificed},"Other"]}}}}],"effects":[{{"CreateToken":{{"token":{{"CopyOf":{{"of":"Paid"}}}},"count":{count}}}}}]}}}}"#
    )
}

#[test]
fn a_copy_of_the_creature_that_was_sacrificed_is_written_as_the_oracle_words_it() {
    assert!(written(
        &skaab(r#"{"Subtype":"Zombie"}"#, 2),
        "{3}, {T}, Sacrifice another Zombie: Create two tokens that are copies of the sacrificed creature."
    ));
}

#[rstest]
#[case::an_artifact(
    "{3}, {T}, Sacrifice another Zombie: Create two tokens that are copies of the sacrificed artifact."
)]
#[case::three_copies(
    "{3}, {T}, Sacrifice another Zombie: Create three tokens that are copies of the sacrificed creature."
)]
#[case::the_card_itself(
    "{3}, {T}, Sacrifice another Zombie: Create two tokens that are copies of ~."
)]
#[case::another_sacrifice(
    "{3}, {T}, Sacrifice another Elf: Create two tokens that are copies of the sacrificed creature."
)]
fn a_copy_of_the_sacrificed_creature_does_not_read_as_another_copy(#[case] clause: &str) {
    assert!(
        !written(&skaab(r#"{"Subtype":"Zombie"}"#, 2), clause),
        "{clause}"
    );
}
