//! Goal: the wordings that alone held back a few precon cards each (the life gained this turn counted as "N or more", a card called "he" that connives, an Equipment that attaches itself as it enters, a choice to gain life) read into the DSL, write back as the same text, and leave their near-misses to the table.
use super::*;
use crate::effect::Effect;
use crate::refs::Ref;
use crate::value::{Amount, Cmp, Condition, TurnTally};

fn trigger(text: &str) -> Option<Ability> {
    clause(text, ClauseKind::Triggered, false)
}

#[rstest]
#[case::angel(
    "At the beginning of each end step, if you gained 4 or more life this turn, create a 4/4 white Angel creature token with flying.",
    4
)]
#[case::draw(
    "At the beginning of each end step, if you gained 3 or more life this turn, draw a card.",
    3
)]
#[case::opponents_lose(
    "At the beginning of your end step, if you gained 3 or more life this turn, each opponent loses 3 life.",
    3
)]
#[case::large(
    "At the beginning of each end step, if you gained 10 or more life this turn, draw a card.",
    10
)]
fn life_gained_this_turn_counts_as_a_total(#[case] text: &str, #[case] least: i32) {
    let read = golden(text, ClauseKind::Triggered, false);
    let Ability::Triggered {
        intervening_if: Some(condition),
        ..
    } = read
    else {
        panic!("no intervening if: {text}");
    };
    let expected = Condition::Compare {
        left: Amount::Tallied {
            who: Ref::You,
            what: TurnTally::LifeGainedTotal,
        },
        op: Cmp::Ge,
        right: Amount::Fixed(least),
    };
    assert_eq!(condition, expected, "{text}");
}

#[rstest]
#[case::one_is_the_plain_wording(
    "At the beginning of your end step, if you gained 1 or more life this turn, draw a card."
)]
#[case::in_words(
    "At the beginning of your end step, if you gained three or more life this turn, draw a card."
)]
#[case::an_opponent(
    "At the beginning of your end step, if an opponent gained 3 or more life this turn, draw a card."
)]
#[case::lost(
    "At the beginning of your end step, if you lost 3 or more life this turn, draw a card."
)]
#[case::no_unit(
    "At the beginning of your end step, if you gained 3 or more this turn, draw a card."
)]
fn life_gained_near_misses_stay_unread(#[case] text: &str) {
    assert_eq!(trigger(text), None, "{text}");
}

#[rstest]
#[case::he("Whenever ~ attacks, he connives.")]
#[case::it("Whenever ~ attacks, it connives.")]
#[case::enters("When ~ enters, he connives.")]
fn a_card_that_is_a_person_connives_as_he(#[case] text: &str) {
    let read = golden(text, ClauseKind::Triggered, false);
    let Ability::Triggered { effects, .. } = read else {
        panic!("not a trigger: {text}");
    };
    assert_eq!(effects, vec![Effect::Connive { what: Ref::This }], "{text}");
}

#[rstest]
#[case::another_creature("Whenever another creature attacks, he connives.")]
#[case::in_a_spell("He connives.")]
#[case::she("Whenever ~ attacks, she connives.")]
fn he_connives_only_for_the_card_that_acts(#[case] text: &str) {
    assert_eq!(trigger(text), None, "{text}");
}

#[rstest]
#[case::it("When ~ enters, attach it to target creature you control.")]
#[case::itself("When ~ enters, attach ~ to target creature you control.")]
#[case::other_targets("When ~ enters, attach it to target creature.")]
fn an_equipment_attaches_itself_as_it_enters(#[case] text: &str) {
    let read = golden(text, ClauseKind::Triggered, false);
    let Ability::Triggered {
        targets, effects, ..
    } = read
    else {
        panic!("not a trigger: {text}");
    };
    assert_eq!(targets.len(), 1, "{text}");
    assert_eq!(
        effects,
        vec![Effect::Attach {
            what: Ref::This,
            to: Ref::Target(0)
        }],
        "{text}"
    );
}

#[rstest]
#[case::another_equipment(
    "When ~ enters, attach target Equipment you control to target creature you control."
)]
#[case::a_spell("Attach ~ to target creature you control.")]
#[case::no_target("When ~ enters, attach it.")]
#[case::another_trigger("Whenever you cast a spell, attach it to target creature you control.")]
fn attach_near_misses_stay_unread(#[case] text: &str) {
    assert_eq!(trigger(text), None, "{text}");
    assert_eq!(spell(text), None, "{text}");
}

fn stat(text: &str) -> Option<Ability> {
    clause(text, ClauseKind::Static, false)
}

#[rstest]
#[case::combat_to_the_card(
    "Prevent all combat damage that would be dealt to this creature.",
    r#"{"to":"This","combat":true}"#,
    r#""Prevent""#
)]
#[case::all_to_the_card(
    "Prevent all damage that would be dealt to this creature.",
    r#"{"to":"This"}"#,
    r#""Prevent""#
)]
#[case::combat_to_you(
    "Prevent all combat damage that would be dealt to you.",
    r#"{"to":"You","combat":true}"#,
    r#""Prevent""#
)]
#[case::with_a_counter_removed(
    "If damage would be dealt to this creature, prevent that damage. Remove a +1/+1 counter from this creature.",
    r#"{"to":"This"}"#,
    r#"{"PreventRemovingCounter":"+1/+1"}"#
)]
fn damage_to_the_card_is_prevented(#[case] text: &str, #[case] when: &str, #[case] by: &str) {
    let json = format!(r#"{{"Replacement":{{"when":{{"Damage":{when}}},"by":{by}}}}}"#);
    reads_as(text, ClauseKind::Static, false, &json);
}

#[rstest]
#[case::to_an_opponent("Prevent all damage that would be dealt to target opponent.")]
#[case::a_counter_of_another_creature(
    "If damage would be dealt to this creature, prevent that damage. Remove a +1/+1 counter from target creature."
)]
#[case::no_kind(
    "If damage would be dealt to this creature, prevent that damage. Remove a counter from this creature."
)]
#[case::two_counters(
    "If damage would be dealt to this creature, prevent that damage. Remove two +1/+1 counters from this creature."
)]
#[case::this_turn("Prevent all combat damage that would be dealt to this creature this turn.")]
fn prevention_near_misses_stay_unread(#[case] text: &str) {
    assert_eq!(stat(text), None, "{text}");
}
