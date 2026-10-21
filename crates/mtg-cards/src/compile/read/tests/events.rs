//! Goal: the events about players and permanents (damage, life, draws, discards, cycling, taps, untaps, mana, counters, blocks and attachments) read in a trigger in the shape of the hand-written files, write back as the card's own text, name "that player" and "that permanent's controller" where the event has them, and every other wording stays with the table.
use super::*;
use crate::refs::ZoneKind;

fn trigger(text: &str) -> Option<Ability> {
    clause(text, ClauseKind::Triggered, false)
}

#[rstest]
#[case::gaining_life(
    "Whenever you gain life, put a +1/+1 counter on ~.",
    r#"{"Triggered":{"when":{"LifeGained":{"who":"You"}},"effects":[{"PutCounters":{"on":"This","counter":"+1/+1"}}]}}"#
)]
#[case::damage_to_an_opponent(
    "Whenever ~ deals damage to an opponent, draw a card.",
    r#"{"Triggered":{"when":{"DealsDamage":{"source":["This"],"to":"Opponents"}},"effects":[{"Draw":{}}]}}"#
)]
#[case::combat_damage_by_a_creature_to_a_player(
    "Whenever a creature you control deals combat damage to a player, draw a card.",
    r#"{"Triggered":{"when":{"DealsDamage":{"source":[{"Type":"Creature"},{"ControlledBy":"You"}],"to":"EachPlayer","combat":true}},"effects":[{"Draw":{}}]}}"#
)]
#[case::an_opponent_draws(
    "Whenever an opponent draws a card, ~ deals 1 damage to that player.",
    r#"{"Triggered":{"when":{"Drew":{"who":"Opponents"}},"effects":[{"DealDamage":{"to":"Triggering","amount":1}}]}}"#
)]
#[case::an_opponent_discards(
    "Whenever an opponent discards a card, ~ deals 2 damage to that player.",
    r#"{"Triggered":{"when":{"Discarded":{"by":"Opponents"}},"effects":[{"DealDamage":{"to":"Triggering","amount":2}}]}}"#
)]
#[case::a_player_taps_a_land_for_mana(
    "Whenever a player taps a land for mana, ~ deals 1 damage to that player.",
    r#"{"Triggered":{"when":{"TappedForMana":{"what":[{"Type":"Land"}]}},"effects":[{"DealDamage":{"to":"Triggering","amount":1}}]}}"#
)]
#[case::a_permanent_becomes_untapped(
    "Whenever a permanent becomes untapped, that permanent's controller mills a card.",
    r#"{"Triggered":{"when":{"Untapped":{"what":[]}},"effects":[{"Mill":{"who":{"ControllerOf":"Triggering"}}}]}}"#
)]
#[case::this_becomes_tapped(
    "Whenever ~ becomes tapped, you gain 2 life.",
    r#"{"Triggered":{"when":{"Tapped":{"what":["This"]}},"effects":[{"GainLife":{"amount":2}}]}}"#
)]
#[case::cycling_this_card(
    "When you cycle ~, you may draw a card.",
    r#"{"Triggered":{"when":{"Cycled":{"what":["This"],"by":"You"}},"from":"Graveyard","effects":[{"May":{"effects":[{"Draw":{}}]}}]}}"#
)]
#[case::cycling_another_card(
    "Whenever you cycle another card, put a +1/+1 counter on ~.",
    r#"{"Triggered":{"when":{"Cycled":{"what":["Other"],"by":"You"}},"effects":[{"PutCounters":{"on":"This","counter":"+1/+1"}}]}}"#
)]
#[case::a_counter_is_put_on_this(
    "Whenever a +1/+1 counter is put on ~, you may draw a card.",
    r#"{"Triggered":{"when":{"CountersPut":{"what":["This"],"counter":"+1/+1"}},"effects":[{"May":{"effects":[{"Draw":{}}]}}]}}"#
)]
#[case::an_aura_becomes_attached_to_this(
    "Whenever an Aura becomes attached to ~, you gain 2 life.",
    r#"{"Triggered":{"when":{"Attached":{"what":[{"Subtype":"Aura"}],"to":["This"]}},"effects":[{"GainLife":{"amount":2}}]}}"#
)]
fn an_event_reads_in_the_shape_of_the_hand_written_files(#[case] text: &str, #[case] json: &str) {
    reads_as(text, ClauseKind::Triggered, false, json);
}

#[rstest]
#[case("Whenever an opponent gains life, you gain 1 life.")]
#[case("Whenever a player loses life, you gain 1 life.")]
#[case("Whenever ~ deals combat damage, draw a card.")]
#[case("Whenever ~ deals noncombat damage to you, draw a card.")]
#[case("Whenever a creature you control deals damage to an opponent, draw a card.")]
#[case("Whenever you draw a card, scry 1.")]
#[case("Whenever you discard a card, draw a card.")]
#[case("Whenever a player cycles a card, you gain 1 life.")]
#[case("Whenever an opponent cycles another card, you gain 1 life.")]
#[case("Whenever you tap a land for mana, draw a card.")]
#[case("Whenever an opponent taps a creature for mana, you gain 1 life.")]
#[case("Whenever a land you control becomes tapped, you gain 1 life.")]
#[case("Whenever ~ becomes untapped, you gain 2 life.")]
#[case("Whenever ~ blocks, draw a card.")]
#[case("Whenever ~ becomes blocked, draw a card.")]
#[case("Whenever ~ attacks or becomes blocked, draw a card.")]
#[case("Whenever a counter is put on a creature you control, draw a card.")]
#[case("Whenever a counter is removed from ~, draw a card.")]
#[case("Whenever a charge counter is removed from a permanent, you gain 1 life.")]
#[case("Whenever an opponent casts a spell, that player loses 1 life.")]
#[case("Whenever an opponent casts a spell, that player loses 1 life unless that player pays {1}.")]
#[case("Whenever a player draws a card, that player gains 1 life.")]
#[case("Whenever an opponent loses life, that player discards a card.")]
#[case("Whenever ~ becomes attached to a creature, draw a card.")]
#[case("Whenever an Equipment becomes attached to a creature you control, draw a card.")]
fn more_events_read_and_write_back(#[case] text: &str) {
    golden(text, ClauseKind::Triggered, false);
}

#[rstest]
#[case::a_cycle_trigger_from_the_graveyard("When you cycle ~, draw a card.", ZoneKind::Graveyard)]
#[case::another_card_from_the_battlefield(
    "Whenever you cycle another card, draw a card.",
    ZoneKind::Battlefield
)]
#[case::an_event_of_a_permanent("Whenever ~ becomes tapped, draw a card.", ZoneKind::Battlefield)]
fn only_the_cycling_of_the_card_itself_works_from_the_graveyard(
    #[case] text: &str,
    #[case] expected: ZoneKind,
) {
    let Some(Ability::Triggered { from, .. }) = trigger(text) else {
        panic!("not read: {text}");
    };
    assert_eq!(from, expected);
}

#[rstest]
#[case::the_player_of_a_step("At the beginning of your upkeep, ~ deals 1 damage to that player.")]
#[case::no_player_in_an_attack("Whenever ~ attacks, ~ deals 1 damage to that player.")]
#[case::no_player_in_a_tap("Whenever ~ becomes tapped, ~ deals 1 damage to that player.")]
#[case::no_permanent_in_a_draw(
    "Whenever you draw a card, that permanent's controller mills a card."
)]
#[case::damage_to_a_creature("Whenever ~ deals damage to a creature, draw a card.")]
#[case::damage_to_a_planeswalker("Whenever ~ deals combat damage to a planeswalker, draw a card.")]
#[case::cycling_a_type("Whenever you cycle a creature card, draw a card.")]
#[case::cycling_this_by_a_player("When a player cycles ~, draw a card.")]
#[case::a_counter_without_an_article("Whenever counters are put on ~, draw a card.")]
#[case::a_counter_of_a_rules_kind("Whenever a stun counter is put on ~, draw a card.")]
#[case::a_counter_moved("Whenever a +1/+1 counter is moved onto ~, draw a card.")]
#[case::tapping_for_nothing("Whenever you tap a land, draw a card.")]
#[case::attached_to_nothing("Whenever an Aura becomes attached, draw a card.")]
#[case::untapping_a_creature_or_not("Whenever ~ becomes tapped or untapped, draw a card.")]
#[case::blocking_something("Whenever ~ blocks a creature, draw a card.")]
#[case::an_effect_not_read("Whenever ~ becomes attached to a creature, tap that creature.")]
fn other_wordings_of_the_events_stay_unread(#[case] text: &str) {
    assert_eq!(trigger(text), None, "{text}");
}
