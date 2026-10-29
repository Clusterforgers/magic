//! Goal: a permanent taken for the turn reads and writes back as the card words it (the untap before the taking, the keywords it gains, the size and the type it gets after the untap), and a near-miss stays with the table.
use super::*;

#[rstest]
#[case::untap_first_with_haste(
    "Untap target permanent and gain control of it until end of turn. It gains haste until end of turn.",
    ClauseKind::Spell
)]
#[case::untap_first_alone(
    "Untap target creature and gain control of it until end of turn.",
    ClauseKind::Spell
)]
#[case::untap_first_two_keywords(
    "Untap target creature an opponent controls and gain control of it until end of turn. It gains trample and haste until end of turn.",
    ClauseKind::Spell
)]
#[case::size_and_keywords(
    "When you cast this spell, gain control of target creature until end of turn. Untap that creature. Until end of turn, it has base power and toughness 10/10 and gains trample, annihilator 2, and haste.",
    ClauseKind::Triggered
)]
#[case::type_in_addition(
    "Whenever this creature or another Pirate you control enters, gain control of target creature until end of turn. Untap that creature. Until end of turn, it gains haste and becomes a Pirate in addition to its other types.",
    ClauseKind::Triggered
)]
#[case::size_alone(
    "When this creature enters, gain control of target creature until end of turn. Untap that creature. Until end of turn, it has base power and toughness 1/1.",
    ClauseKind::Triggered
)]
#[case::a_type_with_a_vowel(
    "When this creature enters, gain control of target creature until end of turn. Untap that creature. Until end of turn, it becomes an Elf in addition to its other types.",
    ClauseKind::Triggered
)]
fn w20c_a_taken_permanent_reads_and_writes_back(#[case] text: &str, #[case] kind: ClauseKind) {
    golden(text, kind, false);
}

#[test]
fn w20c_the_steps_read_into_the_dsl() {
    reads_as(
        "Untap target permanent and gain control of it until end of turn. It gains haste until end of turn.",
        ClauseKind::Spell,
        false,
        r#"{"Spell":{"targets":[{"what":{"Object":[]}}],"effects":[{"Untap":{"what":{"Target":0}}},{"Continuous":{"affected":{"Target":0},"mods":[{"SetController":"You"}],"duration":"UntilEndOfTurn"}},{"Continuous":{"affected":{"Target":0},"mods":[{"AddKeyword":"Haste"}],"duration":"UntilEndOfTurn"}}]}}"#,
    );
    reads_as(
        "When this creature enters, gain control of target creature until end of turn. Untap that creature. Until end of turn, it gains haste and becomes a Pirate in addition to its other types.",
        ClauseKind::Triggered,
        false,
        r#"{"Triggered":{"when":{"ZoneChange":{"what":["This"],"to":"Battlefield"}},"targets":[{"what":{"Object":[{"Type":"Creature"}]}}],"effects":[{"Continuous":{"affected":{"Target":0},"mods":[{"SetController":"You"}],"duration":"UntilEndOfTurn"}},{"Untap":{"what":{"Target":0}}},{"Continuous":{"affected":{"Target":0},"mods":[{"AddKeyword":"Haste"},{"AddSubtype":"Pirate"}],"duration":"UntilEndOfTurn"}}]}}"#,
    );
}

#[rstest]
#[case::untap_first_for_good(
    "Untap target permanent and gain control of it. It gains haste until end of turn.",
    ClauseKind::Spell
)]
#[case::untap_first_for_the_next_turn(
    "Untap target permanent and gain control of it until your next turn.",
    ClauseKind::Spell
)]
#[case::untap_first_two_targets(
    "Untap target permanent and target creature and gain control of it until end of turn.",
    ClauseKind::Spell
)]
#[case::untap_first_other_pronoun(
    "Untap target permanent and gain control of it until end of turn. They gain haste until end of turn.",
    ClauseKind::Spell
)]
#[case::tail_for_the_next_turn(
    "When this creature enters, gain control of target creature until end of turn. Untap that creature. Until your next turn, it has base power and toughness 1/1.",
    ClauseKind::Triggered
)]
#[case::tail_for_the_creature_not_it(
    "When this creature enters, gain control of target creature until end of turn. Untap that creature. Until end of turn, that creature has base power and toughness 1/1.",
    ClauseKind::Triggered
)]
#[case::tail_of_a_loss(
    "When this creature enters, gain control of target creature until end of turn. Untap that creature. Until end of turn, it loses all abilities and has base power and toughness 1/1.",
    ClauseKind::Triggered
)]
#[case::tail_without_the_untap(
    "When this creature enters, gain control of target creature until end of turn. Until end of turn, it has base power and toughness 1/1.",
    ClauseKind::Triggered
)]
#[case::tail_with_a_negative_size(
    "When this creature enters, gain control of target creature until end of turn. Untap that creature. Until end of turn, it has base power and toughness -1/1.",
    ClauseKind::Triggered
)]
#[case::type_without_the_addition(
    "When this creature enters, gain control of target creature until end of turn. Untap that creature. Until end of turn, it becomes a Pirate.",
    ClauseKind::Triggered
)]
fn w20c_a_near_miss_stays_unread(#[case] text: &str, #[case] kind: ClauseKind) {
    assert_eq!(clause(text, kind, false), None, "{text}");
}
