//! Goal: a spell copied ("you copy a spell", "you cast or copy an instant or sorcery spell") and an attacker that isn't blocked read as trigger events in the shape of the hand-written files, write back as the card's own text, and their near-misses stay with the table.
use super::*;

fn trigger(text: &str) -> Option<Ability> {
    clause(text, ClauseKind::Triggered, false)
}

#[rstest]
#[case::you_copy_a_spell(
    "Whenever you copy a spell, put a +1/+1 counter on ~.",
    r#"{"Triggered":{"when":{"SpellCopied":{"by":"You"}},"effects":[{"PutCounters":{"on":"This","counter":"+1/+1"}}]}}"#
)]
#[case::you_copy_an_instant(
    "Whenever you copy an instant spell, put a +1/+1 counter on ~.",
    r#"{"Triggered":{"when":{"SpellCopied":{"what":[{"Type":"Instant"}],"by":"You"}},"effects":[{"PutCounters":{"on":"This","counter":"+1/+1"}}]}}"#
)]
#[case::magecraft(
    "Whenever you cast or copy an instant or sorcery spell, draw a card.",
    r#"{"Triggered":{"when":{"AnyOf":[{"SpellCast":{"what":[{"AnyOf":[{"Type":"Instant"},{"Type":"Sorcery"}]}],"by":"You"}},{"SpellCopied":{"what":[{"AnyOf":[{"Type":"Instant"},{"Type":"Sorcery"}]}],"by":"You"}}]},"effects":[{"Draw":{}}]}}"#
)]
#[case::this_attacks_and_isnt_blocked(
    "Whenever ~ attacks and isn't blocked, draw a card.",
    r#"{"Triggered":{"when":{"Unblocked":{"what":["This"]}},"effects":[{"Draw":{}}]}}"#
)]
fn copies_and_unblocked_attackers_read_in_the_shape_of_the_hand_written_files(
    #[case] text: &str,
    #[case] json: &str,
) {
    reads_as(text, ClauseKind::Triggered, false, json);
}

#[rstest]
#[case("Whenever an opponent copies a spell, you gain 1 life.")]
#[case("Whenever a player copies a spell, that player loses 1 life.")]
#[case("Whenever an opponent casts or copies an instant or sorcery spell, you gain 1 life.")]
#[case("Whenever you cast or copy a spell, scry 1.")]
#[case("Whenever a Rogue you control attacks and isn't blocked, it gets +1/+1 until end of turn.")]
#[case("Whenever a creature you control attacks and isn't blocked, you gain 1 life.")]
#[case("Whenever enchanted creature attacks and isn't blocked, draw a card.")]
fn more_copies_and_unblocked_attackers_read_and_write_back(#[case] text: &str) {
    golden(text, ClauseKind::Triggered, false);
}

#[rstest]
#[case::copy_another_spell("Whenever you copy another spell, draw a card.")]
#[case::copy_a_permanent("Whenever you copy a creature, draw a card.")]
#[case::copy_without_an_object("Whenever you copy, draw a card.")]
#[case::cast_or_copy_twice("Whenever you cast or copy or copy a spell, draw a card.")]
#[case::copy_or_cast("Whenever you copy or cast a spell, draw a card.")]
#[case::unblocked_this_combat("Whenever a creature attacks and isn't blocked this combat, draw a card.")]
#[case::unblocked_attacking_a_player("Whenever ~ attacks a player and isn't blocked, draw a card.")]
#[case::not_blocked_wording("Whenever ~ attacks and is not blocked, draw a card.")]
fn other_wordings_of_copies_and_unblocked_attackers_stay_unread(#[case] text: &str) {
    assert_eq!(trigger(text), None, "{text}");
}

#[test]
fn it_in_an_unblocked_trigger_is_the_attacker() {
    let text = "Whenever a Rogue you control attacks and isn't blocked, it gets +1/+1 until end of turn.";
    let Some(Ability::Triggered { effects, .. }) = trigger(text) else {
        panic!("not read: {text}");
    };
    let written = serde_json::to_string(&effects).unwrap_or_default();
    assert!(written.contains("Triggering"), "{written}");
}
