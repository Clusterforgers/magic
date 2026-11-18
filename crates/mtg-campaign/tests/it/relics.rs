//! Goal: relic offers follow the deck from the filter's content generation on: a relic is offered only when the deck
//! (and its Satchel) holds enough of what its rules care about, generic relics always fit, a short pick is filled and
//! never empty, the offers follow the seed, and a run of an older generation or a deck chosen before the filter draws
//! as it did.
#![allow(clippy::too_many_lines)]

use std::collections::BTreeSet;

use mtg_campaign::relics::{FILTER_FROM, MIN_CARDS, offerable, usable};
use mtg_campaign::rng::Stream;
use mtg_campaign::run::{CONTENT, RELICS_AGAIN_FROM};
use mtg_campaign::{DeckMakeup, HeroDeck, Offer, Run, RunEvent, Want, content};
use mtg_core::card::CardType;
use mtg_testkit::corpus;
use rstest::rstest;

const RED: [&str; 3] = ["ruby_focus", "wildfire_pact", "flame_brand"];
const ARTIFACT: [&str; 3] = ["artisans_loupe", "tinkers_apron", "lockwork_key"];
const CREATURE: [&str; 7] = [
    "ember_idol",
    "trollhide_cloak",
    "soul_vessel",
    "blood_banner",
    "chargers_spur",
    "growth_idol",
    "giants_ring",
];

/// A makeup of `cards` (colours, type line, copies).
fn makeup(cards: &[(&str, &str, u32)]) -> DeckMakeup {
    let mut makeup = DeckMakeup::default();
    for (colors, line, copies) in cards {
        for _ in 0..*copies {
            makeup.add_type_line(colors, line);
        }
    }
    makeup
}

/// A mono-green deck of creatures and a few spells.
fn green_creatures() -> DeckMakeup {
    makeup(&[
        ("G", "Creature — Elf Druid", 30),
        ("G", "Sorcery", 5),
        ("", "Artifact", 3),
        ("", "Land — Forest", 36),
    ])
}

/// A red deck of burn, creatures and many artifacts.
fn red_artifacts() -> DeckMakeup {
    makeup(&[
        ("R", "Creature — Goblin", 14),
        ("R", "Instant", 12),
        ("", "Artifact", 12),
        ("", "Land — Mountain", 36),
    ])
}

fn hero_deck(makeup: Option<DeckMakeup>) -> HeroDeck {
    HeroDeck {
        id: "precon:x".into(),
        name: "Deck".into(),
        colors: "G".into(),
        commanders: Vec::new(),
        makeup,
    }
}

/// The events of a one-hero run in the lobby of content generation `version` (none stored for 0), up to the first boss.
fn started(version: u32, deck: HeroDeck) -> Vec<RunEvent> {
    let mut events = Vec::new();
    if version > 0 {
        events.push(RunEvent::Content { version });
    }
    events.push(RunEvent::Joined {
        name: "Ana".into(),
        user: Some("u1".into()),
        bot: None,
    });
    events.push(RunEvent::DeckChosen { hero: 0, deck });
    events.push(RunEvent::Started);
    events.push(RunEvent::FightCreated { game: "g".into() });
    events
}

/// The relics the hero is offered after the first boss falls, holding `owned` and `satchel`.
fn loot(
    seed: u64,
    version: u32,
    deck: HeroDeck,
    owned: &[String],
    satchel: &[&str],
) -> Vec<String> {
    let content = content().unwrap();
    let events = started(version, deck);
    let mut run = Run::replay(content, "warrens", seed, 0, &events).unwrap();
    run.heroes[0].relics = owned.to_vec();
    run.heroes[0].satchel = satchel.iter().map(|card| (*card).to_owned()).collect();
    let won = RunEvent::FightEnded {
        won: true,
        lives: vec![40],
        spirits: vec![],
    };
    run.apply(content, &won).unwrap();
    match run.heroes[0].offers.first() {
        Some(Offer::Relics { relics }) => relics.clone(),
        other => panic!("a relic pick comes first, not {other:?}"),
    }
}

/// Every relic offered in the first loot of the seeds 0 to 79.
fn offered(deck: &DeckMakeup) -> BTreeSet<String> {
    (0..80)
        .flat_map(|seed| loot(seed, CONTENT, hero_deck(Some(deck.clone())), &[], &[]))
        .collect()
}

#[test]
fn a_green_creature_deck_is_never_offered_a_red_spell_relic() {
    let seen = offered(&green_creatures());
    for id in RED {
        assert!(!seen.contains(id), "{id} was offered to a green deck");
    }
    assert!(
        CREATURE.iter().any(|id| seen.contains(*id)),
        "creature relics are offered: {seen:?}"
    );
    assert!(seen.contains("emerald_focus"), "its own colour is offered");
}

#[test]
fn a_red_deck_is_offered_red_spell_relics() {
    let seen = offered(&red_artifacts());
    assert!(RED.iter().any(|id| seen.contains(*id)), "{seen:?}");
    assert!(!seen.contains("emerald_focus"));
}

#[test]
fn an_artifact_light_deck_is_never_offered_artifact_relics() {
    let light = offered(&green_creatures());
    for id in ARTIFACT {
        assert!(!light.contains(id), "{id} was offered to 3 artifacts");
    }
    let rich = offered(&red_artifacts());
    assert!(ARTIFACT.iter().any(|id| rich.contains(*id)), "{rich:?}");
}

#[test]
fn a_deck_with_one_card_short_of_the_threshold_is_not_offered_the_relic() {
    let below = makeup(&[("", "Artifact", MIN_CARDS - 1), ("", "Land", 40)]);
    let enough = makeup(&[("", "Artifact", MIN_CARDS), ("", "Land", 40)]);
    assert!(!offered(&below).contains("artisans_loupe"));
    assert!(offered(&enough).contains("artisans_loupe"));
}

#[test]
fn the_satchel_counts_toward_what_a_relic_wants() {
    let content = content().unwrap();
    let theme = content.theme("warrens").unwrap();
    let loupe = content.card("artisans_loupe").unwrap();
    let seven = makeup(&[("", "Artifact", MIN_CARDS - 1), ("", "Land", 40)]);
    assert!(!usable(loupe, &seven));
    let with = seven.with_satchel(&["Sol Ring".to_owned()], theme);
    assert!(usable(loupe, &with));
    // A card the pool lacks (a Mirror Pool copy of a deck card) counts nothing more.
    let copy = seven.with_satchel(&["Some Deck Card".to_owned()], theme);
    assert_eq!(copy, seven);
    let offers: BTreeSet<String> = (0..80)
        .flat_map(|seed| {
            loot(
                seed,
                CONTENT,
                hero_deck(Some(seven.clone())),
                &[],
                &["Sol Ring"],
            )
        })
        .collect();
    assert!(ARTIFACT.iter().any(|id| offers.contains(*id)), "{offers:?}");
}

/// The ids of every relic of the content, in id order.
fn relic_ids() -> Vec<String> {
    content()
        .unwrap()
        .relics()
        .iter()
        .map(|c| c.id.clone())
        .collect()
}

#[test]
fn a_short_pick_is_filled_from_the_generic_relics_and_is_never_empty() {
    let content = content().unwrap();
    let deck = green_creatures();
    // The hero holds every relic but a red one and two generic ones: only the generic two fit the deck.
    let left = ["ruby_focus", "hearthstone", "seers_lantern"];
    let owned: Vec<String> = relic_ids()
        .into_iter()
        .filter(|id| !left.contains(&id.as_str()))
        .collect();
    for seed in 0..20 {
        let mut got = loot(
            seed,
            RELICS_AGAIN_FROM - 1,
            hero_deck(Some(deck.clone())),
            &owned,
            &[],
        );
        got.sort();
        assert_eq!(got, ["hearthstone", "seers_lantern"], "seed {seed}");
    }
    // Nothing usable is left: the pick falls back to what is left rather than offering nothing.
    let owned: Vec<String> = relic_ids()
        .into_iter()
        .filter(|id| !["ruby_focus", "artisans_loupe"].contains(&id.as_str()))
        .collect();
    for seed in 0..20 {
        let mut got = loot(
            seed,
            RELICS_AGAIN_FROM - 1,
            hero_deck(Some(deck.clone())),
            &owned,
            &[],
        );
        got.sort();
        assert_eq!(got, ["artisans_loupe", "ruby_focus"], "seed {seed}");
    }
    // A hero who holds every relic is offered none.
    let all = relic_ids();
    assert_eq!(
        loot(1, RELICS_AGAIN_FROM - 1, hero_deck(Some(deck)), &all, &[]).len(),
        0
    );
    assert!(content.relics().len() > 40);
}

#[test]
fn a_pick_holds_three_usable_relics_when_the_deck_can_use_that_many() {
    for seed in 0..40 {
        let got = loot(seed, CONTENT, hero_deck(Some(green_creatures())), &[], &[]);
        assert_eq!(got.len(), 3, "seed {seed}");
        let unique: BTreeSet<&String> = got.iter().collect();
        assert_eq!(unique.len(), 3, "seed {seed}");
    }
}

#[test]
fn the_offers_follow_the_seed() {
    let first: Vec<Vec<String>> = (0..30)
        .map(|seed| loot(seed, CONTENT, hero_deck(Some(green_creatures())), &[], &[]))
        .collect();
    let again: Vec<Vec<String>> = (0..30)
        .map(|seed| loot(seed, CONTENT, hero_deck(Some(green_creatures())), &[], &[]))
        .collect();
    assert_eq!(first, again);
    let distinct: BTreeSet<&Vec<String>> = first.iter().collect();
    assert!(distinct.len() > 10, "the seeds draw differently");
    // The deck, not the draw, decides what is possible.
    let red: Vec<Vec<String>> = (0..30)
        .map(|seed| loot(seed, CONTENT, hero_deck(Some(red_artifacts())), &[], &[]))
        .collect();
    assert_ne!(first, red);
}

#[rstest]
#[case::no_content_event(0)]
#[case::generation_one(1)]
fn a_run_of_an_older_generation_draws_as_it_did(#[case] version: u32) {
    let content = content().unwrap();
    // The first twenty relics, in id order, as the draw saw them before the filter and the larger pool.
    let before: Vec<String> = content
        .relics()
        .iter()
        .filter(|c| c.since == 0)
        .map(|c| c.id.clone())
        .collect();
    assert_eq!(before.len(), 20);
    for seed in 0..40 {
        let events = started(version, hero_deck(Some(green_creatures())));
        let run = Run::replay(content, "warrens", seed, 0, &events).unwrap();
        let node = run.at.unwrap();
        let expected = Stream::new(seed, "loot", u64::from(node) * 16, 0).pick(&before, 3);
        let got = loot(seed, version, hero_deck(Some(green_creatures())), &[], &[]);
        assert_eq!(got, expected, "seed {seed}");
    }
}

#[test]
fn a_deck_chosen_before_the_filter_is_offered_every_relic_of_the_pool() {
    let none: BTreeSet<String> = (0..400)
        .flat_map(|seed| loot(seed, CONTENT, hero_deck(None), &[], &[]))
        .collect();
    // Every relic but the Mythic ones, which loot offers only after the last lieutenant.
    let content = content().unwrap();
    let pool = content
        .relics()
        .iter()
        .filter(|c| c.rarity != mtg_campaign::Rarity::Mythic)
        .count();
    assert_eq!(none.len(), pool, "{none:?}");
}

#[test]
fn a_bot_ally_is_offered_the_same_filtered_relics_and_takes_the_first() {
    let content = content().unwrap();
    for seed in 0..40 {
        // A hero with a red deck and a bot ally with a green one.
        let events = vec![
            RunEvent::Content { version: CONTENT },
            RunEvent::Joined {
                name: "Ana".into(),
                user: Some("u1".into()),
                bot: None,
            },
            RunEvent::Joined {
                name: "Bot".into(),
                user: None,
                bot: Some(mtg_campaign::Brain::Normal),
            },
            RunEvent::DeckChosen {
                hero: 0,
                deck: hero_deck(Some(red_artifacts())),
            },
            RunEvent::DeckChosen {
                hero: 1,
                deck: hero_deck(Some(green_creatures())),
            },
            RunEvent::Started,
            RunEvent::FightCreated { game: "g".into() },
            RunEvent::FightEnded {
                won: true,
                lives: vec![40, 40],
                spirits: vec![],
            },
        ];
        let run = Run::replay(content, "warrens", seed, 0, &events).unwrap();
        // The log replays to the same offers: the filter reads only the stored makeup and the seed.
        assert_eq!(
            Run::replay(content, "warrens", seed, 0, &events).unwrap(),
            run
        );
        let Some(Offer::Relics { relics }) = run.heroes[1].offers.first() else {
            panic!("the bot is offered relics");
        };
        assert!(!relics.is_empty(), "seed {seed}");
        for id in RED {
            assert!(!relics.contains(&id.to_owned()), "seed {seed}: {id}");
        }
    }
}

#[test]
fn the_filter_starts_at_the_generation_that_stores_it() {
    assert_eq!(FILTER_FROM, 2);
    const { assert!(CONTENT >= FILTER_FROM) };
}

/// A makeup holding exactly what `want` names, enough of it.
fn holding(want: &Want) -> DeckMakeup {
    let mut makeup = DeckMakeup::default();
    match want {
        Want::Color(color) => makeup.colors.insert(color.to_string(), MIN_CARDS),
        Want::Type(card_type) => makeup.types.insert(format!("{card_type:?}"), MIN_CARDS),
        Want::Subtype(subtype) => makeup.subtypes.insert(subtype.clone(), MIN_CARDS),
        Want::Tokens => {
            makeup.tokens = MIN_CARDS;
            None
        }
        Want::Counters => {
            makeup.counters = MIN_CARDS;
            None
        }
        Want::Graveyard => {
            makeup.graveyard = MIN_CARDS;
            None
        }
        Want::CreatureCommander => {
            makeup.creature_commanders = 1;
            None
        }
    };
    makeup
}

#[test]
fn every_relic_is_placed_by_what_it_wants() {
    let content = content().unwrap();
    let nothing = DeckMakeup::default();
    // A deck with plenty of everything a relic could want but what this relic wants.
    for relic in content.relics() {
        if relic.wants.is_empty() {
            assert!(usable(relic, &nothing), "{} is generic", relic.id);
            continue;
        }
        assert!(!usable(relic, &nothing), "{} needs something", relic.id);
        for want in &relic.wants {
            assert!(
                usable(relic, &holding(want)),
                "{} fits a deck holding {want:?}",
                relic.id
            );
        }
        let others: Vec<&Want> = content
            .relics()
            .iter()
            .flat_map(|other| other.wants.iter())
            .filter(|want| !relic.wants.contains(want))
            .collect();
        let mut elsewhere = DeckMakeup::default();
        for want in others {
            let held = holding(want);
            for (key, n) in held.colors {
                *elsewhere.colors.entry(key).or_default() += n;
            }
            for (key, n) in held.types {
                *elsewhere.types.entry(key).or_default() += n;
            }
            elsewhere.tokens += held.tokens;
            elsewhere.counters += held.counters;
            elsewhere.graveyard += held.graveyard;
            elsewhere.creature_commanders += held.creature_commanders;
        }
        // Counts of the colours and types it does not name never make it usable.
        for want in &relic.wants {
            match want {
                Want::Color(color) => {
                    elsewhere.colors.remove(&color.to_string());
                }
                Want::Type(card_type) => {
                    elsewhere.types.remove(&format!("{card_type:?}"));
                }
                Want::Tokens => elsewhere.tokens = 0,
                Want::Counters => elsewhere.counters = 0,
                Want::Graveyard => elsewhere.graveyard = 0,
                Want::CreatureCommander => elsewhere.creature_commanders = 0,
                Want::Subtype(_) => {}
            }
        }
        assert!(
            !usable(relic, &elsewhere),
            "{} wants only its own",
            relic.id
        );
    }
}

#[test]
fn a_relic_about_two_types_counts_them_together() {
    let content = content().unwrap();
    let ring = content.card("spellbinders_ring").unwrap();
    let split = makeup(&[("", "Instant", 5), ("", "Sorcery", 3)]);
    assert!(usable(ring, &split));
    let short = makeup(&[("", "Instant", 5), ("", "Sorcery", 2)]);
    assert!(!usable(ring, &short));
}

#[test]
fn a_creature_commander_is_wanted_once() {
    let content = content().unwrap();
    let banner = content.card("banner_of_command").unwrap();
    let mut deck = DeckMakeup::default();
    assert!(!usable(banner, &deck));
    deck.creature_commanders = 1;
    assert!(usable(banner, &deck));
}

#[test]
fn a_pick_never_falls_empty_while_a_relic_is_left() {
    let content = content().unwrap();
    let wanting: Vec<_> = content
        .relics()
        .into_iter()
        .filter(|c| !c.wants.is_empty())
        .collect();
    let picked = offerable(wanting.clone(), &DeckMakeup::default());
    assert_eq!(picked.len(), wanting.len());
    assert_eq!(offerable(Vec::new(), &DeckMakeup::default()).len(), 0);
}

#[test]
fn the_makeup_counts_colours_types_and_subtypes_of_each_card() {
    let mut deck = DeckMakeup::default();
    deck.add_type_line("RG", "Legendary Creature — Elf Druid");
    deck.add_type_line("G", "Artifact Creature — Elf");
    deck.add_type_line("", "Land");
    assert_eq!(deck.count(&Want::Color('G')), 2);
    assert_eq!(deck.count(&Want::Color('R')), 1);
    assert_eq!(deck.count(&Want::Color('W')), 0);
    assert_eq!(deck.count(&Want::Type(CardType::Creature)), 2);
    assert_eq!(deck.count(&Want::Type(CardType::Artifact)), 1);
    assert_eq!(deck.count(&Want::Type(CardType::Land)), 1);
    assert_eq!(deck.count(&Want::Subtype("Elf".into())), 2);
    assert_eq!(deck.count(&Want::Subtype("Druid".into())), 1);
}

#[rstest]
#[case::counters("Hardened Scales", (0, 1, 0))]
#[case::tokens("Abstruse Interference", (1, 0, 0))]
#[case::graveyard("Bookwurm", (0, 0, 1))]
#[case::none("Skullclamp", (0, 0, 0))]
fn the_makeup_reads_what_a_real_cards_rules_do(#[case] name: &str, #[case] seen: (u32, u32, u32)) {
    let mut deck = DeckMakeup::default();
    deck.add_card(&corpus::card(name).unwrap(), false);
    assert_eq!((deck.tokens, deck.counters, deck.graveyard), seen, "{name}");
}

#[test]
fn the_makeup_counts_a_creature_commander_and_a_real_cards_colours_and_types() {
    let mut deck = DeckMakeup::default();
    deck.add_card(&corpus::card("Serra Angel").unwrap(), true);
    deck.add_card(&corpus::card("Sol Ring").unwrap(), false);
    deck.add_card(&corpus::card("Kolaghan's Command").unwrap(), false);
    assert_eq!(deck.creature_commanders, 1);
    assert_eq!(deck.count(&Want::Color('W')), 1);
    assert_eq!(deck.count(&Want::Color('B')), 1);
    assert_eq!(deck.count(&Want::Color('R')), 1);
    assert_eq!(deck.count(&Want::Type(CardType::Artifact)), 1);
    assert_eq!(deck.count(&Want::Type(CardType::Instant)), 1);
    assert_eq!(deck.count(&Want::Subtype("Angel".into())), 1);
}
