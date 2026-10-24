//! Goal: the creatures that died under a player's control this turn ("a creature died under your control this turn", "for each creature that died under your control this turn") and a group that enters with additional counters ("Nontoken creatures you control enter with an additional +1/+1 counter on them for each ...") read into a controller-keyed tally and a replacement of the entering of the group, write back as the same text, and leave every other count, player and group to the table.
use super::*;

const DIED_UNDER: &str = r#"{"Tallied":{"who":"You","what":"CreaturesDiedUnder"}}"#;

#[rstest]
#[case::a_creature(
    "At the beginning of your end step, if a creature died under your control this turn, draw a card.",
    "CreaturesDiedUnder",
    1
)]
#[case::a_nontoken_creature(
    "At the beginning of your end step, if a nontoken creature died under your control this turn, draw a card.",
    "NontokenCreaturesDiedUnder",
    1
)]
#[case::two_or_more(
    "At the beginning of your end step, if two or more creatures died under your control this turn, draw a card.",
    "CreaturesDiedUnder",
    2
)]
fn a_creature_that_died_under_your_control_is_an_intervening_if(
    #[case] text: &str,
    #[case] tally: &str,
    #[case] number: u32,
) {
    let json = format!(
        r#"{{"Triggered":{{"when":{{"StepBegins":{{"step":"End","whose":"You"}}}},"if":{{"Compare":{{"left":{{"Tallied":{{"who":"You","what":"{tally}"}}}},"op":"Ge","right":{number}}}}},"effects":[{{"Draw":{{"who":"You","count":1}}}}]}}}}"#
    );
    reads_as(text, ClauseKind::Triggered, false, &json);
}

#[rstest]
#[case::for_each(
    "Draw a card for each creature that died under your control this turn.",
    ClauseKind::Spell
)]
#[case::the_number_of(
    "Draw X cards, where X is the number of creatures that died under your control this turn.",
    ClauseKind::Spell
)]
#[case::nontoken(
    "Draw a card for each nontoken creature that died under your control this turn.",
    ClauseKind::Spell
)]
fn the_creatures_that_died_under_your_control_are_counted(
    #[case] text: &str,
    #[case] kind: ClauseKind,
) {
    let read = golden(text, kind, false);
    let json = serde_json::to_string(&read).unwrap_or_default();
    assert!(json.contains("DiedUnder"), "{json}");
}

#[rstest]
#[case::an_opponent(
    "Draw a card for each creature that died under an opponent's control this turn."
)]
#[case::their_control("Draw a card for each creature that died under their control this turn.")]
#[case::a_land("Draw a card for each land that died under your control this turn.")]
fn the_creatures_that_died_under_another_player_stay_with_the_table(#[case] text: &str) {
    assert_eq!(spell(text), None, "{text}");
}

#[rstest]
#[case::gorma(
    "Nontoken creatures you control enter with an additional +1/+1 counter on them for each creature that died under your control this turn.",
    r#"[{"Type":"Creature"},{"Not":"Token"},{"ControlledBy":"You"}]"#,
    DIED_UNDER
)]
#[case::each_other(
    "Each other creature you control enters with an additional +1/+1 counter on it.",
    r#"[{"Type":"Creature"},"Other",{"ControlledBy":"You"}]"#,
    "1"
)]
#[case::two_counters(
    "Creatures you control enter with two additional +1/+1 counters on them.",
    r#"[{"Type":"Creature"},{"ControlledBy":"You"}]"#,
    "2"
)]
fn a_group_enters_with_additional_counters(
    #[case] text: &str,
    #[case] what: &str,
    #[case] count: &str,
) {
    let json = format!(
        r#"{{"Replacement":{{"when":{{"Enters":{{"what":{what}}}}},"by":{{"WithCounters":{{"counter":"+1/+1","count":{count}}}}}}}}}"#
    );
    reads_as(text, ClauseKind::Static, false, &json);
}

#[rstest]
#[case::a_spell("Creature spells you cast enter with an additional +1/+1 counter on them.")]
#[case::cards_in_a_zone(
    "Creature cards in your graveyard enter with an additional +1/+1 counter on them."
)]
#[case::not_additional("Creatures you control enter with a +1/+1 counter on them.")]
#[case::two_for_each(
    "Creatures you control enter with two additional +1/+1 counters on them for each land you control."
)]
#[case::the_wrong_pronoun("Creatures you control enter with an additional +1/+1 counter on it.")]
fn a_group_entering_that_is_not_the_shape_stays_with_the_table(#[case] text: &str) {
    assert_eq!(clause(text, ClauseKind::Static, false), None, "{text}");
}
