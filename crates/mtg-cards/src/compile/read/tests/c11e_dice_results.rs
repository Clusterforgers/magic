//! Goal: a result of a roll reads as an amount ("the result" of one die, "that result" and "the other result" of two dice with a choice, in a where clause or after "equal to", the other result in a sentence that opens with "Then"), "whenever you roll one or more dice" and "a die's highest natural result" read as roll triggers, all write back as the same text, and near-misses (three dice, two dice without a choice, a result with no roll word, another roll event) stay with the table.
use super::*;

#[rstest]
#[case::scry_the_result(
    "At the beginning of your upkeep, roll a d4. Scry X, where X is the result.",
    ClauseKind::Triggered,
    r#"{"Triggered":{"when":{"StepBegins":{"step":"Upkeep","whose":"You"}},"effects":[{"RollDice":{"sides":4,"results":[]}},{"Scry":{"count":{"DieResult":"Only"}}}]}}"#
)]
#[case::damage_equal_to_the_result(
    "Whenever enchanted player casts a noncreature spell, roll a d6. This Aura deals damage to that player equal to the result.",
    ClauseKind::Triggered,
    r#"{"Triggered":{"when":{"SpellCast":{"what":[{"Not":{"Type":"Creature"}}],"by":"AttachedPlayer"}},"effects":[{"RollDice":{"sides":6,"results":[]}},{"DealDamage":{"to":"Triggering","amount":{"DieResult":"Only"}}}]}}"#
)]
#[case::draw_the_result(
    "{T}: Roll a d6. Draw cards equal to the result.",
    ClauseKind::Activated,
    r#"{"Activated":{"cost":["Tap"],"effects":[{"RollDice":{"sides":6,"results":[]}},{"Draw":{"count":{"DieResult":"Only"}}}]}}"#
)]
#[case::two_dice_each_result(
    "Roll two d12 and choose one result. ~ deals damage equal to that result to each creature. Then create a number of Treasure tokens equal to the other result.",
    ClauseKind::Spell,
    r#"{"Spell":{"effects":[{"RollDice":{"sides":12,"dice":2,"choose":true,"results":[]}},{"DealDamage":{"to":{"Each":[{"Type":"Creature"}]},"amount":{"DieResult":"Chosen"}}},{"CreateToken":{"token":{"Named":"treasure"},"count":{"DieResult":"Other"}}}]}}"#
)]
#[case::the_other_result_without_then(
    "Roll two d6 and choose one result. Draw cards equal to that result. You gain life equal to the other result.",
    ClauseKind::Spell,
    r#"{"Spell":{"effects":[{"RollDice":{"sides":6,"dice":2,"choose":true,"results":[]}},{"Draw":{"count":{"DieResult":"Chosen"}}},{"GainLife":{"amount":{"DieResult":"Other"}}}]}}"#
)]
#[case::the_other_result_in_a_where_clause(
    "Roll two d10 and choose one result. Draw cards equal to that result. Then each opponent loses X life and you gain X life, where X is the other result.",
    ClauseKind::Spell,
    r#"{"Spell":{"effects":[{"RollDice":{"sides":10,"dice":2,"choose":true,"results":[]}},{"Draw":{"count":{"DieResult":"Chosen"}}},{"LoseLife":{"who":"Opponents","amount":{"DieResult":"Other"}}},{"GainLife":{"amount":{"DieResult":"Other"}}}]}}"#
)]
#[case::roll_one_or_more_dice(
    "Whenever you roll one or more dice, target creature you control gets +2/+0 and gains menace until end of turn.",
    ClauseKind::Triggered,
    r#"{"Triggered":{"when":{"DiceRolled":{"who":"You"}},"targets":[{"what":{"Object":[{"Type":"Creature"},{"ControlledBy":"You"}]}}],"effects":[{"Continuous":{"affected":{"Target":0},"mods":[{"ModifyPT":{"power":2,"toughness":0}},{"AddKeyword":"Menace"}],"duration":"UntilEndOfTurn"}}]}}"#
)]
#[case::the_highest_natural_result(
    "Whenever you roll a die's highest natural result, draw a card.",
    ClauseKind::Triggered,
    r#"{"Triggered":{"when":{"DiceRolled":{"who":"You","highest":true}},"effects":[{"Draw":{}}]}}"#
)]
#[case::an_opponent_rolls(
    "Whenever an opponent rolls one or more dice, you gain 1 life.",
    ClauseKind::Triggered,
    r#"{"Triggered":{"when":{"DiceRolled":{"who":"Opponents"}},"effects":[{"GainLife":{"amount":1}}]}}"#
)]
fn c11e_a_result_reads_as_an_amount_and_a_roll_as_a_trigger(
    #[case] text: &str,
    #[case] kind: ClauseKind,
    #[case] json: &str,
) {
    reads_as(text, kind, false, json);
}

#[rstest]
#[case::three_dice("Roll three d6 and choose one result. Draw cards equal to that result.")]
#[case::two_dice_without_a_choice("Roll two d6. Draw cards equal to the result.")]
#[case::two_dice_choosing_two(
    "Roll two d6 and choose two results. Draw cards equal to that result."
)]
#[case::a_result_word_alone("Draw cards equal to the results.")]
#[case::then_before_the_first_sentence("Then draw a card.")]
fn c11e_a_near_miss_stays_unread(#[case] text: &str) {
    assert!(clause(text, ClauseKind::Spell, false).is_none(), "{text}");
}

#[rstest]
#[case::a_die_lower_than("Whenever you roll a 4 or higher, draw a card.")]
#[case::a_coin_and_a_die("Whenever you roll a die or flip a coin, draw a card.")]
#[case::lowest("Whenever you roll a die's lowest natural result, draw a card.")]
fn c11e_another_roll_event_stays_unread(#[case] text: &str) {
    assert!(
        clause(text, ClauseKind::Triggered, false).is_none(),
        "{text}"
    );
}
