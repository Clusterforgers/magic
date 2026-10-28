//! Goal: "you get an emblem" and "target opponent gets an emblem" read with a static, triggered or activated ability
//! (or two) in quotes and write back as printed; an emblem with no quotes, or quotes nothing reads, stays with the table.
use super::*;
use crate::effect::Effect;
use crate::refs::Ref;

fn emblem(text: &str) -> (Ref, Vec<Ability>) {
    let Ability::Loyalty { effects, .. } = golden(text, ClauseKind::Loyalty, false) else {
        panic!("not a loyalty ability: {text}");
    };
    let [Effect::CreateEmblem { who, abilities }] = effects.as_slice() else {
        panic!("not an emblem: {effects:?}");
    };
    (who.clone(), abilities.clone())
}

#[rstest]
#[case::anthem(
    "−7: You get an emblem with \"Creatures you control get +2/+2 and have flying.\"",
    1
)]
#[case::anthem_plus("−2: You get an emblem with \"Creatures you control get +1/+0.\"", 1)]
#[case::trigger_for_the_owner(
    "−6: You get an emblem with \"Whenever an artifact you control becomes tapped, draw a card.\"",
    1
)]
#[case::this_emblem(
    "−7: You get an emblem with \"Whenever you cast a spell, this emblem deals 5 damage to any target.\"",
    1
)]
#[case::activated(
    "−8: You get an emblem with \"{1}{B}, Sacrifice a creature: You gain 2 life.\"",
    1
)]
#[case::two_abilities(
    "−6: You get an emblem with \"Creatures you control get +1/+0.\" and \"Whenever you cast a spell, this emblem deals 5 damage to any target.\"",
    2
)]
fn w6b_you_get_an_emblem(#[case] text: &str, #[case] count: usize) {
    let (who, abilities) = emblem(text);
    assert_eq!(who, Ref::You);
    assert_eq!(abilities.len(), count);
}

#[rstest]
#[case::lose_life(
    "−8: Target opponent gets an emblem with \"Whenever a player draws a card, you lose 2 life.\""
)]
fn w6b_target_opponent_gets_an_emblem(#[case] text: &str) {
    let (who, abilities) = emblem(text);
    assert_eq!(who, Ref::Target(0));
    assert_eq!(abilities.len(), 1);
}

#[rstest]
#[case::no_quotes("−7: You get an emblem.")]
#[case::unread_ability("−7: You get an emblem with \"Flibber the jabberwock.\"")]
#[case::other_noun("−7: You get a token with \"Creatures you control get +1/+0.\"")]
fn w6b_near_misses_stay_with_the_table(#[case] text: &str) {
    let read = clause(text, ClauseKind::Loyalty, false);
    assert!(
        !matches!(&read, Some(Ability::Loyalty { effects, .. }) if effects.iter().any(|e| matches!(e, Effect::CreateEmblem { .. }))),
        "{read:?}"
    );
}
