//! Goal: the fights of two creatures read into the DSL with the targets numbered in the order of the words (a first target that gets a counter or a pump before it fights, "another target" as other than the first target), a creature that fights by itself in a trigger reads as the card, and each is written back as the card's text.
use super::*;

const YOURS: &str = r#"{"Type":"Creature"},{"ControlledBy":"You"}"#;
const THEIRS: &str = r#"{"Type":"Creature"},{"Not":{"ControlledBy":"You"}}"#;
const OPPONENTS: &str = r#"{"Type":"Creature"},{"ControlledBy":"Opponents"}"#;
const ANOTHER: &str = r#"{"Type":"Creature"},{"Not":{"Is":{"Target":0}}}"#;
const FIGHT: &str = r#"{"Fight":{"what":{"Target":0},"with":{"Target":1}}}"#;

fn spec(selector: &str) -> String {
    format!(r#"{{"what":{{"Object":[{selector}]}}}}"#)
}

fn up_to_one(selector: &str) -> String {
    format!(r#"{{"what":{{"Object":[{selector}]}},"bounds":{{"min":0,"max":1}}}}"#)
}

/// A spell that does `before` to its first target and then makes it fight the second.
fn spell_json(first: &str, second: &str, before: &str) -> String {
    format!(r#"{{"Spell":{{"targets":[{first},{second}],"effects":[{before}{FIGHT}]}}}}"#)
}

#[rstest]
#[case::you_control_and_you_do_not(
    "Target creature you control fights target creature you don't control.",
    spell_json(&spec(YOURS), &spec(THEIRS), "")
)]
#[case::an_opponent_controls(
    "Target creature you control fights target creature an opponent controls.",
    spell_json(&spec(YOURS), &spec(OPPONENTS), "")
)]
#[case::another_target(
    "Target creature fights another target creature.",
    spell_json(&spec(r#"{"Type":"Creature"}"#), &spec(ANOTHER), "")
)]
#[case::the_second_is_optional(
    "Target creature you control fights up to one target creature you don't control.",
    spell_json(&spec(YOURS), &up_to_one(THEIRS), "")
)]
#[case::a_counter_first(
    "Put a +1/+1 counter on target creature you control. Then that creature fights target creature you don't control.",
    spell_json(
        &spec(YOURS),
        &spec(THEIRS),
        r#"{"PutCounters":{"on":{"Target":0},"counter":"+1/+1"}},"#
    )
)]
#[case::a_counter_and_it(
    "Put a +1/+1 counter on target creature you control. Then it fights target creature an opponent controls.",
    spell_json(
        &spec(YOURS),
        &spec(OPPONENTS),
        r#"{"PutCounters":{"on":{"Target":0},"counter":"+1/+1"}},"#
    )
)]
#[case::a_pump_and_it(
    "Target creature you control gets +1/+2 until end of turn. It fights target creature you don't control.",
    spell_json(
        &spec(YOURS),
        &spec(THEIRS),
        r#"{"Continuous":{"affected":{"Target":0},"mods":[{"ModifyPT":{"power":1,"toughness":2}}],"duration":"UntilEndOfTurn"}},"#
    )
)]
#[case::a_sentence_after(
    "Target creature you control fights target creature you don't control. You gain 2 life.",
    spell_json(&spec(YOURS), &spec(THEIRS), "").replace(FIGHT, &format!("{FIGHT},{{\"GainLife\":{{\"amount\":2}}}}"))
)]
#[case::a_sentence_before(
    "Proliferate. Target creature you control fights up to one target creature you don't control.",
    spell_json(&spec(YOURS), &up_to_one(THEIRS), r#"{"Proliferate":{}},"#)
)]
fn two_targets_fight(#[case] text: &str, #[case] json: String) {
    reads_as(text, ClauseKind::Spell, false, &json);
}

#[test]
fn a_pump_by_x_and_an_optional_second_target() {
    let json = spell_json(
        &spec(YOURS),
        &up_to_one(THEIRS),
        r#"{"Continuous":{"affected":{"Target":0},"mods":[{"ModifyPT":{"power":"X","toughness":"X"}}],"duration":"UntilEndOfTurn"}},"#,
    );
    reads_as(
        "Target creature you control gets +X/+X until end of turn. Then it fights up to one target creature you don't control.",
        ClauseKind::Spell,
        true,
        &json,
    );
}

#[rstest]
#[case::the_card_fights_when_it_enters(
    "When ~ enters, it fights up to one target creature you don't control.",
    &up_to_one(THEIRS),
    r#"{"Fight":{"what":"This","with":{"Target":0}}}"#
)]
#[case::the_card_fights_by_name(
    "When ~ enters, ~ fights target creature an opponent controls.",
    &spec(OPPONENTS),
    r#"{"Fight":{"what":"This","with":{"Target":0}}}"#
)]
#[case::the_card_fights_another_creature(
    "When ~ enters, it fights another target creature.",
    &spec(r#"{"Type":"Creature"},"Other""#),
    r#"{"Fight":{"what":"This","with":{"Target":0}}}"#
)]
#[case::a_target_fights_when_it_enters(
    "When ~ enters, target creature you control fights up to one target creature you don't control.",
    &format!("{},{}", spec(YOURS), up_to_one(THEIRS)),
    FIGHT
)]
fn a_trigger_makes_creatures_fight(
    #[case] text: &str,
    #[case] targets: &str,
    #[case] effect: &str,
) {
    let json = format!(
        r#"{{"Triggered":{{"when":{{"ZoneChange":{{"what":["This"],"to":"Battlefield"}}}},"targets":[{targets}],"effects":[{effect}]}}}}"#
    );
    reads_as(text, ClauseKind::Triggered, false, &json);
}

#[rstest]
#[case::enrage(
    "Whenever ~ is dealt damage, it fights up to one target creature you don't control.",
    r#"{"Triggered":{"when":{"DealsDamage":{"source":[],"to":"This"}},"targets":[{"what":{"Object":[{"Type":"Creature"},{"Not":{"ControlledBy":"You"}}]},"bounds":{"min":0,"max":1}}],"effects":[{"Fight":{"what":"This","with":{"Target":0}}}]}}"#
)]
#[case::the_card_or_another_that_enters(
    "Whenever ~ or another Vampire you control enters, ~ fights up to one target creature an opponent controls.",
    r#"{"Triggered":{"when":{"AnyOf":[{"ZoneChange":{"what":["This"],"to":"Battlefield"}},{"ZoneChange":{"what":[{"Subtype":"Vampire"},"Other",{"ControlledBy":"You"}],"to":"Battlefield"}}]},"targets":[{"what":{"Object":[{"Type":"Creature"},{"ControlledBy":"Opponents"}]},"bounds":{"min":0,"max":1}}],"effects":[{"Fight":{"what":"This","with":{"Target":0}}}]}}"#
)]
#[case::the_creature_that_entered(
    "Whenever ~ or another creature you control enters, that creature fights target creature an opponent controls.",
    r#"{"Triggered":{"when":{"AnyOf":[{"ZoneChange":{"what":["This"],"to":"Battlefield"}},{"ZoneChange":{"what":[{"Type":"Creature"},"Other",{"ControlledBy":"You"}],"to":"Battlefield"}}]},"targets":[{"what":{"Object":[{"Type":"Creature"},{"ControlledBy":"Opponents"}]}}],"effects":[{"Fight":{"what":"Triggering","with":{"Target":0}}}]}}"#
)]
fn other_events_make_creatures_fight(#[case] text: &str, #[case] json: &str) {
    reads_as(text, ClauseKind::Triggered, false, json);
}

/// "It" after a sentence about the one target is that target, not the permanent of the event.
#[test]
fn it_after_a_counter_on_the_target_is_the_target() {
    reads_as(
        "Whenever a land you control enters, put two +1/+1 counters on target creature you control. It gains vigilance until end of turn.",
        ClauseKind::Triggered,
        false,
        r#"{"Triggered":{"when":{"ZoneChange":{"what":[{"Type":"Land"},{"ControlledBy":"You"}],"to":"Battlefield"}},"targets":[{"what":{"Object":[{"Type":"Creature"},{"ControlledBy":"You"}]}}],"effects":[{"PutCounters":{"on":{"Target":0},"counter":"+1/+1","count":2}},{"Continuous":{"affected":{"Target":0},"mods":[{"AddKeyword":"Vigilance"}],"duration":"UntilEndOfTurn"}}]}}"#,
    );
}

#[test]
fn it_after_an_untap_of_the_target_is_the_target() {
    reads_as(
        "Untap target creature you control. It gains vigilance until end of turn.",
        ClauseKind::Spell,
        false,
        r#"{"Spell":{"targets":[{"what":{"Object":[{"Type":"Creature"},{"ControlledBy":"You"}]}}],"effects":[{"Untap":{"what":{"Target":0}}},{"Continuous":{"affected":{"Target":0},"mods":[{"AddKeyword":"Vigilance"}],"duration":"UntilEndOfTurn"}}]}}"#,
    );
}

#[rstest]
#[case::a_creature_in_a_graveyard(
    "Target creature you control fights target creature card in a graveyard."
)]
#[case::three_targets(
    "Target creature you control fights target creature you don't control and another target creature."
)]
#[case::itself("Target creature fights itself.")]
#[case::each_creature("Target creature you control fights each creature you don't control.")]
#[case::it_without_a_sentence_about_a_target(
    "Draw a card. It fights target creature you don't control."
)]
#[case::a_pump_of_a_group_then_it(
    "Creatures you control get +1/+1 until end of turn. Then it fights target creature you don't control."
)]
fn near_misses_stay_with_the_table(#[case] text: &str) {
    for kind in [ClauseKind::Spell, ClauseKind::Triggered] {
        assert_eq!(clause(text, kind, false), None, "{text}");
    }
}
