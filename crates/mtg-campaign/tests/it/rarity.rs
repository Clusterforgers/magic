//! Goal: from the rarity generation on every relic has a rarity that decides where it comes from: loot and treasure offer
//! mostly Common with some Rare (more on later floors), the shop sells one Common and one Rare at prices by rarity and
//! never a risky relic, a Mythic comes only from an event or the loot of the last lieutenant, and a run of an older
//! generation draws its relics and events as it did.
#![allow(clippy::too_many_lines)]

use std::collections::BTreeMap;

use mtg_campaign::relics::{FILTER_FROM, RARITY_FROM, price};
use mtg_campaign::rng::Stream;
use mtg_campaign::run::CONTENT;
use mtg_campaign::{
    Content, HeroDeck, Offer, Rarity, RoomKind, Run, RunEvent, ShopItem, Status, content,
};
use rstest::rstest;

fn deck() -> HeroDeck {
    HeroDeck {
        id: "precon:x".into(),
        name: "Deck".into(),
        colors: "WUBRG".into(),
        commanders: Vec::new(),
        makeup: None,
    }
}

/// A one-hero run of content generation `version` at its first boss's fight.
fn at_first_boss(content: &Content, seed: u64, version: u32) -> Run {
    let events = vec![
        RunEvent::Content { version },
        RunEvent::Joined {
            name: "Ana".into(),
            user: Some("u1".into()),
            bot: None,
        },
        RunEvent::DeckChosen {
            hero: 0,
            deck: deck(),
        },
        RunEvent::Started,
        RunEvent::FightCreated { game: "g".into() },
    ];
    Run::replay(content, "warrens", seed, 0, &events).unwrap()
}

/// The boss rooms of the ladder, first boss first.
fn bosses(run: &Run) -> Vec<u8> {
    let mut rooms: Vec<(u8, u8)> = run
        .map
        .nodes
        .iter()
        .filter(|n| n.kind == RoomKind::Boss)
        .map(|n| (n.row, n.id))
        .collect();
    rooms.sort_unstable();
    rooms.into_iter().map(|(_, id)| id).collect()
}

/// The relics offered after the boss of `rank` falls.
fn loot(content: &Content, seed: u64, version: u32, rank: usize) -> Vec<String> {
    let mut run = at_first_boss(content, seed, version);
    run.at = Some(bosses(&run)[rank]);
    run.apply(
        content,
        &RunEvent::FightEnded {
            won: true,
            lives: vec![40],
            spirits: vec![],
        },
    )
    .unwrap();
    match run.heroes[0].offers.first() {
        Some(Offer::Relics { relics }) => relics.clone(),
        other => panic!("a relic pick comes first, not {other:?}"),
    }
}

/// The offer of the camp of `kind` the run enters as its first camp, for the one hero.
fn camp(content: &Content, seed: u64, version: u32, kind: RoomKind) -> Offer {
    let mut run = at_first_boss(content, seed, version);
    let room = run.map.choices(run.at)[0];
    run.map
        .nodes
        .iter_mut()
        .find(|n| n.id == room)
        .unwrap()
        .kind = kind;
    run.status = Status::Map;
    run.heroes[0].offers.clear();
    run.apply(
        content,
        &RunEvent::Voted {
            hero: 0,
            node: room,
        },
    )
    .unwrap();
    run.heroes[0].offers.first().cloned().unwrap()
}

fn rarity(content: &Content, id: &str) -> Rarity {
    content.card(id).unwrap().rarity
}

#[test]
fn the_pool_has_every_tier_and_a_few_run_defining_mythics() {
    let content = content().unwrap();
    let mut tiers: BTreeMap<Rarity, usize> = BTreeMap::new();
    for relic in content.relics() {
        *tiers.entry(relic.rarity).or_default() += 1;
        if relic.rarity == Rarity::Mythic {
            assert_eq!(relic.since, RARITY_FROM, "{}", relic.id);
            assert_ne!(relic.role, "Risky", "{}", relic.id);
        }
    }
    assert!(tiers[&Rarity::Common] > tiers[&Rarity::Rare], "{tiers:?}");
    assert!((8..=12).contains(&tiers[&Rarity::Mythic]), "{tiers:?}");
    // A tiered pair: the Common focus takes {1}, the Rare prism the colour's own pip.
    assert_eq!(rarity(content, "ruby_focus"), Rarity::Common);
    assert_eq!(rarity(content, "ruby_prism"), Rarity::Rare);
}

#[rstest]
#[case::first_boss(0)]
#[case::second_boss(1)]
fn loot_offers_mostly_common_some_rare_and_never_mythic_before_the_last_lieutenant(
    #[case] rank: usize,
) {
    let content = content().unwrap();
    let mut seen: BTreeMap<Rarity, usize> = BTreeMap::new();
    for seed in 0..200 {
        for id in loot(content, seed, CONTENT, rank) {
            *seen.entry(rarity(content, &id)).or_default() += 1;
        }
    }
    assert_eq!(seen.get(&Rarity::Mythic), None, "{seen:?}");
    let (common, rare) = (seen[&Rarity::Common], seen[&Rarity::Rare]);
    // The chance of a Rare slot is 15 % after the first boss and 25 % after the second.
    let share = rare * 100 / (common + rare);
    let expected = [15, 25][rank];
    assert!(share.abs_diff(expected) <= 6, "{share}% rare: {seen:?}");
}

#[test]
fn the_last_lieutenants_loot_leads_with_a_mythic() {
    let content = content().unwrap();
    for seed in 0..40 {
        let run = at_first_boss(content, seed, CONTENT);
        let last = bosses(&run).len() - 2;
        let got = loot(content, seed, CONTENT, last);
        assert_eq!(got.len(), 3, "seed {seed}");
        assert_eq!(rarity(content, &got[0]), Rarity::Mythic, "seed {seed}");
        assert!(
            got[1..]
                .iter()
                .all(|id| rarity(content, id) != Rarity::Mythic),
            "seed {seed}: {got:?}"
        );
    }
}

#[test]
fn treasure_offers_rare_more_often_than_loot_and_never_mythic() {
    let content = content().unwrap();
    let mut rare = 0;
    let mut total = 0;
    for seed in 0..200 {
        let Offer::Relics { relics } = camp(content, seed, CONTENT, RoomKind::Treasure) else {
            panic!("a treasure offers relics")
        };
        for id in relics {
            let tier = rarity(content, &id);
            assert_ne!(tier, Rarity::Mythic, "seed {seed}");
            rare += usize::from(tier == Rarity::Rare);
            total += 1;
        }
    }
    // After the first boss: 15 % and the treasure's 20 more.
    let share = rare * 100 / total;
    assert!(share.abs_diff(35) <= 7, "{share}% rare");
}

#[test]
fn the_shop_sells_one_common_and_one_rare_at_their_prices_and_never_a_risky_relic() {
    let content = content().unwrap();
    for seed in 0..60 {
        let Offer::Shop { items } = camp(content, seed, CONTENT, RoomKind::Shop) else {
            panic!("a shop")
        };
        let relics: Vec<(Rarity, u32, String)> = items
            .iter()
            .filter_map(|item| match item {
                ShopItem::Relic { id, price, .. } => {
                    Some((rarity(content, id), *price, id.clone()))
                }
                _ => None,
            })
            .collect();
        let tiers: Vec<Rarity> = relics.iter().map(|(tier, _, _)| *tier).collect();
        assert_eq!(tiers, [Rarity::Common, Rarity::Rare], "seed {seed}");
        for (tier, paid, id) in relics {
            assert_eq!(paid, price(tier), "{id}");
            assert_ne!(content.card(&id).unwrap().role, "Risky", "{id}");
        }
    }
}

#[test]
fn a_mythic_relic_comes_only_from_an_event_or_the_last_lieutenants_loot() {
    let content = content().unwrap();
    let mythic = |ids: &[String]| ids.iter().any(|id| rarity(content, id) == Rarity::Mythic);
    for seed in 0..60 {
        let run = at_first_boss(content, seed, CONTENT);
        let last = bosses(&run).len() - 2;
        for rank in 0..last {
            assert!(!mythic(&loot(content, seed, CONTENT, rank)), "loot {rank}");
        }
        for kind in [RoomKind::Treasure, RoomKind::Shop] {
            let ids = match camp(content, seed, CONTENT, kind) {
                Offer::Relics { relics } => relics,
                Offer::Shop { items } => items
                    .into_iter()
                    .filter_map(|item| match item {
                        ShopItem::Relic { id, .. } => Some(id),
                        _ => None,
                    })
                    .collect(),
                other => panic!("{other:?}"),
            };
            assert!(!mythic(&ids), "{kind:?} seed {seed}");
        }
    }
}

#[rstest]
#[case::generation_two(FILTER_FROM)]
#[case::generation_one(1)]
fn an_older_run_never_meets_the_new_relics_or_events_and_its_shop_keeps_its_prices(
    #[case] version: u32,
) {
    let content = content().unwrap();
    for seed in 0..40 {
        for id in loot(content, seed, version, 0) {
            assert!(content.card(&id).unwrap().since <= version, "{id}");
        }
        let Offer::Shop { items } = camp(content, seed, version, RoomKind::Shop) else {
            panic!("a shop")
        };
        for item in items {
            if let ShopItem::Relic { id, price, .. } = item {
                assert_eq!(price, content.card(&id).unwrap().price, "{id}");
            }
        }
        let Offer::Event { event } = camp(content, seed, version, RoomKind::Event) else {
            panic!("an event")
        };
        let theme = content.theme("warrens").unwrap();
        let spec = theme.event(&event).unwrap();
        assert!(spec.since <= version && spec.until == Some(2), "{event}");
    }
}

#[test]
fn an_older_run_draws_its_loot_exactly_as_before() {
    let content = content().unwrap();
    // Generation 2 with no stored makeup: every relic of generations 0 to 2 in id order, as the draw always saw them.
    let pool: Vec<String> = content
        .relics()
        .iter()
        .filter(|c| c.since <= FILTER_FROM)
        .map(|c| c.id.clone())
        .collect();
    for seed in 0..40 {
        let run = at_first_boss(content, seed, FILTER_FROM);
        let node = run.at.unwrap();
        let expected = Stream::new(seed, "loot", u64::from(node) * 16, 0).pick(&pool, 3);
        assert_eq!(loot(content, seed, FILTER_FROM, 0), expected, "seed {seed}");
    }
}
