//! Goal: an event of a group (one or more things enter, die, leave a zone, are put into a graveyard, attack) is written as the card words it, "Whenever you attack with ..." and "each of them" included, and another count, kind of thing, zone or owner is not its text.
use super::*;

/// Whether one of the wordings of the ability is the text.
fn writes(json: &str, text: &str) -> bool {
    render_variants(&ability(json))
        .iter()
        .any(|written| same_text(written, text))
}

/// A trigger on an event with the effects.
fn trigger(when: &str, effects: &str) -> String {
    format!(r#"{{"Triggered":{{"when":{when},"effects":{effects}}}}}"#)
}

const DRAW: &str = r#"[{"Draw":{}}]"#;
const BIRDS: &str = r#"{"GroupAttacks":{"what":[{"Subtype":"Bird"},{"ControlledBy":"You"}]}}"#;
const CREATURES: &str =
    r#"{"GroupAttacks":{"what":[{"Type":"Creature"},{"ControlledBy":"You"}],"at_least":2}}"#;
const FLYERS: &str = r#"{"GroupAttacks":{"what":[{"Type":"Creature"},{"ControlledBy":"You"},{"HasKeyword":"Flying"}],"at_least":2}}"#;
const ANY_ATTACK: &str =
    r#"{"GroupAttacks":{"what":[{"Type":"Creature"},{"ControlledBy":"You"}]}}"#;
const INSECTS_GET_COUNTERS: &str = r#"[{"PutCounters":{"on":{"Each":[{"Subtype":"Insect"},{"ControlledBy":"You"},"Attacking"]},"counter":"+1/+1"}}]"#;

#[rstest]
#[case::hermes_attacks_with_birds(
    BIRDS,
    r#"[{"Scry":{"count":2}}]"#,
    "Whenever you attack with one or more Birds, scry 2."
)]
#[case::birds_attack(
    BIRDS,
    r#"[{"Scry":{"count":2}}]"#,
    "Whenever one or more Birds you control attack, scry 2."
)]
#[case::tide_skimmer(
    FLYERS,
    DRAW,
    "Whenever you attack with two or more creatures with flying, draw a card."
)]
#[case::firemane_commando(
    CREATURES,
    DRAW,
    "Whenever you attack with two or more creatures, draw a card."
)]
#[case::temmet_attacks(ANY_ATTACK, DRAW, "Whenever you attack, draw a card.")]
#[case::vrestin_each_of_them(
    r#"{"GroupAttacks":{"what":[{"Subtype":"Insect"},{"ControlledBy":"You"}]}}"#,
    INSECTS_GET_COUNTERS,
    "Whenever you attack with one or more Insects, put a +1/+1 counter on each of them."
)]
#[case::light_party_those_creatures(
    r#"{"GroupAttacks":{"what":[{"Type":"Creature"},{"ControlledBy":"You"}],"at_least":8}}"#,
    r#"[{"Continuous":{"affected":{"Each":[{"Type":"Creature"},{"ControlledBy":"You"},"Attacking"]},"mods":[{"ModifyPT":{"power":4,"toughness":4}}],"duration":"UntilEndOfTurn"}}]"#,
    "Whenever you attack with eight or more creatures, those creatures get +4/+4 until end of turn."
)]
fn the_attackers_of_a_group_are_written_as_the_card_words_them(
    #[case] when: &str,
    #[case] effects: &str,
    #[case] text: &str,
) {
    let json = trigger(when, effects);
    assert!(
        writes(&json, text),
        "{text}: {:?}",
        render_variants(&ability(&json))
    );
}

#[rstest]
#[case::thopter_shop(
    r#"{"Batch":{"ZoneChange":{"what":[{"Type":"Artifact"},{"Type":"Creature"},{"ControlledBy":"You"}],"from":"Battlefield","to":"Graveyard"}}}"#,
    "Whenever one or more artifact creatures you control die, draw a card."
)]
#[case::ingenious_smith(
    r#"{"Batch":{"ZoneChange":{"what":[{"Type":"Artifact"},{"ControlledBy":"You"}],"to":"Battlefield"}}}"#,
    "Whenever one or more artifacts you control enter, draw a card."
)]
#[case::elvish_warmaster(
    r#"{"Batch":{"ZoneChange":{"what":[{"Subtype":"Elf"},"Other",{"ControlledBy":"You"}],"to":"Battlefield"}}}"#,
    "Whenever one or more other Elves you control enter, draw a card."
)]
#[case::marneus_calgar(
    r#"{"Batch":{"ZoneChange":{"what":["Token",{"ControlledBy":"You"}],"to":"Battlefield"}}}"#,
    "Whenever one or more tokens you control enter, draw a card."
)]
#[case::welcoming_vampire(
    r#"{"Batch":{"ZoneChange":{"what":[{"Type":"Creature"},"Other",{"ControlledBy":"You"},{"Compare":{"stat":"Power","op":"Le","to":2}}],"to":"Battlefield"}}}"#,
    "Whenever one or more other creatures you control with power 2 or less enter, draw a card."
)]
#[case::creatures_leave_the_battlefield(
    r#"{"Batch":{"ZoneChange":{"what":[{"Type":"Creature"},{"ControlledBy":"You"}],"from":"Battlefield"}}}"#,
    "Whenever one or more creatures you control leave the battlefield, draw a card."
)]
#[case::quintorius_cards_leave(
    r#"{"Batch":{"ZoneChange":{"what":[{"OwnedBy":"You"}],"from":"Graveyard"}}}"#,
    "Whenever one or more cards leave your graveyard, draw a card."
)]
#[case::skeleton_crew_creature_cards_leave(
    r#"{"Batch":{"ZoneChange":{"what":[{"Type":"Creature"},{"OwnedBy":"You"}],"from":"Graveyard"}}}"#,
    "Whenever one or more creature cards leave your graveyard, draw a card."
)]
#[case::turntimber_sower_from_anywhere(
    r#"{"Batch":{"ZoneChange":{"what":[{"Type":"Land"},{"OwnedBy":"You"}],"to":"Graveyard"}}}"#,
    "Whenever one or more land cards are put into your graveyard from anywhere, draw a card."
)]
#[case::unshakable_tail_from_the_library(
    r#"{"Batch":{"ZoneChange":{"what":[{"Type":"Creature"},{"OwnedBy":"You"}],"from":"Library","to":"Graveyard"}}}"#,
    "Whenever one or more creature cards are put into your graveyard from your library, draw a card."
)]
fn a_batch_of_moves_is_written_as_the_card_words_it(#[case] when: &str, #[case] text: &str) {
    let json = trigger(when, DRAW);
    assert!(
        writes(&json, text),
        "{text}: {:?}",
        render_variants(&ability(&json))
    );
}

#[rstest]
#[case::once_each_turn(
    r#"{"Triggered":{"when":{"Batch":{"ZoneChange":{"what":[{"Type":"Artifact"},{"Type":"Creature"},{"ControlledBy":"You"}],"from":"Battlefield","to":"Graveyard"}}},"limit":"OncePerTurn","effects":[{"Draw":{}}]}}"#,
    "Whenever one or more artifact creatures you control die, draw a card. This ability triggers only once each turn."
)]
fn a_batch_may_trigger_once_each_turn(#[case] json: &str, #[case] text: &str) {
    assert!(writes(json, text), "{text}");
}

#[rstest]
#[case::two_for_one(BIRDS, "Whenever you attack with two or more Birds, draw a card.")]
#[case::another_kind(BIRDS, "Whenever you attack with one or more Insects, draw a card.")]
#[case::creatures_for_birds(BIRDS, "Whenever you attack, draw a card.")]
#[case::three_for_two(
    CREATURES,
    "Whenever you attack with three or more creatures, draw a card."
)]
#[case::flying_lost(FLYERS, "Whenever you attack with two or more creatures, draw a card.")]
#[case::an_opponent_attacks(
    BIRDS,
    "Whenever an opponent attacks with one or more Birds, draw a card."
)]
#[case::two_creatures_attack_is_not_any_attack(CREATURES, "Whenever you attack, draw a card.")]
fn another_attack_is_not_the_text(#[case] when: &str, #[case] text: &str) {
    let json = trigger(when, DRAW);
    assert!(!writes(&json, text), "{text}");
}

#[rstest]
#[case::enter_for_die(
    r#"{"Batch":{"ZoneChange":{"what":[{"Type":"Artifact"},{"ControlledBy":"You"}],"to":"Battlefield"}}}"#,
    "Whenever one or more artifacts you control die, draw a card."
)]
#[case::an_opponents_artifacts(
    r#"{"Batch":{"ZoneChange":{"what":[{"Type":"Artifact"},{"ControlledBy":"Opponents"}],"to":"Battlefield"}}}"#,
    "Whenever one or more artifacts you control enter, draw a card."
)]
#[case::the_graveyard_for_anywhere(
    r#"{"Batch":{"ZoneChange":{"what":[{"Type":"Land"},{"OwnedBy":"You"}],"from":"Graveyard"}}}"#,
    "Whenever one or more land cards are put into your graveyard from anywhere, draw a card."
)]
#[case::lands_for_creatures(
    r#"{"Batch":{"ZoneChange":{"what":[{"Type":"Land"},{"OwnedBy":"You"}],"to":"Graveyard"}}}"#,
    "Whenever one or more creature cards are put into your graveyard from anywhere, draw a card."
)]
#[case::other_dropped(
    r#"{"Batch":{"ZoneChange":{"what":[{"Subtype":"Elf"},{"ControlledBy":"You"}],"to":"Battlefield"}}}"#,
    "Whenever one or more other Elves you control enter, draw a card."
)]
#[case::a_batch_is_not_one_thing(
    r#"{"Batch":{"ZoneChange":{"what":[{"Type":"Artifact"},{"ControlledBy":"You"}],"to":"Battlefield"}}}"#,
    "Whenever an artifact you control enters, draw a card."
)]
fn another_batch_is_not_the_text(#[case] when: &str, #[case] text: &str) {
    let json = trigger(when, DRAW);
    assert!(!writes(&json, text), "{text}");
}
