//! Goal: "When you next cast <spell> [this turn], copy that spell ..." reads into a `DelayedTrigger` on that cast, once and for the turn when it says so, whose `CopySpell` copies the spell cast with its count, exceptions and new targets; it is written back as the card's text in spells and activated abilities, and every near-miss stays with the table.
use super::*;
use crate::effect::Effect;

const IS: &str = r#"[{"AnyOf":[{"Type":"Instant"},{"Type":"Sorcery"}]}]"#;

/// A spell ability whose one effect is the delayed copy of the next spell of `what`.
fn delayed(what: &str, this_turn: bool, copy: &str) -> String {
    let turn = if this_turn {
        r#","this_turn":true"#
    } else {
        ""
    };
    format!(
        r#"{{"Spell":{{"effects":[{{"DelayedTrigger":{{"when":{{"SpellCast":{{"what":{what},"by":"You"}}}}{turn},"effects":[{copy}]}}}}]}}}}"#
    )
}

#[rstest]
#[case::that_spell(
    "When you next cast an instant or sorcery spell this turn, copy that spell. You may choose new targets for the copy.",
    IS,
    true,
    r#"{"CopySpell":{"what":"Triggering","new_targets":true}}"#
)]
#[case::copy_it(
    "When you next cast an instant or sorcery spell this turn, copy it. You may choose new targets for the copy.",
    IS,
    true,
    r#"{"CopySpell":{"what":"Triggering","new_targets":true}}"#
)]
#[case::twice(
    "When you next cast an instant or sorcery spell this turn, copy that spell twice. You may choose new targets for the copies.",
    IS,
    true,
    r#"{"CopySpell":{"what":"Triggering","count":2,"new_targets":true}}"#
)]
#[case::without_new_targets(
    "When you next cast an instant or sorcery spell this turn, copy that spell.",
    IS,
    true,
    r#"{"CopySpell":{"what":"Triggering"}}"#
)]
#[case::commander_casts(
    "When you next cast an instant or sorcery spell this turn, copy it for each time you've cast your commander from the command zone this game. You may choose new targets for the copies.",
    IS,
    true,
    r#"{"CopySpell":{"what":"Triggering","count":{"CommanderCasts":"You"},"new_targets":true}}"#
)]
#[case::any_spell(
    "When you next cast a spell this turn, copy that spell.",
    "[]",
    true,
    r#"{"CopySpell":{"what":"Triggering"}}"#
)]
#[case::an_exception(
    "When you next cast a creature spell this turn, copy it, except the copy is red.",
    r#"[{"Type":"Creature"}]"#,
    true,
    r#"{"CopySpell":{"what":"Triggering","except":[{"SetColors":["Red"]}]}}"#
)]
fn the_next_spell_cast_is_copied(
    #[case] text: &str,
    #[case] what: &str,
    #[case] this_turn: bool,
    #[case] copy: &str,
) {
    reads_as(
        text,
        ClauseKind::Spell,
        false,
        &delayed(what, this_turn, copy),
    );
}

#[test]
fn a_count_of_x_reads_with_an_x_in_the_cost() {
    let text = "When you next cast an instant or sorcery spell this turn, copy that spell X times. You may choose new targets for the copies.";
    let json = delayed(
        IS,
        true,
        r#"{"CopySpell":{"what":"Triggering","count":"X","new_targets":true}}"#,
    );
    reads_as(text, ClauseKind::Spell, true, &json);
    assert_eq!(spell(text), None);
}

#[rstest]
#[case::tap(
    "{T}: When you next cast an instant or sorcery spell this turn, copy that spell. You may choose new targets for the copy."
)]
#[case::sacrifice(
    "{2}{U}, Sacrifice ~: When you next cast an instant or sorcery spell this turn, copy it for each time you've cast your commander from the command zone this game. You may choose new targets for the copies."
)]
fn an_activated_ability_sets_the_copy_up(#[case] text: &str) {
    let read = golden(text, ClauseKind::Activated, false);
    let Ability::Activated { effects, .. } = read else {
        panic!("{read:?}");
    };
    assert!(
        matches!(
            effects.as_slice(),
            [Effect::DelayedTrigger {
                once: true,
                this_turn: true,
                ..
            }]
        ),
        "{effects:?}"
    );
}

#[rstest]
#[case::an_additional_time(
    "When you next cast an instant or sorcery spell this turn, copy that spell an additional time. You may choose new targets for the copy."
)]
#[case::joined_new_targets(
    "When you next cast an instant or sorcery spell this turn, copy it and you may choose new targets for the copy."
)]
#[case::not_a_copy("When you next cast a spell this turn, draw a card.")]
#[case::another_spell("When you next cast another spell this turn, copy that spell.")]
#[case::no_comma("When you next cast an instant or sorcery spell this turn copy that spell.")]
#[case::not_a_spell("When you next cast a creature this turn, copy that spell.")]
#[case::an_opponent("When an opponent next casts a spell this turn, copy that spell.")]
#[case::wrong_count_word(
    "When you next cast an instant or sorcery spell this turn, copy that spell twice. You may choose new targets for the copy."
)]
#[case::a_target(
    "When you next cast an instant or sorcery spell this turn, copy target instant or sorcery spell."
)]
fn near_misses_of_the_next_cast_stay_unread(#[case] text: &str) {
    assert_eq!(spell(text), None, "{text}");
}
