//! Goal: the drafts play: a fixed sample of 64 of them (`MTG_DRAFT_SMOKE=all`: every one) runs as an approved `Read` card
//! through the smoke gate, and the sample is the same for the same drafts, distinct and at most 64.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::drafts::items;

use mtg_cards::builtin;
use mtg_cards::card::Card;
use mtg_cards::{Caps, PrintedCard};
use mtg_cli::drafts::{SmokeKind, read_definitions, smoke};
use mtg_testkit::cards::creature;
use rstest::rstest;

/// The drafts played when `MTG_DRAFT_SMOKE` is not `all`.
const SAMPLE: usize = 64;

/// The seed of the sample: another seed picks other drafts.
const SEED: u64 = 0x5eed_d4af;

/// The index of a slug in the order of the sample: FNV-1a over the seed and the slug.
fn rank(slug: &str) -> u64 {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325 ^ SEED;
    for byte in slug.bytes() {
        hash = (hash ^ u64::from(byte)).wrapping_mul(0x0100_0000_01b3);
    }
    hash
}

/// `count` of the slugs, the same ones for the same list whatever its order.
fn sample(slugs: &[String], count: usize) -> Vec<String> {
    let mut ranked: Vec<&String> = slugs.iter().collect();
    ranked.sort_by_key(|slug| (rank(slug), (*slug).clone()));
    let mut chosen: Vec<String> = ranked.into_iter().take(count).cloned().collect();
    chosen.sort();
    chosen
}

#[rstest]
#[case::none(0)]
#[case::fewer_than_the_sample(10)]
#[case::the_sample(64)]
#[case::more_than_the_sample(500)]
fn the_sample_is_fixed_distinct_and_at_most_64(#[case] drafts: usize) {
    let slugs: Vec<String> = (0..drafts).map(|n| format!("card_{n}")).collect();
    let mut shuffled = slugs.clone();
    shuffled.reverse();

    let chosen = sample(&slugs, SAMPLE);

    assert_eq!(chosen.len(), drafts.min(SAMPLE));
    assert_eq!(chosen, sample(&shuffled, SAMPLE));
    let mut distinct = chosen.clone();
    distinct.dedup();
    assert_eq!(distinct, chosen);
    assert!(chosen.iter().all(|slug| slugs.contains(slug)));
}

#[test]
fn the_gate_passes_a_card_the_games_use_and_fails_one_they_never_see() {
    let bear = (*creature("Bear", 2, &[], 2, 2)).clone();
    // Costs more than any library's lands can pay: no game casts it.
    let colossus = (*creature("Colossus", 20, &[], 9, 9)).clone();

    let passed = smoke(&bear).unwrap();
    let (kind, detail) = smoke(&colossus).unwrap_err();

    assert!(passed.uses.cast > 0, "{passed:?}");
    assert_eq!(kind, SmokeKind::NeverSeen);
    assert!(detail.contains("never seen"), "{detail}");
}

#[test]
fn a_draft_that_is_gone_leaves_the_sample() {
    let slugs: Vec<String> = (0..500).map(|n| format!("card_{n}")).collect();
    let first = sample(&slugs, SAMPLE)[0].clone();
    let fewer: Vec<String> = slugs.into_iter().filter(|slug| *slug != first).collect();

    let chosen = sample(&fewer, SAMPLE);

    assert_eq!(chosen.len(), SAMPLE);
    assert!(!chosen.contains(&first));
}

#[test]
fn the_drafts_play_as_approved_read_cards_in_the_smoke_gate() {
    let (items, problems) = items();
    assert!(problems.is_empty(), "{}", problems.join("\n"));
    let all = std::env::var("MTG_DRAFT_SMOKE").is_ok_and(|value| value == "all");
    let slugs: Vec<String> = items.iter().map(|item| item.slug.clone()).collect();
    let chosen = if all { slugs } else { sample(&slugs, SAMPLE) };
    let played: Vec<&crate::drafts::Item> = items
        .iter()
        .filter(|item| chosen.contains(&item.slug))
        .collect();
    let pairs: Vec<(&PrintedCard, &Card)> = played
        .iter()
        .map(|item| (&item.printed, &item.card))
        .collect();
    let caps: Caps = mtg_engine::caps();

    let defined = read_definitions(builtin::library().unwrap(), &caps, &pairs);

    let mut failures = Vec::new();
    for (item, definition) in played.iter().zip(defined) {
        match definition {
            Err(message) => failures.push(format!("{}: {message}", item.slug)),
            Ok(definition) => {
                if let Err((kind, detail)) = smoke(&definition) {
                    failures.push(format!("{}: {}: {detail}", item.slug, kind.word()));
                }
            }
        }
    }
    assert!(
        failures.is_empty(),
        "{} of {} drafts fail:\n{}",
        failures.len(),
        played.len(),
        failures.join("\n")
    );
}
