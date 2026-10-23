//! Goal: the limits on blocking read into one shape each and write back as the same text: "can block only creatures with flying" for a creature, an Aura's host and a group, "can't be blocked by more than one creature" for a creature, a group and what an Equipment or Aura is attached to, and "can't be blocked except by two or more creatures"; near-misses stay with the table.
use mtg_core::card::CardType;

use super::*;
use crate::keyword::Keyword;
use crate::modify::Modification;
use crate::refs::{Pred, Ref, Selector};
use crate::value::{Amount, Cmp, StatKind};

fn stat(text: &str) -> Option<Ability> {
    clause(text, ClauseKind::Static, false)
}

fn creature() -> Pred {
    Pred::Type(CardType::Creature)
}

fn fliers() -> Selector {
    Selector(vec![creature(), Pred::HasKeyword(Keyword::Flying)])
}

fn limit(affected: Ref, modification: Modification) -> Ability {
    Ability::Static {
        affected,
        mods: vec![modification],
        condition: None,
        defining: false,
        from: crate::refs::ZoneKind::Battlefield,
    }
}

fn mine(extra: Vec<Pred>) -> Ref {
    let mut preds = vec![creature(), Pred::ControlledBy(Ref::You)];
    preds.extend(extra);
    Ref::Each(Selector(preds))
}

#[rstest]
#[case::the_card_itself(
    "~ can block only creatures with flying.",
    Ref::This,
    Modification::CanBlockOnly(fliers())
)]
#[case::this_creature(
    "This creature can block only creatures with flying.",
    Ref::This,
    Modification::CanBlockOnly(fliers())
)]
#[case::an_aura_host(
    "Enchanted creature can block only creatures with flying.",
    Ref::Attached,
    Modification::CanBlockOnly(fliers())
)]
#[case::a_group(
    "Creatures with flying can block only creatures with flying.",
    Ref::Each(fliers()),
    Modification::CanBlockOnly(fliers())
)]
#[case::one_blocker(
    "~ can't be blocked by more than one creature.",
    Ref::This,
    Modification::MaxBlockers(1)
)]
#[case::an_equipped_creature(
    "Equipped creature can't be blocked by more than one creature.",
    Ref::Attached,
    Modification::MaxBlockers(1)
)]
#[case::every_creature_you_control(
    "Each creature you control can't be blocked by more than one creature.",
    mine(Vec::new()),
    Modification::MaxBlockers(1)
)]
#[case::big_creatures(
    "Each creature you control with power 4 or greater can't be blocked by more than one creature.",
    mine(vec![Pred::Compare { stat: StatKind::Power, op: Cmp::Ge, to: Amount::Fixed(4) }]),
    Modification::MaxBlockers(1)
)]
#[case::two_or_more(
    "~ can't be blocked except by two or more creatures.",
    Ref::This,
    Modification::MinBlockers(2)
)]
#[case::three_or_more(
    "Enchanted creature can't be blocked except by three or more creatures.",
    Ref::Attached,
    Modification::MinBlockers(3)
)]
fn a_limit_on_blocking_reads_into_one_modification(
    #[case] text: &str,
    #[case] affected: Ref,
    #[case] modification: Modification,
) {
    assert_eq!(
        golden(text, ClauseKind::Static, false),
        limit(affected, modification),
        "{text}"
    );
}

#[rstest]
#[case::a_number_of_blockers_that_is_not_one("~ can't be blocked by more than two creatures.")]
#[case::fewer_than_two_or_more("~ can't be blocked except by one or more creatures.")]
#[case::only_a_quality_without_a_noun("~ can block only flying.")]
#[case::a_condition_after("~ can block only creatures with flying as long as it's your turn.")]
#[case::another_verb("~ can attack only creatures with flying.")]
#[case::a_target("Target creature can block only creatures with flying.")]
fn a_near_miss_is_left_to_the_table(#[case] text: &str) {
    assert_eq!(stat(text), None, "{text}");
}
