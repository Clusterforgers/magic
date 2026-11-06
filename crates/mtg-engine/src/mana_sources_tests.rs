//! Goal: a pool keeps the kind of the source of each mana it holds while that mana stays: spending takes mana with a known source first and tells it, emptying drops the sources of the mana that went, and restricted mana keeps none.

use mtg_cards::spend::{ManaLasts, SpendOn, SpendTraits};
use mtg_core::card::{CardType, Subtype};
use mtg_core::mana::ManaType;
use rstest::rstest;

use crate::mana::ManaPool;

use ManaType::{Blue, Red};

fn treasure() -> SpendTraits {
    SpendTraits {
        types: [CardType::Artifact].into_iter().collect(),
        subtypes: vec![Subtype("Treasure".to_owned())],
        ..SpendTraits::default()
    }
}

/// A pool of one plain Blue, then one Blue from a Treasure lasting `lasts`.
fn pool(lasts: ManaLasts) -> ManaPool {
    let mut pool = ManaPool::default();
    pool.add_restricted(Blue, Vec::new(), ManaLasts::Step);
    pool.add_from(Blue, Vec::new(), lasts, Some(treasure()));
    pool
}

#[test]
fn spending_takes_the_mana_with_a_known_source_first() {
    let mut held = pool(ManaLasts::Step);
    assert_eq!(held.sources().len(), 1);
    assert_eq!(held.spend_noting(Red), None);
    assert_eq!(held.spend_noting(Blue), Some(Some(treasure())));
    assert_eq!(held.spend_noting(Blue), Some(None));
    assert_eq!(held, ManaPool::default());
}

#[rstest]
#[case::the_step_takes_it(ManaLasts::Step, 0)]
#[case::kept_mana_keeps_its_source(ManaLasts::Turn, 1)]
fn emptying_drops_the_sources_of_the_mana_that_went(#[case] lasts: ManaLasts, #[case] left: usize) {
    let mut held = pool(lasts);
    held.empty(ManaLasts::Step);
    assert_eq!(held.sources().len(), left);
}

#[test]
fn restricted_mana_keeps_no_source() {
    let mut held = ManaPool::default();
    held.add_from(Blue, vec![SpendOn::Snow], ManaLasts::Step, Some(treasure()));
    assert!(held.sources().is_empty());
}
