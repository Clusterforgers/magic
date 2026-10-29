//! Goal: the evasion statics read into one shape the engine runs and write back as the same text: blockers picked by their power compared with the source's, "can't block it" and "can't block creatures you control", and "can't be blocked except by" a quality; near-misses stay with the table.
use mtg_core::card::{CardType, Subtype};

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

fn power_than_this(op: Cmp) -> Pred {
    Pred::Compare {
        stat: StatKind::Power,
        op,
        to: Amount::Stat {
            of: Ref::This,
            stat: StatKind::Power,
        },
    }
}

fn restricted(affected: Ref, blockers: Vec<Pred>) -> Ability {
    Ability::Static {
        affected,
        mods: vec![Modification::CantBeBlockedBy(Selector(blockers))],
        condition: None,
        defining: false,
        from: crate::refs::ZoneKind::Battlefield,
    }
}

fn not(pred: Pred) -> Pred {
    Pred::Not(Box::new(pred))
}

#[rstest]
#[case::less_it(
    "Creatures with power less than this creature's power can't block it.",
    Ref::This,
    vec![creature(), power_than_this(Cmp::Lt)]
)]
#[case::greater_it(
    "Creatures with power greater than this creature's power can't block it.",
    Ref::This,
    vec![creature(), power_than_this(Cmp::Gt)]
)]
#[case::skulk_wording(
    "~ can't be blocked by creatures with greater power.",
    Ref::This,
    vec![creature(), power_than_this(Cmp::Gt)]
)]
#[case::your_creatures(
    "Creatures with power less than this creature's power can't block creatures you control.",
    Ref::Each(Selector(vec![creature(), Pred::ControlledBy(Ref::You)])),
    vec![creature(), power_than_this(Cmp::Lt)]
)]
#[case::by_a_quality(
    "~ can't be blocked by creatures with flying.",
    Ref::This,
    vec![creature(), Pred::HasKeyword(Keyword::Flying)]
)]
#[case::except_flying(
    "~ can't be blocked except by creatures with flying.",
    Ref::This,
    vec![not(Pred::HasKeyword(Keyword::Flying))]
)]
#[case::except_artifacts(
    "~ can't be blocked except by artifact creatures.",
    Ref::This,
    vec![not(Pred::Type(CardType::Artifact))]
)]
#[case::except_slivers(
    "Slivers can't be blocked except by Slivers.",
    Ref::Each(Selector(vec![Pred::Subtype(Subtype("Sliver".to_owned()))])),
    vec![not(Pred::Subtype(Subtype("Sliver".to_owned())))]
)]
fn a_block_restriction_reads_into_the_selector_of_blockers(
    #[case] text: &str,
    #[case] affected: Ref,
    #[case] blockers: Vec<Pred>,
) {
    let read = golden(text, ClauseKind::Static, false);
    assert_eq!(read, restricted(affected, blockers), "{text}");
}

#[rstest]
#[case::power_without_a_comparison("Creatures with power less than 3 can't block it.")]
#[case::another_stat("Creatures with toughness less than this creature's loyalty can't block it.")]
#[case::an_unread_object("Creatures with power less than this creature's power can't block them.")]
#[case::except_nothing("~ can't be blocked except by creatures.")]
fn a_near_miss_is_left_to_the_table(#[case] text: &str) {
    assert_eq!(stat(text), None, "{text}");
}
