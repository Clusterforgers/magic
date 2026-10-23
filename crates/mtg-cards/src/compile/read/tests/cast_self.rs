//! Goal: "When you cast this spell" reads as a trigger of the card on the stack (a `SpellCast` of the card itself, working from the stack), "copy it" in it names the card, the copy count may be the commander casts, the wording is written back, and the near-misses (another player, another spell, a later zone) stay with the table.
use super::*;
use crate::effect::Effect;
use crate::refs::{Ref, ZoneKind};

fn trigger(text: &str) -> Option<Ability> {
    clause(text, ClauseKind::Triggered, false)
}

const SELF_CAST: &str = r#""when":{"SpellCast":{"what":["This"],"by":"You"}},"from":"Stack""#;

#[rstest]
#[case::draw("When you cast this spell, draw a card.", r#"{"Draw":{}}"#)]
#[case::life(
    "When you cast this spell, you gain 2 life.",
    r#"{"GainLife":{"amount":2}}"#
)]
#[case::copy_it(
    "When you cast this spell, copy it. You may choose new targets for the copy.",
    r#"{"CopySpell":{"what":"This","new_targets":true}}"#
)]
#[case::copy_it_twice(
    "When you cast this spell, copy it twice.",
    r#"{"CopySpell":{"what":"This","count":2}}"#
)]
#[case::the_commander_casts(
    "When you cast this spell, copy it for each time you've cast your commander from the command zone this game. You may choose new targets for the copies.",
    r#"{"CopySpell":{"what":"This","count":{"CommanderCasts":"You"},"new_targets":true}}"#
)]
#[case::the_commander_casts_without_new_targets(
    "When you cast this spell, copy it for each time you've cast your commander from the command zone this game.",
    r#"{"CopySpell":{"what":"This","count":{"CommanderCasts":"You"}}}"#
)]
fn the_card_cast_triggers_from_the_stack(#[case] text: &str, #[case] effect: &str) {
    reads_as(
        text,
        ClauseKind::Triggered,
        false,
        &format!(r#"{{"Triggered":{{{SELF_CAST},"effects":[{effect}]}}}}"#),
    );
}

#[test]
fn this_spell_and_the_card_name_are_one_wording() {
    let text = "When you cast ~, draw a card.";
    assert!(trigger(text).is_some());
    assert_eq!(
        trigger(text),
        trigger("When you cast this spell, draw a card.")
    );
}

#[rstest]
#[case::an_opponent_casts_it("When an opponent casts this spell, draw a card.")]
#[case::a_player_casts_it("When a player casts this spell, draw a card.")]
#[case::another_spell("When you cast another spell, draw a card.")]
#[case::from_a_zone("When you cast this spell from your hand, draw a card.")]
#[case::it_is_not_the_cast("When you cast this spell, copy that card.")]
#[case::no_effect("When you cast this spell,")]
fn near_misses_stay_with_the_table(#[case] text: &str) {
    assert_eq!(trigger(text), None, "{text}");
}

#[test]
fn the_spell_of_another_cast_is_the_triggering_one() {
    let read = golden(
        "Whenever you cast a spell, copy it for each time you've cast your commander from the command zone this game.",
        ClauseKind::Triggered,
        false,
    );
    let Ability::Triggered { from, effects, .. } = read else {
        panic!("not a trigger");
    };
    assert_eq!(from, ZoneKind::Battlefield);
    assert!(matches!(
        effects.first(),
        Some(Effect::CopySpell {
            what: Ref::Triggering,
            ..
        })
    ));
}
