//! Goal: a prevention that names its damage ("prevent that damage and put that many +1/+1 counters on it", "prevent all but 1 of that damage"), its shield for the turn over the one target of a trigger, a token replacement for one kind of token and the sacrifice of another permanent of one of two subtypes read into the DSL, write back as the same text, and leave every near-miss to the table.
use super::*;
use crate::trigger::{ReplaceBy, ReplaceWhen};

#[rstest]
#[case::equipped_creature_grows(
    "If equipped creature would be dealt damage, prevent that damage and put that many +1/+1 counters on it.",
    r#"{"Replacement":{"when":{"Damage":{"to":"Attached"}},"by":{"PreventAnd":[{"PutCounters":{"on":"Triggering","counter":"+1/+1","count":"Triggering"}}]}}}"#
)]
#[case::the_card_grows(
    "If this creature would be dealt damage, prevent that damage and put that many +1/+1 counters on it.",
    r#"{"Replacement":{"when":{"Damage":{"to":"This"}},"by":{"PreventAnd":[{"PutCounters":{"on":"Triggering","counter":"+1/+1","count":"Triggering"}}]}}}"#
)]
#[case::all_but_one(
    "If a source would deal damage to another Dinosaur you control, prevent all but 1 of that damage.",
    r#"{"Replacement":{"when":{"Damage":{"to":{"Each":[{"Subtype":"Dinosaur"},"Other",{"ControlledBy":"You"}]}}},"by":{"PreventAllBut":1}}}"#
)]
#[case::angels_instead(
    "If one or more creature tokens would be created under your control, that many 4/4 white Angel creature tokens with flying and vigilance are created instead.",
    r#"{"Replacement":{"when":{"TokenCreation":{"what":[{"Type":"Creature"}]}},"by":{"Instead":[{"CreateToken":{"token":{"Custom":{"name":"Angel","colors":["White"],"types":["Creature"],"subtypes":["Angel"],"power":4,"toughness":4,"abilities":[{"Keyword":"Flying"},{"Keyword":"Vigilance"}]}},"count":"Triggering"}}]}}}"#
)]
fn a_prevention_or_token_replacement_reads_back(#[case] text: &str, #[case] json: &str) {
    reads_as(text, ClauseKind::Static, false, json);
}

#[rstest]
#[case::shield_of_the_target(
    "When this creature enters, choose target creature you control. If damage would be dealt to that creature this turn, prevent that damage and put that many +1/+1 counters on it."
)]
fn a_shield_that_names_its_damage_reads_back(#[case] text: &str) {
    golden(text, ClauseKind::Triggered, false);
}

#[test]
fn the_sacrifice_of_another_of_two_subtypes_reads_back() {
    golden(
        "{2}{B}, Sacrifice another Vampire or Zombie: Put two +1/+1 counters on this creature.",
        ClauseKind::Activated,
        false,
    );
}

#[rstest]
#[case::any_damage("If equipped creature would be dealt damage, prevent that damage.")]
#[case::other_counters(
    "If equipped creature would be dealt damage, prevent that damage and put that many -1/-1 counters on it."
)]
#[case::all_tokens(
    "If one or more tokens would be created under your control, that many 4/4 white Angel creature tokens with flying are created instead."
)]
#[case::an_opponent(
    "If one or more creature tokens would be created under an opponent's control, that many 1/1 white Spirit creature tokens are created instead."
)]
fn near_misses_are_not_read_as_these(#[case] text: &str) {
    let read = clause(text, ClauseKind::Static, false);
    assert!(
        !matches!(
            read,
            Some(
                Ability::Replacement {
                    by: ReplaceBy::PreventAnd(_) | ReplaceBy::PreventAllBut(_),
                    ..
                } | Ability::Replacement {
                    when: ReplaceWhen::TokenCreation { .. },
                    by: ReplaceBy::Instead(_),
                    ..
                }
            )
        ),
        "{text}: {read:?}"
    );
}

#[rstest]
#[case::any_one("Sacrifice a Vampire or Zombie: Draw a card.")]
#[case::lowercase_is_no_subtype("Sacrifice a vampire or zombie: Draw a card.")]
fn a_sacrifice_of_either_subtype_reads_only_for_subtypes(#[case] text: &str) {
    let read = clause(text, ClauseKind::Activated, false);
    assert_eq!(read.is_some(), text.contains("Vampire"), "{text}: {read:?}");
}
