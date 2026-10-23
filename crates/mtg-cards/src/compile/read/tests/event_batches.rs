//! Goal: the triggers whose subject follows an attachment ("equipped creature"), the events that happen together ("one or more"), the nth card or spell of a turn and the amount of the event ("that much", "that many") read in the shape of the hand-written files and write back as the card's own text; every other wording stays unread.
use super::*;

#[rstest]
#[case::equipped_creature_dies(
    "Whenever equipped creature dies, draw two cards.",
    r#"{"Triggered":{"when":{"ZoneChange":{"what":[{"Is":"Attached"}],"from":"Battlefield","to":"Graveyard"}},"effects":[{"Draw":{"count":2}}]}}"#
)]
#[case::combat_damage_by_the_host(
    "Whenever equipped creature deals combat damage to a player, you gain 3 life.",
    r#"{"Triggered":{"when":{"DealsDamage":{"source":[{"Is":"Attached"}],"to":"EachPlayer","combat":true}},"effects":[{"GainLife":{"amount":3}}]}}"#
)]
#[case::batch_of_damage(
    "Whenever one or more creatures you control deal combat damage to a player, draw a card.",
    r#"{"Triggered":{"when":{"Batch":{"DealsDamage":{"source":[{"Type":"Creature"},{"ControlledBy":"You"}],"to":"EachPlayer","combat":true}}},"effects":[{"Draw":{}}]}}"#
)]
#[case::batch_of_cards_milled(
    "Whenever one or more cards are put into your graveyard from your library, you gain 1 life.",
    r#"{"Triggered":{"when":{"Batch":{"ZoneChange":{"what":[{"OwnedBy":"You"}],"from":"Library","to":"Graveyard"}}},"effects":[{"GainLife":{"amount":1}}]}}"#
)]
#[case::attackers_together(
    "Whenever one or more creatures you control attack, draw a card.",
    r#"{"Triggered":{"when":{"GroupAttacks":{"what":[{"Type":"Creature"},{"ControlledBy":"You"}]}},"effects":[{"Draw":{}}]}}"#
)]
#[case::the_second_card(
    "Whenever you draw your second card each turn, put a +1/+1 counter on ~.",
    r#"{"Triggered":{"when":{"Nth":{"event":{"Drew":{"who":"You"}},"nth":2}},"effects":[{"PutCounters":{"on":"This","counter":"+1/+1"}}]}}"#
)]
#[case::the_first_attack(
    "Whenever ~ attacks for the first time each turn, draw a card.",
    r#"{"Triggered":{"when":{"Nth":{"event":{"Attacks":{"what":["This"]}},"nth":1}},"effects":[{"Draw":{}}]}}"#
)]
#[case::the_first_time_a_target_of_yours(
    "Whenever ~ becomes the target of a spell or ability you control for the first time each turn, put a +1/+1 counter on ~.",
    r#"{"Triggered":{"when":{"Nth":{"event":{"BecomesTarget":{"what":["This"],"by":[{"ControlledBy":"You"}]}},"nth":1}},"effects":[{"PutCounters":{"on":"This","counter":"+1/+1"}}]}}"#
)]
#[case::the_second_time_a_target_of_anyone(
    "Whenever a creature you control becomes the target of a spell or ability for the second time each turn, draw a card.",
    r#"{"Triggered":{"when":{"Nth":{"event":{"BecomesTarget":{"what":[{"Type":"Creature"},{"ControlledBy":"You"}]}},"nth":2}},"effects":[{"Draw":{}}]}}"#
)]
#[case::a_target_of_yours(
    "Whenever ~ becomes the target of a spell or ability you control, draw a card.",
    r#"{"Triggered":{"when":{"BecomesTarget":{"what":["This"],"by":[{"ControlledBy":"You"}]}},"effects":[{"Draw":{}}]}}"#
)]
#[case::a_target_of_an_opponent(
    "Whenever ~ becomes the target of a spell or ability an opponent controls, draw a card.",
    r#"{"Triggered":{"when":{"BecomesTarget":{"what":["This"],"by":[{"ControlledBy":"Opponents"}]}},"effects":[{"Draw":{}}]}}"#
)]
#[case::dealt_damage(
    "Whenever ~ is dealt damage, draw a card.",
    r#"{"Triggered":{"when":{"DealsDamage":{"source":[],"to":"This"}},"effects":[{"Draw":{}}]}}"#
)]
#[case::first_dealt_damage(
    "Whenever ~ is dealt damage for the first time each turn, draw a card.",
    r#"{"Triggered":{"when":{"Nth":{"event":{"DealsDamage":{"source":[],"to":"This"}},"nth":1}},"effects":[{"Draw":{}}]}}"#
)]
#[case::a_creature_dealt_damage(
    "Whenever a creature you control is dealt damage, draw a card.",
    r#"{"Triggered":{"when":{"DealsDamage":{"source":[],"to":{"Each":[{"Type":"Creature"},{"ControlledBy":"You"}]}}},"effects":[{"Draw":{}}]}}"#
)]
#[case::that_much_life(
    "Whenever you gain life, target opponent loses that much life.",
    r#"{"Triggered":{"when":{"LifeGained":{"who":"You"}},"targets":[{"what":"Opponent"}],"effects":[{"LoseLife":{"who":{"Target":0},"amount":"Triggering"}}]}}"#
)]
fn the_new_events_read_in_the_shape_of_the_hand_written_files(
    #[case] text: &str,
    #[case] json: &str,
) {
    reads_as(text, ClauseKind::Triggered, false, json);
}

#[rstest]
#[case("Whenever equipped creature attacks, draw a card.")]
#[case("Whenever enchanted creature dies, draw a card.")]
#[case("Whenever equipped creature becomes tapped, you gain 1 life.")]
#[case("Whenever one or more creatures attack, draw a card.")]
#[case("Whenever two or more creatures you control attack, draw a card.")]
#[case("Whenever one or more creatures deal combat damage to an opponent, draw a card.")]
#[case("Whenever you cast your first spell each turn, draw a card.")]
#[case("Whenever you draw your third card each turn, you gain 1 life.")]
#[case("Whenever you gain life, draw that many cards.")]
#[case("Whenever you gain life, put that many +1/+1 counters on ~.")]
#[case("Whenever ~ deals combat damage to a player, you gain that much life.")]
#[case("Whenever ~ deals damage to an opponent, ~ deals that much damage to target creature.")]
fn more_of_the_new_events_read_and_write_back(#[case] text: &str) {
    golden(text, ClauseKind::Triggered, false);
}

#[test]
fn a_trigger_given_in_quotes_names_the_amount_of_its_event() {
    golden(
        "Sliver creatures you control have \"Whenever this creature is dealt damage, it deals that much damage to target player or planeswalker.\"",
        ClauseKind::Static,
        false,
    );
}

#[rstest]
#[case::life_gained_after_a_loss_in_a_spell(
    "Target opponent loses 1 life for each attacking creature you control. You gain that much life.",
    ClauseKind::Spell
)]
#[case::life_gained_after_damage_in_a_spell(
    "~ deals 3 damage to target creature. You gain that much life.",
    ClauseKind::Spell
)]
#[case::cards_drawn_in_a_spell("Exile target creature. Draw that many cards.", ClauseKind::Spell)]
#[case::life_gained_in_an_activated_ability(
    "{T}: Target opponent loses 2 life. You gain that much life.",
    ClauseKind::Activated
)]
#[case::cards_drawn_in_an_activated_ability(
    "{2}, {T}: Draw that many cards.",
    ClauseKind::Activated
)]
#[case::counters_put_in_an_activated_ability(
    "{1}{G}: Put that many +1/+1 counters on ~.",
    ClauseKind::Activated
)]
fn only_a_trigger_has_an_event_for_that_much_and_that_many_to_name(
    #[case] text: &str,
    #[case] kind: ClauseKind,
) {
    assert_eq!(clause(text, kind, false), None, "{text}");
}

#[rstest]
#[case::a_creature_that_is_not_attached("Whenever the creature dies, draw a card.")]
#[case::an_equipped_permanent("Whenever equipped permanent dies, draw a card.")]
#[case::one_of_two_in_a_batch(
    "Whenever two or more creatures you control deal combat damage to a player, draw a card."
)]
#[case::a_count_of_a_target_of_an_opponent(
    "Whenever ~ becomes the target of a spell or ability an opponent controls for the first time each turn, draw a card."
)]
#[case::a_count_in_a_list_of_events(
    "Whenever ~ attacks for the first time each turn or dies, draw a card."
)]
#[case::a_sixth_attack("Whenever ~ attacks for the sixth time each turn, draw a card.")]
#[case::a_count_of_a_spell_target(
    "Whenever ~ becomes the target of a spell for the first time each turn, draw a card."
)]
#[case::a_batch_of_a_cast("Whenever one or more spells are cast, draw a card.")]
#[case::the_nth_of_something_else("Whenever you gain your second life each turn, draw a card.")]
#[case::a_sixth("Whenever you draw your sixth card each turn, draw a card.")]
fn other_wordings_of_the_new_events_stay_unread(#[case] text: &str) {
    assert_eq!(clause(text, ClauseKind::Triggered, false), None, "{text}");
}
