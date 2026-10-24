//! Goal: a permanent that doesn't untap reads into a static `DoesntUntap` (the card itself, or the creature an Aura or Equipment is on, under the monarch condition when the words give one), two sentences of the controller joined with "and" or ", then" read as the sentences in order, and each is written back as the card's text; the wordings of other players, steps and conditions stay with the table.
use super::*;

#[rstest]
#[case::the_artifact_itself(
    "This artifact doesn't untap during your untap step.",
    r#"{"Static":{"affected":"This","mods":["DoesntUntap"]}}"#
)]
#[case::the_creature_itself(
    "This creature doesn't untap during your untap step.",
    r#"{"Static":{"affected":"This","mods":["DoesntUntap"]}}"#
)]
#[case::an_enchanted_creature(
    "Enchanted creature doesn't untap during its controller's untap step.",
    r#"{"Static":{"affected":"Attached","mods":["DoesntUntap"]}}"#
)]
#[case::unless_its_controller_is_the_monarch(
    "Enchanted creature doesn't untap during its controller's untap step unless that player is the monarch.",
    r#"{"Static":{"affected":"Attached","mods":["DoesntUntap"],"condition":{"Not":{"Monarch":{"ControllerOf":"Attached"}}}}}"#
)]
#[case::unless_you_are_the_monarch(
    "This creature doesn't untap during your untap step unless you're the monarch.",
    r#"{"Static":{"affected":"This","mods":["DoesntUntap"],"condition":{"Not":{"Monarch":"You"}}}}"#
)]
fn a_permanent_that_stays_tapped(#[case] text: &str, #[case] json: &str) {
    reads_as(text, ClauseKind::Static, false, json);
}

#[test]
fn a_sentence_of_the_controller_joined_with_and() {
    reads_as(
        "When this Aura enters, tap enchanted creature and you become the monarch.",
        ClauseKind::Triggered,
        false,
        r#"{"Triggered":{"when":{"ZoneChange":{"what":["This"],"to":"Battlefield"}},"effects":[{"Tap":{"what":"Attached"}},{"BecomeMonarch":{}}]}}"#,
    );
}

#[test]
fn a_damage_spell_and_a_sentence_of_the_controller() {
    golden(
        "~ deals 3 damage to any target and you lose 3 life.",
        ClauseKind::Spell,
        false,
    );
}

#[rstest]
#[case::then_you_gain(
    "Whenever this creature becomes untapped, put two +1/+1 counters on it, then you gain 2 life."
)]
#[case::then_you_gain_and_draw(
    "Whenever this creature becomes untapped, put two +1/+1 counters on it, then you gain 2 life and draw a card."
)]
#[case::on_an_attack(
    "Whenever this creature attacks, put two +1/+1 counters on it, then you gain 2 life."
)]
fn a_then_sentence_of_the_controller(#[case] text: &str) {
    golden(text, ClauseKind::Triggered, false);
}

#[rstest]
#[case::the_next_untap_step("This creature doesn't untap during your next untap step.")]
#[case::its_controllers_step_for_the_card(
    "This creature doesn't untap during its controller's untap step."
)]
#[case::your_step_for_an_aura("Enchanted creature doesn't untap during your untap step.")]
#[case::another_condition(
    "Enchanted creature doesn't untap during its controller's untap step unless that player controls a Swamp."
)]
#[case::the_wrong_monarch(
    "Enchanted creature doesn't untap during its controller's untap step unless you're the monarch."
)]
#[case::a_group("Creatures you control don't untap during your untap step.")]
fn near_misses_stay_with_the_table(#[case] text: &str) {
    assert_eq!(clause(text, ClauseKind::Static, false), None, "{text}");
}

#[rstest]
#[case::another_player(
    "When this Aura enters, tap enchanted creature and each opponent becomes the monarch."
)]
#[case::a_may("When this Aura enters, tap enchanted creature and you may become the monarch.")]
#[case::then_a_player_who_is_not_you(
    "Whenever this creature attacks, put two +1/+1 counters on it, then target player gains 2 life."
)]
fn joined_sentences_that_stay_with_the_table(#[case] text: &str) {
    assert_eq!(clause(text, ClauseKind::Triggered, false), None, "{text}");
}
